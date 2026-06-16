use super::manager::ModelsManager;
use super::manager::ModelsManagerFuture;
use super::manager::RefreshStrategy;
use super::manager::StaticModelsManager;
use crate::manager::OpenAiModelsManager;
use codex_login::AuthManager;
use codex_models_dev::ModelsDevCache;
use codex_protocol::config_types::CollaborationModeMask;
use codex_protocol::openai_models::ModelInfo;
use codex_protocol::openai_models::ModelsResponse;
use codex_provider_catalog::ProviderCatalogStore;
use codex_provider_catalog::build_models_response;
use codex_provider_catalog::write_provider_catalog;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::TryLockError;
use tracing::warn;

/// Offline-first model manager backed by generated provider catalogs.
#[derive(Debug)]
pub struct ProviderCatalogModelsManager {
    inner: StaticModelsManager,
    provider_id: String,
    codex_home: PathBuf,
    models_dev: ModelsDevCache,
}

impl ProviderCatalogModelsManager {
    pub fn new(
        codex_home: PathBuf,
        provider_id: String,
        auth_manager: Option<Arc<AuthManager>>,
        initial_catalog: ModelsResponse,
    ) -> Self {
        let models_dev_cache_path = codex_home.join(codex_models_dev::MODELS_DEV_CACHE_FILE);
        Self {
            inner: StaticModelsManager::new(auth_manager, initial_catalog),
            provider_id,
            codex_home: codex_home.clone(),
            models_dev: ModelsDevCache::with_paths(
                models_dev_cache_path,
                codex_models_dev::DEFAULT_MODELS_DEV_URL.to_string(),
            ),
        }
    }

    pub fn spawn_background_refresh(&self) {
        let provider_id = self.provider_id.clone();
        let codex_home = self.codex_home.clone();
        let models_dev = self.models_dev.clone();
        tokio::spawn(async move {
            let Ok(providers) = models_dev.get(/*force_refresh*/ false).await else {
                return;
            };
            let Some(provider) = providers.get(&provider_id) else {
                warn!(provider_id, "provider missing from models.dev refresh");
                return;
            };
            if let Err(err) = write_provider_catalog(&codex_home, &provider_id, provider) {
                warn!(provider_id, "failed to write provider catalog: {err}");
            }
        });
    }

    pub async fn bootstrap_from_disk(
        codex_home: PathBuf,
        provider_id: String,
        auth_manager: Option<Arc<AuthManager>>,
    ) -> Option<Self> {
        let store = ProviderCatalogStore::new(codex_home.clone());
        let catalog = store.load(&provider_id).ok()??;
        Some(Self::new(codex_home, provider_id, auth_manager, catalog))
    }
}

impl ModelsManager for ProviderCatalogModelsManager {
    fn raw_model_catalog(
        &self,
        refresh_strategy: RefreshStrategy,
    ) -> ModelsManagerFuture<'_, ModelsResponse> {
        let provider_id = self.provider_id.clone();
        let codex_home = self.codex_home.clone();
        let models_dev = self.models_dev.clone();
        Box::pin(async move {
            if refresh_strategy == RefreshStrategy::Online {
                if let Ok(providers) = models_dev.get(/*force_refresh*/ false).await {
                    if let Some(provider) = providers.get(&provider_id) {
                        let response = build_models_response(provider);
                        let store = ProviderCatalogStore::new(codex_home.clone());
                        let _ = store.save(&provider_id, &response);
                        return response;
                    }
                }
            }
            self.inner.raw_model_catalog(RefreshStrategy::Offline).await
        })
    }

    fn get_remote_models(&self) -> ModelsManagerFuture<'_, Vec<ModelInfo>> {
        self.inner.get_remote_models()
    }

    fn try_get_remote_models(&self) -> Result<Vec<ModelInfo>, TryLockError> {
        self.inner.try_get_remote_models()
    }

    fn auth_manager(&self) -> Option<&AuthManager> {
        self.inner.auth_manager()
    }

    fn list_collaboration_modes(&self) -> Vec<CollaborationModeMask> {
        self.inner.list_collaboration_modes()
    }

    fn refresh_if_new_etag(&self, _etag: String) -> ModelsManagerFuture<'_, ()> {
        Box::pin(async {})
    }
}

pub fn provider_scoped_cache_path(codex_home: &PathBuf, provider_id: &str) -> PathBuf {
    codex_home.join(format!("models_cache.{provider_id}.json"))
}

pub fn openai_models_manager_with_provider_scope(
    codex_home: PathBuf,
    provider_id: &str,
    endpoint_client: Arc<dyn super::manager::ModelsEndpointClient>,
    auth_manager: Option<Arc<AuthManager>>,
) -> OpenAiModelsManager {
    let cache_path = provider_scoped_cache_path(&codex_home, provider_id);
    OpenAiModelsManager::new_with_cache_path(codex_home, cache_path, endpoint_client, auth_manager)
}
