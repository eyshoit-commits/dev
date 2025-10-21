use std::{collections::HashMap, path::PathBuf};

use dirs::home_dir;
use serde::{Deserialize, Serialize};

use crate::error::{HelixError, HelixResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HelixConfig {
    pub data_dir: PathBuf,
    pub rest: RestConfig,
    pub security: SecurityConfig,
    pub telemetry: TelemetryConfig,
    pub feature_flags: HelixFeatureFlags,
    pub plugins: HashMap<String, PluginConfig>,
}

impl HelixConfig {
    pub fn load(path: Option<PathBuf>) -> HelixResult<Self> {
        let mut settings = config::Config::builder();
        if let Some(path) = path {
            settings = settings.add_source(config::File::from(path));
        } else if let Some(home) = home_dir() {
            let default = home.join(".helix").join("braindb-config.yaml");
            if default.exists() {
                settings = settings.add_source(config::File::from(default));
            }
        }
        settings = settings.add_source(config::Environment::with_prefix("HELIX").separator("_"));
        let cfg = settings
            .build()
            .map_err(|err| HelixError::Configuration(err.to_string()))?;
        cfg.try_deserialize()
            .map_err(|err| HelixError::Configuration(err.to_string()))
    }

    pub fn ensure_dirs(&self) -> HelixResult<()> {
        std::fs::create_dir_all(&self.data_dir)
            .map_err(|err| HelixError::Configuration(err.to_string()))
    }
}

impl Default for HelixConfig {
    fn default() -> Self {
        let data_dir = home_dir()
            .map(|dir| dir.join(".helix").join("data"))
            .unwrap_or_else(|| PathBuf::from("./data"));
        Self {
            data_dir,
            rest: RestConfig::default(),
            security: SecurityConfig::default(),
            telemetry: TelemetryConfig::default(),
            feature_flags: HelixFeatureFlags::default(),
            plugins: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RestConfig {
    pub bind_addr: String,
    pub cors_allowed_origins: Vec<String>,
}

impl Default for RestConfig {
    fn default() -> Self {
        Self {
            bind_addr: "0.0.0.0:6969".to_string(),
            cors_allowed_origins: vec![],
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SecurityConfig {
    pub api_keys: Vec<ApiKeyConfig>,
    pub jwt_issuer: Option<String>,
    pub jwt_audience: Option<String>,
    pub rbac_roles: HashMap<String, Vec<String>>,
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            api_keys: vec![],
            jwt_issuer: None,
            jwt_audience: None,
            rbac_roles: HashMap::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ApiKeyConfig {
    pub name: String,
    pub key: String,
    pub scopes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TelemetryConfig {
    pub prometheus_endpoint: Option<String>,
    pub enable_logs: bool,
}

impl Default for TelemetryConfig {
    fn default() -> Self {
        Self {
            prometheus_endpoint: Some("0.0.0.0:9600".to_string()),
            enable_logs: true,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct HelixFeatureFlags {
    pub braindb: Vec<String>,
    pub pgml: Vec<String>,
    pub db3: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginConfig {
    pub base_url: String,
    pub api_key: Option<String>,
    pub jwt: Option<String>,
    pub extra: HashMap<String, serde_json::Value>,
}
