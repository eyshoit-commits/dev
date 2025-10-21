use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use bytes::Bytes;
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tokio::sync::RwLock;
use tracing::info;

use crate::error::{HelixError, HelixResult};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityInvocation {
    pub payload: Bytes,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CapabilityResponse {
    pub payload: Bytes,
    pub content_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginEvent {
    pub name: String,
    pub timestamp: DateTime<Utc>,
    pub event_type: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PluginHealth {
    pub status: String,
    pub last_heartbeat: DateTime<Utc>,
    pub details: serde_json::Value,
}

#[async_trait]
pub trait Capability: Send + Sync {
    fn name(&self) -> &str;
    async fn invoke(&self, invocation: CapabilityInvocation) -> HelixResult<CapabilityResponse>;
}

#[async_trait]
pub trait Plugin: Send + Sync {
    fn name(&self) -> &str;
    async fn register(&self, bus: &mut PluginBus) -> HelixResult<()>;
    fn capabilities(&self) -> Vec<String>;
    async fn health(&self) -> HelixResult<PluginHealth>;
}

#[derive(Clone)]
pub struct PluginBus {
    capabilities: Arc<RwLock<HashMap<String, Arc<dyn Capability>>>>,
    events: Arc<RwLock<Vec<PluginEvent>>>,
}

impl PluginBus {
    pub fn new() -> Self {
        Self {
            capabilities: Arc::new(RwLock::new(HashMap::new())),
            events: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub async fn register_capability(&self, capability: Arc<dyn Capability>) -> HelixResult<()> {
        info!(name = capability.name(), "registering capability");
        self.capabilities
            .write()
            .await
            .insert(capability.name().to_string(), capability);
        self.record_event("capability.registered").await;
        Ok(())
    }

    pub async fn invoke(
        &self,
        name: &str,
        invocation: CapabilityInvocation,
    ) -> HelixResult<CapabilityResponse> {
        let capabilities = self.capabilities.read().await;
        let capability = capabilities
            .get(name)
            .ok_or_else(|| HelixError::Plugin(format!("Capability {name} not found")))?;
        capability.invoke(invocation).await
    }

    pub async fn list(&self) -> Vec<String> {
        self.capabilities.read().await.keys().cloned().collect()
    }

    pub async fn events(&self) -> Vec<PluginEvent> {
        self.events.read().await.clone()
    }

    pub async fn record_event(&self, name: &str) {
        self.events.write().await.push(PluginEvent {
            name: name.to_string(),
            timestamp: Utc::now(),
            event_type: "event".to_string(),
        });
    }
}
