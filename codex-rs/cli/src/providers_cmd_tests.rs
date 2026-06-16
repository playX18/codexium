use codex_model_provider_info::CodexChatReasoningConfig;
use codex_model_provider_info::ModelProviderInfo;
use codex_model_provider_info::UpstreamWireApi;
use codex_model_provider_info::WireApi;
use codex_models_dev::ModelsDevModel;
use codex_models_dev::ModelsDevModelLimit;
use codex_models_dev::ModelsDevProvider;
use pretty_assertions::assert_eq;
use std::fs;
use tempfile::TempDir;

use super::default_model_id;
use super::model_provider_info_to_toml_item;
use super::openai_auth_status;
use super::write_provider_activation_config;

fn xiaomi_provider_info() -> ModelProviderInfo {
    ModelProviderInfo {
        name: "Xiaomi Token Plan (Singapore)".into(),
        base_url: Some("https://token-plan-sgp.xiaomimimo.com/v1".into()),
        env_key: Some("XIAOMI_API_KEY".into()),
        env_key_instructions: Some(
            "Set the `XIAOMI_API_KEY` environment variable with your Xiaomi Token Plan (Singapore) API key.".into(),
        ),
        wire_api: WireApi::Responses,
        upstream_wire_api: UpstreamWireApi::ChatCompletions,
        codex_chat_reasoning: Some(CodexChatReasoningConfig {
            supports_thinking: Some(true),
            supports_effort: Some(true),
            thinking_param: Some("thinking".to_string()),
            effort_param: Some("reasoning_effort".to_string()),
            effort_value_mode: Some("openai".to_string()),
            output_format: None,
        }),
        requires_openai_auth: false,
        ..Default::default()
    }
}

#[test]
fn model_provider_info_to_toml_item_parses_xiaomi_provider() {
    let item = model_provider_info_to_toml_item(&xiaomi_provider_info())
        .expect("provider info should convert to toml_edit item");
    let table = item.as_table().expect("provider info should be a table");
    assert_eq!(
        table.get("name").and_then(|value| value.as_str()),
        Some("Xiaomi Token Plan (Singapore)")
    );
    assert_eq!(
        table.get("base_url").and_then(|value| value.as_str()),
        Some("https://token-plan-sgp.xiaomimimo.com/v1")
    );
}

#[test]
fn write_provider_activation_config_produces_valid_toml() {
    let codex_home = TempDir::new().expect("tempdir");
    let provider_id = "xiaomi-token-plan-sgp";
    let provider_info = xiaomi_provider_info();

    write_provider_activation_config(
        codex_home.path(),
        provider_id,
        provider_info,
        Some("mimo-v2-flash".to_string()),
    )
    .expect("config write should succeed");

    let contents =
        fs::read_to_string(codex_home.path().join("config.toml")).expect("config should exist");
    let doc =
        toml::from_str::<toml::Value>(&contents).expect("written config.toml must be valid TOML");

    assert_eq!(
        doc.get("model_provider").and_then(|value| value.as_str()),
        Some(provider_id)
    );
    assert_eq!(
        doc.get("model").and_then(|value| value.as_str()),
        Some("mimo-v2-flash")
    );
    assert_eq!(
        doc.get("model_catalog_json")
            .and_then(|value| value.as_str()),
        Some("provider-catalog/xiaomi-token-plan-sgp.json")
    );

    let provider_table = doc
        .get("model_providers")
        .and_then(|value| value.get(provider_id))
        .expect("provider table should exist");
    assert_eq!(
        provider_table.get("name").and_then(|value| value.as_str()),
        Some("Xiaomi Token Plan (Singapore)")
    );
    assert_eq!(
        provider_table
            .get("base_url")
            .and_then(|value| value.as_str()),
        Some("https://token-plan-sgp.xiaomimimo.com/v1")
    );
    assert_eq!(
        provider_table
            .get("env_key")
            .and_then(|value| value.as_str()),
        Some("XIAOMI_API_KEY")
    );
    assert!(
        doc.get("model_providers")
            .and_then(|v| v.get(provider_id))
            .is_some()
    );
}

#[test]
fn openai_auth_status_reads_oauth_auth_json() {
    let codex_home = TempDir::new().expect("tempdir");
    fs::write(
        codex_home.path().join("auth.json"),
        r#"{"OPENAI_API_KEY":null,"tokens":{"access_token":"token"}}"#,
    )
    .expect("write auth");

    assert_eq!(
        openai_auth_status(codex_home.path()).expect("auth status"),
        Some("oauth")
    );
}

#[test]
fn openai_auth_status_reads_api_key_auth_json() {
    let codex_home = TempDir::new().expect("tempdir");
    fs::write(
        codex_home.path().join("auth.json"),
        r#"{"OPENAI_API_KEY":"sk-test"}"#,
    )
    .expect("write auth");

    assert_eq!(
        openai_auth_status(codex_home.path()).expect("auth status"),
        Some("api-key")
    );
}

#[test]
fn write_provider_activation_config_merges_existing_config() {
    let codex_home = TempDir::new().expect("tempdir");
    fs::write(
        codex_home.path().join("config.toml"),
        "profile = \"work\"\nmodel = \"gpt-5\"\n",
    )
    .expect("seed config");

    write_provider_activation_config(
        codex_home.path(),
        "anthropic",
        ModelProviderInfo {
            name: "Anthropic".into(),
            base_url: Some("https://api.anthropic.com/v1".into()),
            env_key: Some("ANTHROPIC_API_KEY".into()),
            wire_api: WireApi::Responses,
            upstream_wire_api: UpstreamWireApi::ChatCompletions,
            requires_openai_auth: false,
            ..Default::default()
        },
        Some("claude-sonnet".to_string()),
    )
    .expect("config merge should succeed");

    let doc = toml::from_str::<toml::Value>(
        &fs::read_to_string(codex_home.path().join("config.toml")).expect("config should exist"),
    )
    .expect("merged config must remain valid TOML");

    assert_eq!(
        doc.get("profile").and_then(|value| value.as_str()),
        Some("work")
    );
    assert_eq!(
        doc.get("model").and_then(|value| value.as_str()),
        Some("claude-sonnet")
    );
    assert!(doc.get("model_providers").is_some());
}

#[test]
fn default_model_id_uses_lexicographically_first_model() {
    let provider = ModelsDevProvider {
        id: "anthropic".to_string(),
        name: "Anthropic".to_string(),
        env: vec!["ANTHROPIC_API_KEY".to_string()],
        api: Some("https://api.anthropic.com/v1".to_string()),
        npm: None,
        models: [
            (
                "claude-sonnet".to_string(),
                sample_model("claude-sonnet", "Claude Sonnet"),
            ),
            (
                "claude-haiku".to_string(),
                sample_model("claude-haiku", "Claude Haiku"),
            ),
        ]
        .into(),
    };

    assert_eq!(default_model_id(&provider).as_deref(), Some("claude-haiku"));
}

fn sample_model(id: &str, name: &str) -> ModelsDevModel {
    ModelsDevModel {
        id: id.to_string(),
        name: name.to_string(),
        reasoning: false,
        tool_call: true,
        attachment: false,
        temperature: true,
        release_date: None,
        family: None,
        limit: ModelsDevModelLimit {
            context: 128_000,
            output: 8_192,
            input: None,
        },
        status: None,
        cost: None,
    }
}
