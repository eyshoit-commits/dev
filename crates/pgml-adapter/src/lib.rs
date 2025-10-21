use std::sync::Arc;

use async_trait::async_trait;
use bytes::Bytes;
use helix_db::{
    Capability, CapabilityInvocation, CapabilityResponse, Plugin, PluginBus, PluginHealth,
};
use reqwest::{header::CONTENT_TYPE, Client};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tokio::time::{timeout, Duration};
use tracing::instrument;
use validator::Validate;

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct PgmlAdapterConfig {
    #[validate(url)]
    pub base_url: String,
    pub api_key: Option<String>,
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u64,
    #[serde(default = "default_name")]
    pub name: String,
}

const fn default_timeout_secs() -> u64 {
    30
}

fn default_name() -> String {
    "pgml".to_string()
}

#[derive(Error, Debug)]
pub enum PgmlError {
    #[error("http error: {0}")]
    Http(String),
    #[error("validation error: {0}")]
    Validation(String),
}

#[derive(Clone)]
pub struct PgmlAdapter {
    config: PgmlAdapterConfig,
    client: Client,
    capabilities: Vec<Arc<dyn Capability>>,
}

impl PgmlAdapter {
    pub fn new(config: PgmlAdapterConfig) -> anyhow::Result<Self> {
        config
            .validate()
            .map_err(|err| PgmlError::Validation(err.to_string()))?;
        let client = Client::builder()
            .user_agent("helix-pgml-adapter/1.0")
            .build()
            .map_err(|err| PgmlError::Http(err.to_string()))?;
        let adapter = Self {
            client,
            capabilities: Vec::new(),
            config,
        };
        Ok(adapter.with_default_capabilities())
    }

    fn with_default_capabilities(mut self) -> Self {
        let client = self.client.clone();
        let config = self.config.clone();
        self.capabilities = vec![
            Arc::new(HttpCapability::new(
                format!("{}::train", config.name),
                client.clone(),
                config.clone(),
                "/pgml/train",
            )),
            Arc::new(HttpCapability::new(
                format!("{}::predict", config.name),
                client.clone(),
                config.clone(),
                "/pgml/predict",
            )),
            Arc::new(HttpCapability::new(
                format!("{}::embed", config.name),
                client.clone(),
                config.clone(),
                "/pgml/embed",
            )),
            Arc::new(HttpCapability::new(
                format!("{}::transform", config.name),
                client,
                config,
                "/pgml/transform",
            )),
        ];
        self
    }
}

#[async_trait]
impl Plugin for PgmlAdapter {
    fn name(&self) -> &str {
        &self.config.name
    }

    async fn register(&self, bus: &mut PluginBus) -> helix_db::HelixResult<()> {
        for capability in &self.capabilities {
            bus.register_capability(capability.clone()).await?;
        }
        Ok(())
    }

    fn capabilities(&self) -> Vec<String> {
        self.capabilities
            .iter()
            .map(|cap| cap.name().to_string())
            .collect()
    }

    async fn health(&self) -> helix_db::HelixResult<PluginHealth> {
        Ok(PluginHealth {
            status: "ok".to_string(),
            last_heartbeat: chrono::Utc::now(),
            details: serde_json::json!({
                "base_url": self.config.base_url,
                "capabilities": self.capabilities(),
            }),
        })
    }
}

#[derive(Clone)]
struct HttpCapability {
    name: String,
    client: Client,
    config: PgmlAdapterConfig,
    endpoint: &'static str,
}

impl HttpCapability {
    fn new(
        name: String,
        client: Client,
        config: PgmlAdapterConfig,
        endpoint: &'static str,
    ) -> Self {
        Self {
            name,
            client,
            config,
            endpoint,
        }
    }
}

#[async_trait]
impl Capability for HttpCapability {
    fn name(&self) -> &str {
        &self.name
    }

    #[instrument(skip(self, invocation))]
    async fn invoke(
        &self,
        invocation: CapabilityInvocation,
    ) -> helix_db::HelixResult<CapabilityResponse> {
        let url = format!("{}{}", self.config.base_url, self.endpoint);
        let request = self
            .client
            .post(&url)
            .header(CONTENT_TYPE, "application/json")
            .body(invocation.payload.clone());
        let request = if let Some(key) = &self.config.api_key {
            request.header("X-API-Key", key)
        } else {
            request
        };
        let timeout_duration = Duration::from_secs(self.config.timeout_secs);
        let response = timeout(timeout_duration, request.send())
            .await
            .map_err(|err| helix_db::HelixError::Plugin(err.to_string()))??;
        if !response.status().is_success() {
            return Err(helix_db::HelixError::Plugin(format!(
                "pgML request failed with status {}",
                response.status()
            )));
        }
        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("application/json")
            .to_string();
        let bytes = response
            .bytes()
            .await
            .map_err(|err| helix_db::HelixError::Plugin(err.to_string()))?;
        Ok(CapabilityResponse {
            payload: Bytes::from(bytes.to_vec()),
            content_type,
        })
    }
}
