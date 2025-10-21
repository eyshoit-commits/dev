use std::{convert::TryInto, sync::Arc};

use async_trait::async_trait;
use bytes::Bytes;
use ed25519_dalek::{Signature, Signer, SigningKey};
use helix_db::{
    Capability, CapabilityInvocation, CapabilityResponse, Plugin, PluginBus, PluginHealth,
};
use reqwest::{header::CONTENT_TYPE, Client};
use serde::{Deserialize, Serialize};
use sha3::{Digest, Sha3_256};
use thiserror::Error;
use tokio::time::{timeout, Duration};
use tracing::instrument;
use url::Url;
use validator::Validate;

#[derive(Debug, Clone, Serialize, Deserialize, Validate)]
pub struct Db3AdapterConfig {
    #[validate(url)]
    pub base_url: String,
    #[serde(default)]
    pub admin_address: Option<String>,
    #[serde(default)]
    pub signing_key: Option<String>,
    #[serde(default = "default_timeout_secs")]
    pub timeout_secs: u64,
    #[serde(default = "default_name")]
    pub name: String,
}

const fn default_timeout_secs() -> u64 {
    30
}

fn default_name() -> String {
    "db3".to_string()
}

#[derive(Error, Debug)]
pub enum Db3Error {
    #[error("invalid configuration: {0}")]
    Configuration(String),
}

#[derive(Clone)]
pub struct Db3Adapter {
    config: Db3AdapterConfig,
    client: Client,
    signer: Option<SigningKey>,
    capabilities: Vec<Arc<dyn Capability>>,
}

impl Db3Adapter {
    pub fn new(config: Db3AdapterConfig) -> helix_db::HelixResult<Self> {
        config
            .validate()
            .map_err(|err| helix_db::HelixError::Configuration(err.to_string()))?;
        let signer = if let Some(secret) = &config.signing_key {
            let bytes = hex::decode(secret)
                .map_err(|err| helix_db::HelixError::Configuration(err.to_string()))?;
            let key = SigningKey::from_bytes(&bytes.try_into().map_err(|_| {
                helix_db::HelixError::Configuration("invalid signing key length".into())
            })?);
            Some(key)
        } else {
            None
        };
        let client = Client::builder()
            .user_agent("helix-db3-adapter/1.0")
            .build()
            .map_err(|err| helix_db::HelixError::Configuration(err.to_string()))?;
        let mut adapter = Self {
            config,
            client,
            signer,
            capabilities: Vec::new(),
        };
        adapter.initialize_capabilities();
        Ok(adapter)
    }

    fn initialize_capabilities(&mut self) {
        let client = self.client.clone();
        let config = self.config.clone();
        let signer = self.signer.clone();
        self.capabilities = vec![
            Arc::new(Db3Capability::new(
                format!("{}::add_doc", config.name),
                client.clone(),
                config.clone(),
                signer.clone(),
                "/db3/archive",
                CapabilityKind::Write,
            )),
            Arc::new(Db3Capability::new(
                format!("{}::query", config.name),
                client.clone(),
                config.clone(),
                signer.clone(),
                "/db3/query",
                CapabilityKind::Read,
            )),
            Arc::new(Db3Capability::new(
                format!("{}::archive_status", config.name),
                client,
                config,
                signer,
                "/db3/sync/status",
                CapabilityKind::Read,
            )),
        ];
    }
}

#[async_trait]
impl Plugin for Db3Adapter {
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
                "wallet": self.config.admin_address,
            }),
        })
    }
}

#[derive(Clone)]
struct Db3Capability {
    name: String,
    client: Client,
    config: Db3AdapterConfig,
    signer: Option<SigningKey>,
    endpoint: &'static str,
    kind: CapabilityKind,
}

#[derive(Clone, Copy)]
enum CapabilityKind {
    Write,
    Read,
}

impl Db3Capability {
    fn new(
        name: String,
        client: Client,
        config: Db3AdapterConfig,
        signer: Option<SigningKey>,
        endpoint: &'static str,
        kind: CapabilityKind,
    ) -> Self {
        Self {
            name,
            client,
            config,
            signer,
            endpoint,
            kind,
        }
    }
}

#[async_trait]
impl Capability for Db3Capability {
    fn name(&self) -> &str {
        &self.name
    }

    #[instrument(skip(self, invocation))]
    async fn invoke(
        &self,
        invocation: CapabilityInvocation,
    ) -> helix_db::HelixResult<CapabilityResponse> {
        let url = Url::parse(&self.config.base_url)
            .and_then(|base| base.join(self.endpoint))
            .map_err(|err| helix_db::HelixError::Plugin(err.to_string()))?;
        let mut request = self
            .client
            .post(url)
            .header(CONTENT_TYPE, "application/json")
            .body(invocation.payload.clone());
        if matches!(self.kind, CapabilityKind::Write) {
            if let Some(signer) = &self.signer {
                let signature = sign_payload(signer, &invocation.payload);
                request = request.header("X-DB3-Signature", signature);
            }
        }
        if let Some(address) = &self.config.admin_address {
            request = request.header("X-DB3-Admin", address);
        }
        let response = timeout(
            Duration::from_secs(self.config.timeout_secs),
            request.send(),
        )
        .await
        .map_err(|err| helix_db::HelixError::Plugin(err.to_string()))??;
        if !response.status().is_success() {
            return Err(helix_db::HelixError::Plugin(format!(
                "DB3 request failed with status {}",
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

fn sign_payload(signer: &SigningKey, payload: &Bytes) -> String {
    let mut hasher = Sha3_256::new();
    hasher.update(payload);
    let digest = hasher.finalize();
    let signature: Signature = signer.sign(&digest);
    hex::encode(signature.to_bytes())
}
