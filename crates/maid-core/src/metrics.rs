use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct MetricSnapshot {
    pub timestamp: DateTime<Utc>,
    pub cpu_usage: f32,
    pub ram_usage: f32,
    pub throughput_rps: f32,
    pub error_rate: f32,
    pub status_codes: BTreeMap<String, u64>,
    pub latency_p50_ms: f32,
    pub latency_p90_ms: f32,
    pub latency_p95_ms: f32,
    pub latency_p99_ms: f32,
    pub latency_p999_ms: f32,
    pub network_in_kbps: f32,
    pub network_out_kbps: f32,
    pub phase: EnginePhase,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LogEvent {
    pub timestamp: DateTime<Utc>,
    pub level: LogLevel,
    pub message: String,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub enum StreamEnvelope {
    Metrics(MetricSnapshot),
    Log(LogEvent),
    Status(StatusEnvelope),
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct StatusEnvelope {
    pub run_id: String,
    pub phase: EnginePhase,
    pub active_users: u32,
    pub duration_seconds: u64,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "kebab-case")]
pub enum EnginePhase {
    Idle,
    Increase,
    Maintain,
    Decrease,
    Shutdown,
    Completed,
}

#[derive(Clone, Debug, Serialize, Deserialize, JsonSchema, PartialEq, Eq)]
#[serde(rename_all = "lowercase")]
pub enum LogLevel {
    Trace,
    Debug,
    Info,
    Warn,
    Error,
}
