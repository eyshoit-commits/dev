use metrics_exporter_prometheus::{PrometheusBuilder, PrometheusHandle};
use once_cell::sync::OnceCell;
use tracing::info;

use crate::{
    config::TelemetryConfig,
    error::{HelixError, HelixResult},
};

static RECORDER: OnceCell<PrometheusHandle> = OnceCell::new();

#[derive(Clone)]
pub struct MetricsService {
    handle: PrometheusHandle,
}

impl MetricsService {
    pub fn initialize(config: &TelemetryConfig) -> HelixResult<Self> {
        if let Some(handle) = RECORDER.get() {
            return Ok(Self {
                handle: handle.clone(),
            });
        }
        let mut builder = PrometheusBuilder::new();
        if let Some(listener) = &config.prometheus_endpoint {
            builder = builder.with_http_listener(
                listener
                    .parse()
                    .map_err(|err| HelixError::Configuration(err.to_string()))?,
            );
            info!(endpoint = listener, "Starting Prometheus exporter");
        }
        let handle = builder
            .install_recorder()
            .map_err(|err| HelixError::Telemetry(err.to_string()))?;
        let _ = RECORDER.set(handle.clone());
        Ok(Self { handle })
    }

    pub fn render(&self) -> String {
        self.handle.render()
    }
}
