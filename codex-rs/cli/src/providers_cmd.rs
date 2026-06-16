use crate::provider_picker::pick_provider_id;
use clap::Parser;
use clap::Subcommand;
use codex_core::config::find_codex_home;
use codex_model_provider_info::ModelProviderInfo;
use codex_model_provider_info::OPENAI_PROVIDER_ID;
use codex_models_dev::ModelsDevCache;
use codex_models_dev::ModelsDevProvider;
use codex_provider_catalog::PROVIDER_CATALOG_DIR;
use codex_provider_catalog::ProviderAuthStore;
use codex_provider_catalog::map_provider_to_model_provider_info;
use codex_provider_catalog::write_provider_catalog;
use crossterm::event::Event;
use crossterm::event::KeyCode;
use crossterm::event::KeyEventKind;
use crossterm::event::KeyModifiers;
use crossterm::terminal::disable_raw_mode;
use crossterm::terminal::enable_raw_mode;
use owo_colors::OwoColorize;
use std::collections::HashMap;
use std::fs;
use std::io::IsTerminal;
use std::io::Write;
use std::io::{self};
use std::path::Path;
use std::str::FromStr;
use toml::Value as TomlValue;

#[derive(Debug, Parser)]
pub struct ProvidersCli {
    #[clap(subcommand)]
    pub subcommand: ProvidersSubcommand,
}

#[derive(Debug, Subcommand)]
pub enum ProvidersSubcommand {
    /// List configured third-party providers and credential status.
    List,
    /// Log in to a models.dev provider (API key).
    Login(ProvidersLoginArgs),
    /// Remove stored credentials for a provider.
    Logout(ProvidersLogoutArgs),
    /// Force refresh the models.dev catalog cache.
    Refresh,
}

#[derive(Debug, Parser)]
pub struct ProvidersLoginArgs {
    /// Provider id from models.dev (for example `anthropic`, `openrouter`).
    pub provider_id: Option<String>,
}

#[derive(Debug, Parser)]
pub struct ProvidersLogoutArgs {
    /// Provider id to remove from provider-auth.json.
    pub provider_id: Option<String>,
}

pub async fn run_providers_command(cli: ProvidersCli) -> io::Result<()> {
    match cli.subcommand {
        ProvidersSubcommand::List => run_list().await,
        ProvidersSubcommand::Login(args) => run_login(args).await,
        ProvidersSubcommand::Logout(args) => run_logout(args).await,
        ProvidersSubcommand::Refresh => run_refresh().await,
    }
}

async fn run_list() -> io::Result<()> {
    let codex_home = find_codex_home().map_err(io::Error::other)?;
    let auth = ProviderAuthStore::load_from(codex_home.as_path())?;
    let active = read_active_provider_id(codex_home.as_path())?;
    let openai_auth = openai_auth_status(codex_home.as_path())?;

    println!("{}", "Configured providers".bold());
    println!("Active provider: {active}");
    if let Some(status) = openai_auth {
        println!("- {OPENAI_PROVIDER_ID}: {status}");
    }
    if auth.entries.is_empty() && openai_auth.is_none() {
        println!("No credentials in auth.json or provider-auth.json");
        return Ok(());
    }

    for (provider_id, entry) in &auth.entries {
        let status = match entry {
            codex_provider_catalog::ProviderAuthEntry::Api { .. } => "api-key",
            codex_provider_catalog::ProviderAuthEntry::Oauth { .. } => "oauth",
        };
        println!("- {provider_id}: {status}");
    }
    Ok(())
}

fn openai_auth_status(codex_home: &Path) -> io::Result<Option<&'static str>> {
    let auth_path = codex_home.join("auth.json");
    let contents = match fs::read_to_string(auth_path) {
        Ok(contents) => contents,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(err) => return Err(err),
    };
    let value = serde_json::from_str::<serde_json::Value>(&contents)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err.to_string()))?;
    if value.get("tokens").is_some_and(|tokens| !tokens.is_null()) {
        return Ok(Some("oauth"));
    }
    if value
        .get("OPENAI_API_KEY")
        .and_then(|key| key.as_str())
        .is_some_and(|key| !key.is_empty())
    {
        return Ok(Some("api-key"));
    }
    Ok(None)
}

async fn run_refresh() -> io::Result<()> {
    let codex_home = find_codex_home().map_err(io::Error::other)?;
    let cache = ModelsDevCache::new(codex_home.to_path_buf());
    cache
        .refresh(/*force*/ true)
        .await
        .map_err(io::Error::other)?;
    println!("{}", "models.dev cache refreshed".green());
    Ok(())
}

