use std::path::Path;
use std::path::PathBuf;
use std::sync::Arc;

use codex_agent_identity::AgentIdentityKey;
use codex_agent_identity::AgentTaskAuthorizationTarget;
use codex_agent_identity::authorization_header_for_agent_task;
use codex_api::AuthProvider;
use codex_api::SharedAuthProvider;
use codex_login::AuthManager;
use codex_login::CodexAuth;
use codex_model_provider_info::ModelProviderInfo;
use codex_protocol::error::CodexErr;
use codex_provider_catalog::ProviderAuthStore;
use codex_provider_catalog::resolve_env_key_api_key;
use http::HeaderMap;
use http::HeaderValue;

use crate::bearer_auth_provider::BearerAuthProvider;

const BEDROCK_API_KEY_UNSUPPORTED_MESSAGE: &str =
    "Bedrock API key auth is only supported by the Amazon Bedrock model provider";

/// Runtime context needed to resolve provider credentials from `provider-auth.json`.
#[derive(Debug, Clone)]
pub struct ProviderRuntimeContext {
    pub provider_id: String,
    pub codex_home: PathBuf,
}

impl ProviderRuntimeContext {
    pub fn new(provider_id: impl Into<String>, codex_home: impl Into<PathBuf>) -> Self {
        Self {
            provider_id: provider_id.into(),
            codex_home: codex_home.into(),
        }
    }
}

#[derive(Clone, Debug)]
struct ProviderAuthStoreSnapshot {
    store: ProviderAuthStore,
}

impl ProviderAuthStoreSnapshot {
    fn load(codex_home: &Path) -> Self {
        Self {
            store: ProviderAuthStore::load_from(codex_home).unwrap_or_default(),
        }
    }
}

#[derive(Clone, Debug)]
struct AgentIdentityAuthProvider {
    auth: codex_login::auth::AgentIdentityAuth,
}

impl AuthProvider for AgentIdentityAuthProvider {
    fn add_auth_headers(&self, headers: &mut HeaderMap) {
        let record = self.auth.record();
        let header_value = authorization_header_for_agent_task(
            AgentIdentityKey {
                agent_runtime_id: &record.agent_runtime_id,
                private_key_pkcs8_base64: &record.agent_private_key,
            },
            AgentTaskAuthorizationTarget {
                agent_runtime_id: &record.agent_runtime_id,
                task_id: self.auth.process_task_id(),
            },
        )
        .map_err(std::io::Error::other);

        if let Ok(header_value) = header_value
            && let Ok(header) = HeaderValue::from_str(&header_value)
        {
            let _ = headers.insert(http::header::AUTHORIZATION, header);
        }

        if let Ok(header) = HeaderValue::from_str(self.auth.account_id()) {
            let _ = headers.insert("ChatGPT-Account-ID", header);
        }

        if self.auth.is_fedramp_account() {
            let _ = headers.insert("X-OpenAI-Fedramp", HeaderValue::from_static("true"));
        }
    }
}

// Some providers are meant to send no auth headers. Examples include local OSS
// providers and custom test providers with `requires_openai_auth = false`.
#[derive(Clone, Debug)]
struct UnauthenticatedAuthProvider;

impl AuthProvider for UnauthenticatedAuthProvider {
    fn add_auth_headers(&self, _headers: &mut HeaderMap) {}
}

pub fn unauthenticated_auth_provider() -> SharedAuthProvider {
    Arc::new(UnauthenticatedAuthProvider)
}

/// Returns the provider-scoped auth manager when this provider uses command-backed auth.
///
/// Providers without custom auth continue using the caller-supplied base manager, when present.
pub(crate) fn auth_manager_for_provider(
    auth_manager: Option<Arc<AuthManager>>,
    provider: &ModelProviderInfo,
) -> Option<Arc<AuthManager>> {
    match provider.auth.clone() {
        Some(config) => Some(AuthManager::external_bearer_only(config)),
        None => auth_manager,
    }
}

