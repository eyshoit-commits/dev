use std::thread;

use chrono::{DateTime, Utc};
use flume::{Receiver, Sender};
use futures::executor;
use serde::{Deserialize, Serialize};
use tracing::info;

use crate::error::HelixResult;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum TelemetryEvent {
    QueryExecuted {
        query: String,
        latency_ms: f64,
        timestamp: DateTime<Utc>,
    },
    PluginRegistered {
        name: String,
        timestamp: DateTime<Utc>,
    },
    DocumentInserted {
        id: String,
        timestamp: DateTime<Utc>,
    },
    VectorSearch {
        metric: String,
        top_k: usize,
        latency_ms: f64,
        timestamp: DateTime<Utc>,
    },
}

#[derive(Clone)]
pub struct TelemetryHub {
    tx: Sender<TelemetryEvent>,
}

impl TelemetryHub {
    pub fn new(buffer: usize) -> Self {
        let (tx, rx) = flume::bounded(buffer);
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            handle.spawn(Self::drain(rx));
        } else {
            thread::spawn(|| {
                executor::block_on(async {
                    Self::drain(rx).await;
                });
            });
        }
        Self { tx }
    }

    pub fn publish(&self, event: TelemetryEvent) -> HelixResult<()> {
        self.tx
            .send(event)
            .map_err(|err| crate::error::HelixError::Telemetry(err.to_string()))
    }

    async fn drain(rx: Receiver<TelemetryEvent>) {
        while let Ok(event) = rx.recv_async().await {
            info!(event = ?event, "telemetry");
        }
    }
}