async fn run_login(args: ProvidersLoginArgs) -> io::Result<()> {
    let codex_home = find_codex_home().map_err(io::Error::other)?;
    let cache = ModelsDevCache::new(codex_home.to_path_buf());
    let providers = cache
        .get(/*force_refresh*/ false)
        .await
        .map_err(io::Error::other)?;
    let providers = filter_providers_for_config(&providers, codex_home.as_path())?;

    let provider_id = match args.provider_id {
        Some(id) => id,
        None => {
            require_interactive_terminal(
                "interactive provider login requires a TTY; pass a provider id: `codexium providers login <provider-id>`",
            )?;
            pick_provider_id(&providers)?
        }
    };

    let Some(provider) = providers.get(&provider_id) else {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("unknown provider `{provider_id}`"),
        ));
    };

    let api_key = read_api_key_prompt(&provider.name)?;

    let mut auth = ProviderAuthStore::load_from(codex_home.as_path())?;
    auth.set_api_key(&provider_id, api_key);
    auth.save_to(codex_home.as_path())?;

    let catalog_path = write_provider_catalog(codex_home.as_path(), &provider_id, provider)
        .map_err(io::Error::other)?;
    let provider_info = map_provider_to_model_provider_info(provider);
    if let Some(env_key) = provider_info.env_key.as_deref() {
        auth.apply_env_for_provider(&provider_id, env_key)?;
    }

    write_provider_activation_config(
        codex_home.as_path(),
        &provider_id,
        provider_info,
        default_model_id(provider),
    )?;

    println!(
        "{}",
        format!(
            "Provider `{provider_id}` configured. Catalog: {}",
            catalog_path.display()
        )
        .green()
    );
    println!("{}", "ChatGPT auth.json was not modified.".dimmed());
    Ok(())
}

async fn run_logout(args: ProvidersLogoutArgs) -> io::Result<()> {
    let codex_home = find_codex_home().map_err(io::Error::other)?;
    let provider_id = match args.provider_id {
        Some(id) => id,
        None => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "provider id is required",
            ));
        }
    };
    let mut auth = ProviderAuthStore::load_from(codex_home.as_path())?;
    if auth.remove(&provider_id).is_none() {
        return Err(io::Error::new(
            io::ErrorKind::NotFound,
            format!("no credentials for `{provider_id}`"),
        ));
    }
    auth.save_to(codex_home.as_path())?;
    println!(
        "{}",
        format!("Removed credentials for `{provider_id}`").green()
    );
    Ok(())
}

fn read_active_provider_id(codex_home: &Path) -> io::Result<String> {
    let config_path = codex_home.join("config.toml");
    let contents = match fs::read_to_string(&config_path) {
        Ok(contents) => contents,
        Err(err) if err.kind() == io::ErrorKind::NotFound => {
            return Ok(OPENAI_PROVIDER_ID.to_string());
        }
        Err(err) => return Err(err),
    };
    let doc = toml::from_str::<TomlValue>(&contents)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err.to_string()))?;
    Ok(doc
        .get("model_provider")
        .and_then(|value| value.as_str())
        .unwrap_or(OPENAI_PROVIDER_ID)
        .to_string())
}

fn write_provider_activation_config(
    codex_home: &Path,
    provider_id: &str,
    provider_info: ModelProviderInfo,
    default_model: Option<String>,
) -> io::Result<()> {
    let config_path = codex_home.join("config.toml");
    let mut doc = match fs::read_to_string(&config_path) {
        Ok(contents) => toml_edit::DocumentMut::from_str(&contents)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err.to_string()))?,
        Err(err) if err.kind() == io::ErrorKind::NotFound => toml_edit::DocumentMut::new(),
        Err(err) => return Err(err),
    };

    doc["model_provider"] = toml_edit::value(provider_id);
    doc["model_catalog_json"] =
        toml_edit::value(format!("{PROVIDER_CATALOG_DIR}/{provider_id}.json"));
    if let Some(model) = default_model {
        doc["model"] = toml_edit::value(model);
    }

    let provider_table = model_provider_info_to_toml_item(&provider_info)?;
    doc["model_providers"][provider_id] = provider_table;

    fs::write(config_path, doc.to_string())?;
    Ok(())
}

fn model_provider_info_to_toml_item(
    provider_info: &ModelProviderInfo,
) -> io::Result<toml_edit::Item> {
    let value = TomlValue::try_from(provider_info)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err.to_string()))?;
    toml_value_to_item(&value)
}

fn toml_value_to_item(value: &TomlValue) -> io::Result<toml_edit::Item> {
    match value {
        TomlValue::Table(table) => {
            let mut table_item = toml_edit::Table::new();
            table_item.set_implicit(false);
            for (key, val) in table {
                table_item.insert(key, toml_value_to_item(val)?);
            }
            Ok(toml_edit::Item::Table(table_item))
        }
        other => Ok(toml_edit::Item::Value(toml_value_to_value(other)?)),
    }
}

