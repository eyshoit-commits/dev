use config::FileFormat;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::path::{Path, PathBuf};

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub security: SecurityConfig,
}

impl Settings {
    pub fn load() -> anyhow::Result<Self> {
        let mut builder = config::Config::builder();
        let cwd = std::env::current_dir()?;
        let default = cwd.join("config.apikeys.json");
        if default.exists() {
            builder = builder.add_source(config::File::from(default).format(FileFormat::Json));
        }
        let runtime = cwd.join("config.apikeys.runtime.json");
        if runtime.exists() {
            builder = builder.add_source(config::File::from(runtime).format(FileFormat::Json));
        }
        builder = builder.add_source(
            config::Environment::with_prefix("MAID_APIKEYS")
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
            database: DatabaseConfig::default(),
            security: SecurityConfig::default(),
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
}

impl ServerConfig {
    fn default_host() -> IpAddr {
        IpAddr::V4(Ipv4Addr::UNSPECIFIED)
    }

    fn default_port() -> u16 {
        43119
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
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DatabaseConfig {
    #[serde(default = "DatabaseConfig::default_path")]
    pub path: PathBuf,
}

impl DatabaseConfig {
    fn default_path() -> PathBuf {
        dirs::data_local_dir()
            .unwrap_or_else(|| PathBuf::from("./data"))
            .join("maid")
            .join("apikeys.sqlite")
    }
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self {
            path: Self::default_path(),
        }
    }
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SecurityConfig {
    #[serde(default = "SecurityConfig::default_secret")]
    pub jwt_secret: String,
    #[serde(default = "SecurityConfig::default_issuer")]
    pub jwt_issuer: String,
    #[serde(default = "SecurityConfig::default_audience")]
    pub jwt_audience: String,
    #[serde(default = "SecurityConfig::default_expiry")]
    pub jwt_expiry_minutes: i64,
    #[serde(default = "SecurityConfig::default_scopes")]
    pub default_scopes: Vec<String>,
    #[serde(default = "SecurityConfig::default_key_prefix")]
    pub default_api_key_prefix: String,
}

impl SecurityConfig {
    fn default_secret() -> String {
        std::env::var("MAID_APIKEYS_SECRET")
            .unwrap_or_else(|_| "change-me-super-secret".to_string())
    }

    fn default_issuer() -> String {
        "maid.apikeys".to_string()
    }

    fn default_audience() -> String {
        "maid.clients".to_string()
    }

    fn default_expiry() -> i64 {
        60
    }

    fn default_scopes() -> Vec<String> {
        vec!["read:loadtests".to_string(), "write:loadtests".to_string()]
    }

    fn default_key_prefix() -> String {
        "maid_live_".to_string()
    }
}

impl Default for SecurityConfig {
    fn default() -> Self {
        Self {
            jwt_secret: Self::default_secret(),
            jwt_issuer: Self::default_issuer(),
            jwt_audience: Self::default_audience(),
            jwt_expiry_minutes: Self::default_expiry(),
            default_scopes: Self::default_scopes(),
            default_api_key_prefix: Self::default_key_prefix(),
        }
    }
}