pub(crate) fn resolve_provider_auth(
    auth: Option<&CodexAuth>,
    provider: &ModelProviderInfo,
    runtime_context: Option<&ProviderRuntimeContext>,
) -> codex_protocol::error::Result<SharedAuthProvider> {
    if matches!(auth, Some(CodexAuth::BedrockApiKey(_))) {
        return Err(CodexErr::UnsupportedOperation(
            BEDROCK_API_KEY_UNSUPPORTED_MESSAGE.to_string(),
        ));
    }

    if let Some(auth) = bearer_auth_for_provider(provider, runtime_context)? {
        return Ok(Arc::new(auth));
    }

    Ok(match auth {
        Some(auth) => auth_provider_from_auth(auth),
        None => unauthenticated_auth_provider(),
    })
}

fn bearer_auth_for_provider(
    provider: &ModelProviderInfo,
    runtime_context: Option<&ProviderRuntimeContext>,
) -> codex_protocol::error::Result<Option<BearerAuthProvider>> {
    if let Some(api_key) = resolve_env_key_auth(provider, runtime_context)? {
        return Ok(Some(BearerAuthProvider::new(api_key)));
    }

    if let Some(token) = provider.experimental_bearer_token.clone() {
        return Ok(Some(BearerAuthProvider::new(token)));
    }

    Ok(None)
}

fn resolve_env_key_auth(
    provider: &ModelProviderInfo,
    runtime_context: Option<&ProviderRuntimeContext>,
) -> codex_protocol::error::Result<Option<String>> {
    let Some(runtime_context) = runtime_context else {
        return provider.api_key();
    };

    resolve_provider_env_key_auth(
        &runtime_context.provider_id,
        provider,
        &runtime_context.codex_home,
    )
}

/// Resolves an API key for providers configured with `env_key`.
pub fn resolve_provider_env_key_auth(
    provider_id: &str,
    provider: &ModelProviderInfo,
    codex_home: &Path,
) -> codex_protocol::error::Result<Option<String>> {
    let auth_store = ProviderAuthStoreSnapshot::load(codex_home);
    resolve_env_key_api_key(provider_id, provider, &auth_store.store)
}

/// Builds request-header auth for a first-party Codex auth snapshot.
pub fn auth_provider_from_auth(auth: &CodexAuth) -> SharedAuthProvider {
    match auth {
        CodexAuth::AgentIdentity(auth) => {
            Arc::new(AgentIdentityAuthProvider { auth: auth.clone() })
        }
        CodexAuth::BedrockApiKey(_) => unreachable!("{BEDROCK_API_KEY_UNSUPPORTED_MESSAGE}"),
        CodexAuth::ApiKey(_)
        | CodexAuth::Chatgpt(_)
        | CodexAuth::ChatgptAuthTokens(_)
        | CodexAuth::PersonalAccessToken(_) => Arc::new(BearerAuthProvider {
            token: auth.get_token().ok(),
            account_id: auth.get_account_id(),
            is_fedramp_account: auth.is_fedramp_account(),
        }),
    }
}

#[cfg(test)]
mod tests {
    use codex_login::auth::BedrockApiKeyAuth;
    use codex_model_provider_info::WireApi;
    use codex_model_provider_info::create_oss_provider_with_base_url;
    use pretty_assertions::assert_eq;

    use super::*;

    #[test]
    fn unauthenticated_auth_provider_adds_no_headers() {
        let provider =
            create_oss_provider_with_base_url("http://localhost:11434/v1", WireApi::Responses);
        let auth =
            resolve_provider_auth(/*auth*/ None, &provider, /*runtime_context*/ None)
                .expect("auth should resolve");

        assert!(auth.to_auth_headers().is_empty());
    }

    #[test]
    fn openai_provider_rejects_bedrock_api_key_auth() {
        let provider = ModelProviderInfo::create_openai_provider(/*base_url*/ None);
        let auth = CodexAuth::BedrockApiKey(BedrockApiKeyAuth {
            api_key: "bedrock-api-key-test".to_string(),
            region: "us-east-1".to_string(),
        });

        match resolve_provider_auth(Some(&auth), &provider, /*runtime_context*/ None) {
            Err(CodexErr::UnsupportedOperation(message)) => {
                assert_eq!(message, BEDROCK_API_KEY_UNSUPPORTED_MESSAGE);
            }
            Err(err) => panic!("unexpected auth error: {err:?}"),
            Ok(_) => panic!("Bedrock API key auth should be rejected"),
        }
    }
}