fn toml_value_to_value(value: &TomlValue) -> io::Result<toml_edit::Value> {
    match value {
        TomlValue::String(val) => Ok(toml_edit::Value::from(val.clone())),
        TomlValue::Integer(val) => Ok(toml_edit::Value::from(*val)),
        TomlValue::Float(val) => Ok(toml_edit::Value::from(*val)),
        TomlValue::Boolean(val) => Ok(toml_edit::Value::from(*val)),
        TomlValue::Datetime(val) => Ok(toml_edit::Value::from(*val)),
        TomlValue::Array(items) => {
            let mut array = toml_edit::Array::new();
            for item in items {
                array.push(toml_value_to_value(item)?);
            }
            Ok(toml_edit::Value::Array(array))
        }
        TomlValue::Table(table) => {
            let mut inline = toml_edit::InlineTable::new();
            for (key, val) in table {
                inline.insert(key, toml_value_to_value(val)?);
            }
            Ok(toml_edit::Value::InlineTable(inline))
        }
    }
}

fn default_model_id(provider: &ModelsDevProvider) -> Option<String> {
    let mut ids: Vec<_> = provider.models.keys().cloned().collect();
    ids.sort();
    ids.into_iter().next()
}

fn require_interactive_terminal(message: &str) -> io::Result<()> {
    if io::stdin().is_terminal() && io::stdout().is_terminal() {
        Ok(())
    } else {
        Err(io::Error::new(io::ErrorKind::NotConnected, message))
    }
}

fn read_api_key_prompt(provider_name: &str) -> io::Result<String> {
    let prompt = format!("Enter API key for {provider_name}: ");
    let api_key = if io::stdin().is_terminal() {
        read_hidden_line(&prompt)?
    } else {
        print!("{prompt}");
        io::stdout().flush()?;
        read_line_from_stdin()?
    };
    if api_key.is_empty() {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            "API key must not be empty",
        ));
    }
    Ok(api_key)
}

fn read_hidden_line(prompt: &str) -> io::Result<String> {
    print!("{prompt}");
    io::stdout().flush()?;
    enable_raw_mode()?;
    let result = read_hidden_line_raw();
    disable_raw_mode()?;
    println!();
    result
}

fn read_hidden_line_raw() -> io::Result<String> {
    let mut line = String::new();
    loop {
        let event = crossterm::event::read()?;
        let Event::Key(key) = event else {
            continue;
        };
        if key.kind != KeyEventKind::Press {
            continue;
        }
        match key.code {
            KeyCode::Enter => break,
            KeyCode::Char('c') if key.modifiers.contains(KeyModifiers::CONTROL) => {
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "API key entry cancelled",
                ));
            }
            KeyCode::Char(c) => line.push(c),
            KeyCode::Backspace => {
                line.pop();
            }
            KeyCode::Esc => {
                return Err(io::Error::new(
                    io::ErrorKind::Interrupted,
                    "API key entry cancelled",
                ));
            }
            _ => {}
        }
    }
    Ok(line)
}

fn read_line_from_stdin() -> io::Result<String> {
    let mut line = String::new();
    io::stdin().read_line(&mut line)?;
    Ok(line.trim().to_string())
}

fn filter_providers_for_config(
    providers: &HashMap<String, ModelsDevProvider>,
    codex_home: &Path,
) -> io::Result<HashMap<String, ModelsDevProvider>> {
    let config_path = codex_home.join("config.toml");
    let contents = match fs::read_to_string(&config_path) {
        Ok(contents) => contents,
        Err(err) if err.kind() == io::ErrorKind::NotFound => return Ok(providers.clone()),
        Err(err) => return Err(err),
    };
    let doc = toml::from_str::<TomlValue>(&contents)
        .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err.to_string()))?;
    let enabled = doc
        .get("enabled_providers")
        .and_then(|value| value.as_array())
        .map(|values| {
            values
                .iter()
                .filter_map(|value| value.as_str().map(str::to_string))
                .collect::<Vec<_>>()
        });
    let disabled = doc
        .get("disabled_providers")
        .and_then(|value| value.as_array())
        .map(|values| {
            values
                .iter()
                .filter_map(|value| value.as_str().map(str::to_string))
                .collect::<Vec<_>>()
        })
        .unwrap_or_default();

    let filtered = providers
        .iter()
        .filter(|(id, _)| {
            if disabled.iter().any(|disabled_id| disabled_id == *id) {
                return false;
            }
            if let Some(enabled) = enabled.as_ref() {
                return enabled.iter().any(|enabled_id| enabled_id == *id);
            }
            true
        })
        .map(|(id, provider)| (id.clone(), provider.clone()))
        .collect();
    Ok(filtered)
}

#[cfg(test)]
#[path = "providers_cmd_tests.rs"]
mod tests;
