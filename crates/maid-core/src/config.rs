use config::FileFormat;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_with::{serde_as, DurationSeconds};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};
use std::time::Duration;
use validator::{Validate, ValidationError};

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct GooseRunConfig {
    #[validate(length(min = 1))]
    pub target_base_url: String,
    #[validate(range(min = 1, max = 100_000))]
    pub users: u32,
    #[validate(range(min = 1))]
    pub hatch_rate: u32,
    #[validate]
    pub duration: GooseDuration,
    #[validate(range(min = 0, max = 60))]
    pub think_time_seconds: u64,
    #[serde(default)]
    #[validate(length(min = 1))]
    pub scenarios: Vec<ScenarioConfig>,
    #[serde(default)]
    pub scheduler: SchedulerType,
    #[serde(default)]
    pub tls: TlsOptions,
    #[serde(default)]
    pub reports: ReportOptions,
    #[serde(default)]
    pub max_history: Option<u32>,
    #[serde(default)]
    pub log_level: LogLevel,
    #[serde(default)]
    pub throughput_cap_rps: Option<u64>,
    #[serde(default)]
    pub plugin_hints: PluginHints,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct ScenarioConfig {
    #[validate(length(min = 1))]
    pub name: String,
    #[serde(default)]
    #[validate(length(min = 1))]
    pub transactions: Vec<TransactionConfig>,
    #[serde(default)]
    pub weight: u32,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct TransactionConfig {
    #[validate(length(min = 1))]
    pub name: String,
    #[serde(default)]
    pub weight: u32,
    #[validate]
    pub request: RequestConfig,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct RequestConfig {
    #[validate(custom = "validate_http_method")]
    pub method: String,
    #[validate(length(min = 1))]
    pub path: String,
    #[serde(default)]
    pub headers: Vec<HeaderConfig>,
    #[serde(default)]
    pub query: Vec<QueryConfig>,
    #[serde(default)]
    pub body: Option<String>,
    #[serde(default)]
    pub allow_redirects: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct HeaderConfig {
    #[validate(length(min = 1))]
    pub name: String,
    #[validate(length(min = 1))]
    pub value: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct QueryConfig {
    #[validate(length(min = 1))]
    pub name: String,
    #[validate(length(min = 1))]
    pub value: String,
}

#[serde_as]
#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, Validate)]
#[serde(rename_all = "camelCase")]
pub struct GooseDuration {
    #[serde_as(as = "DurationSeconds<u64>")]
    pub seconds: Duration,
}

impl GooseDuration {
    pub fn as_secs(&self) -> u64 {
        self.seconds.as_secs()
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PluginHints {
    #[serde(default)]
    pub mistral_recipe_prompt: Option<String>,
    #[serde(default)]
    pub tags: Vec<String>,
}

impl Default for PluginHints {
    fn default() -> Self {
        Self {
            mistral_recipe_prompt: None,
            tags: Vec::new(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct TlsOptions {
    #[serde(default)]
    pub verify: bool,
    #[serde(default)]
    pub ca_bundle: Option<PathBuf>,
}

impl Default for TlsOptions {
    fn default() -> Self {
        Self {
            verify: true,
            ca_bundle: None,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ReportOptions {
    #[serde(default = "ReportOptions::default_formats")]
    pub formats: Vec<ReportFormat>,
}

impl ReportOptions {
    fn default_formats() -> Vec<ReportFormat> {
        vec![ReportFormat::Json, ReportFormat::Csv, ReportFormat::Html]
    }
}

impl Default for ReportOptions {
    fn default() -> Self {
        Self {
            formats: Self::default_formats(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "kebab-case")]
pub enum ReportFormat {
    Json,
    Csv,
    Html,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "kebab-case")]
pub enum SchedulerType {
    #[default]
    RoundRobin,
    Serial,
    Random,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Trace,
    Debug,
    #[default]
    Info,
    Warn,
    Error,
}

fn validate_http_method(value: &str) -> Result<(), ValidationError> {
    let allowed = ["GET", "POST", "PUT", "PATCH", "DELETE", "HEAD", "OPTIONS"];
    if allowed.contains(&value.to_uppercase().as_str()) {
        Ok(())
    } else {
        Err(ValidationError::new("unsupported_method"))
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub server: ServerConfig,
    pub persistence: PersistenceConfig,
    pub security: SecurityConfig,
    #[serde(default)]
    pub plugin_bus: PluginBusConfig,
}

impl Settings {
    pub fn load() -> anyhow::Result<Self> {
        let mut builder = config::Config::builder();
        let cwd = std::env::current_dir()?;
        let base = cwd.join("config.json");
        let runtime = cwd.join("config.runtime.json");
        builder = builder.add_source(
            config::File::from(Path::new("/etc/maid/config.json"))
                .format(config::FileFormat::Json)
                .required(false),
        );
        if base.exists() {
            builder = builder.add_source(config::File::from(base).format(config::FileFormat::Json));
        }
        if runtime.exists() {
            builder =
                builder.add_source(config::File::from(runtime).format(config::FileFormat::Json));
        }
        builder = builder.add_source(
            config::Environment::with_prefix("MAID")
                .separator("__")
                .try_parsing(true)
                .list_separator(","),
        );
        let settings: Settings = builder.build()?.try_deserialize()?;
        Ok(settings)
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            server: ServerConfig::default(),
            persistence: PersistenceConfig::default(),
            security: SecurityConfig::default(),
            plugin_bus: PluginBusConfig::default(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ServerConfig {
    #[serde(default = "ServerConfig::default_host")]
    pub host: IpAddr,
    #[serde(default = "ServerConfig::default_port")]
    pub port: u16,
    #[serde(default)]
    pub websocket_path: String,
}

impl ServerConfig {
    fn default_host() -> IpAddr {
        IpAddr::V4(Ipv4Addr::UNSPECIFIED)
    }

    fn default_port() -> u16 {
        43121
    }

    pub fn socket_addr(&self) -> SocketAddr {
        SocketAddr::new(self.host, self.port)
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self {
            host: Self::default_host(),
            port: Self::default_port(),
            websocket_path: "/api/goose/stream".to_string(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PersistenceConfig {
    #[serde(default = "PersistenceConfig::default_db_path")]
    pub database_path: PathBuf,
    #[serde(default = "PersistenceConfig::default_report_dir")]
    pub report_dir: PathBuf,
}

impl PersistenceConfig {
    fn default_db_path() -> PathBuf {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("./data"))
            .join("maid")
            .join("goose_runs.sqlite")
    }

    fn default_report_dir() -> PathBuf {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("./data"))
            .join("maid")
            .join("reports")
    }
}

impl Default for PersistenceConfig {
    fn default() -> Self {
        Self {
            database_path: Self::default_db_path(),
            report_dir: Self::default_report_dir(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SecurityConfig {
    #[serde(default = "SecurityConfig::default_required")]
    pub require_authentication: bool,
    #[serde(default)]
    pub api_keys_url: Option<String>,
    #[serde(default)]
    pub jwt_audience: Option<String>,
    #[serde(default)]
    pub jwt_issuer: Option<String>,
    #[serde(default)]
    pub tls_required: bool,
}

impl SecurityConfig {
    fn default_required() -> bool {
        true
    }
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            require_authentication: true,
            api_keys_url: None,
            jwt_audience: None,
            jwt_issuer: None,
            tls_required: false,
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, Default)]
#[serde(rename_all = "camelCase")]
pub struct PluginBusConfig {
    #[serde(default)]
    pub mistral_endpoint: Option<String>,
    #[serde(default)]
    pub mistral_api_key: Option<String>,
    #[serde(default)]
    pub api_keys_endpoint: Option<String>,
    #[serde(default)]
    pub bus_channel: Option<String>,
}
