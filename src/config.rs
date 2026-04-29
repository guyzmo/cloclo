use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

use crate::chanson::DEFAULT_PORT;
use crate::error::ClocloError;

/// Top-level configuration, loaded from `~/.config/cloclo/config.toml`.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ClocloConfig {
    #[serde(default)]
    pub general: GeneralConfig,
    #[serde(default)]
    pub profiles: HashMap<String, ProfileConfig>,
}

/// General proxy settings.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GeneralConfig {
    #[serde(default = "default_port")]
    pub port: u16,
    #[serde(default = "default_bind")]
    pub bind: String,
    #[serde(default = "default_profile_name")]
    pub default_profile: String,
    pub log_file: Option<PathBuf>,
    pub pid_file: Option<PathBuf>,
    pub claude_bin: Option<PathBuf>,
}

fn default_port() -> u16 {
    DEFAULT_PORT
}
fn default_bind() -> String {
    "127.0.0.1".to_string()
}
fn default_profile_name() -> String {
    "default".to_string()
}

impl Default for GeneralConfig {
    fn default() -> Self {
        Self {
            port: DEFAULT_PORT,
            bind: "127.0.0.1".to_string(),
            default_profile: "default".to_string(),
            log_file: None,
            pid_file: None,
            claude_bin: None,
        }
    }
}

/// A profile configuration, tagged by type.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ProfileConfig {
    #[serde(rename = "api_key")]
    ApiKey {
        display_name: String,
        api_key: SecretSource,
        base_url: Option<String>,
        model: Option<String>,
    },
    #[serde(rename = "oauth")]
    OAuth {
        display_name: String,
        token_file: PathBuf,
        base_url: Option<String>,
        model: Option<String>,
    },
    #[serde(rename = "enterprise_sso")]
    EnterpriseSso {
        display_name: String,
        token_file: PathBuf,
        base_url: Option<String>,
        model: Option<String>,
    },
    #[serde(rename = "proxy")]
    Proxy {
        display_name: String,
        upstream_url: String,
        auth_token: Option<SecretSource>,
        model: Option<String>,
        subprocess: Option<SubprocessConfig>,
    },
}

impl ProfileConfig {
    /// Returns the human-readable display name for this profile.
    pub fn display_name(&self) -> &str {
        match self {
            ProfileConfig::ApiKey { display_name, .. } => display_name,
            ProfileConfig::OAuth { display_name, .. } => display_name,
            ProfileConfig::EnterpriseSso { display_name, .. } => display_name,
            ProfileConfig::Proxy { display_name, .. } => display_name,
        }
    }
}

/// How to retrieve a secret value: literal string, file path, or environment variable.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(untagged)]
pub enum SecretSource {
    Literal(String),
    File { file: PathBuf },
    Env { env: String },
}

/// Configuration for a managed subprocess (e.g., a local LLM proxy).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SubprocessConfig {
    pub command: String,
    #[serde(default)]
    pub args: Vec<String>,
    pub health_port: u16,
    #[serde(default = "default_startup_timeout")]
    pub startup_timeout_secs: u64,
}

fn default_startup_timeout() -> u64 {
    30
}

/// Returns the path to the config file: `~/.config/cloclo/config.toml`.
pub fn config_path() -> PathBuf {
    dirs::config_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("cloclo")
        .join("config.toml")
}

/// Loads the configuration from the default path.
pub fn load_config() -> Result<ClocloConfig, ClocloError> {
    let path = config_path();
    if !path.exists() {
        return Err(ClocloError::Config(format!(
            "Config file not found at {}. Run `cloclo init` to create one.",
            path.display()
        )));
    }
    let contents = std::fs::read_to_string(&path).map_err(|e| {
        ClocloError::Config(format!("Failed to read {}: {}", path.display(), e))
    })?;
    let config: ClocloConfig = toml::from_str(&contents).map_err(|e| {
        ClocloError::Config(format!("Failed to parse {}: {}", path.display(), e))
    })?;
    Ok(config)
}

/// Creates a default config file. If `force` is true, overwrites existing.
pub fn init_config(force: bool) -> Result<(), ClocloError> {
    let path = config_path();
    if path.exists() && !force {
        return Err(ClocloError::Config(format!(
            "Config already exists at {}. Use --force to overwrite.",
            path.display()
        )));
    }
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let default_config = r#"[general]
port = 9393
bind = "127.0.0.1"
default_profile = "default"

[profiles.default]
type = "api_key"
display_name = "Default API Key"
api_key = "sk-ant-REPLACE_ME"
"#;

    std::fs::write(&path, default_config)?;
    Ok(())
}
