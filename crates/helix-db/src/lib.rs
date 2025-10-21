pub mod auth;
pub mod config;
pub mod document;
pub mod error;
pub mod graph;
pub mod helixql;
pub mod metrics;
pub mod plugin;
pub mod server;
pub mod storage;
pub mod telemetry;
pub mod vector;

pub use auth::{ApiKey, AuthManager};
pub use config::{HelixConfig, HelixFeatureFlags};
pub use error::{HelixError, HelixResult};
pub use graph::{EdgeRecord, GraphEngine, NodeRecord};
pub use helixql::{HelixQlEngine, HelixQlRequest, HelixQuery, HelixQueryLiteral};
pub use metrics::MetricsService;
pub use plugin::{
    Capability, CapabilityInvocation, CapabilityResponse, Plugin, PluginBus, PluginEvent,
    PluginHealth,
};
pub use server::HelixServer;
pub use storage::StorageEngine;
pub use telemetry::{TelemetryEvent, TelemetryHub};
pub use vector::{SimilarityMetric, VectorIndex, VectorRecord};
