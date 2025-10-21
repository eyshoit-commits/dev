use crate::config::{GooseRunConfig, ReportFormat, Settings};
use crate::metrics::{
    EnginePhase, LogEvent, LogLevel, MetricSnapshot, StatusEnvelope, StreamEnvelope,
};
use crate::persistence::{ensure_report_directories, HistoryStore, RunRecord, RunStatus};
use chrono::{DateTime, Utc};
use rand::Rng;
use serde::Serialize;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{broadcast, Mutex, MutexGuard};
use tokio::task::JoinHandle;
use tokio::{select, time};
use tracing::{error, info, warn};
use uuid::Uuid;
use validator::Validate;

#[derive(Clone)]
pub struct EngineHandle {
    inner: Arc<EngineInner>,
}

struct EngineInner {
    settings: Settings,
    history: HistoryStore,
    sender: broadcast::Sender<StreamEnvelope>,
    current: Mutex<Option<Arc<RunController>>>,
}

struct RunController {
    run_id: String,
    config: GooseRunConfig,
    start_time: DateTime<Utc>,
    metrics: Arc<Mutex<Vec<MetricSnapshot>>>,
    phase: Arc<Mutex<EnginePhase>>,
    stop: tokio::sync::watch::Sender<bool>,
    handle: JoinHandle<()>,
}

#[derive(Debug, Serialize)]
pub struct EngineStatus {
    pub active: bool,
    pub run_id: Option<String>,
    pub phase: EnginePhase,
    pub started_at: Option<DateTime<Utc>>,
}

impl EngineHandle {
    pub fn new(settings: Settings, history: HistoryStore) -> Self {
        let (sender, _) = broadcast::channel(256);
        Self {
            inner: Arc::new(EngineInner {
                settings,
                history,
                sender,
                current: Mutex::new(None),
            }),
        }
    }

    pub fn metrics_sender(&self) -> broadcast::Sender<StreamEnvelope> {
        self.inner.sender.clone()
    }

    async fn current(&self) -> MutexGuard<'_, Option<Arc<RunController>>> {
        self.inner.current.lock().await
    }

    pub async fn start_run(&self, mut config: GooseRunConfig) -> anyhow::Result<String> {
        config.validate()?;
        let mut guard = self.current().await;
        if guard.is_some() {
            anyhow::bail!("load test already running");
        }
        let run_id = Uuid::new_v4().to_string();
        let start_time = Utc::now();
        let record = RunRecord {
            run_id: run_id.clone(),
            plugin_id: "maid.core".to_string(),
            config_json: config.clone(),
            metrics_json: None,
            start_time,
            end_time: None,
            status: RunStatus::Running,
        };
        self.inner.history.insert_run(&record)?;

        let (stop_tx, stop_rx) = tokio::sync::watch::channel(false);
        let metrics_vec = Arc::new(Mutex::new(Vec::new()));
        let phase_state = Arc::new(Mutex::new(EnginePhase::Increase));
        let controller = Arc::new(RunController {
            run_id: run_id.clone(),
            config: config.clone(),
            start_time,
            metrics: metrics_vec.clone(),
            phase: phase_state.clone(),
            stop: stop_tx,
            handle: spawn_run_task(
                run_id.clone(),
                config,
                start_time,
                metrics_vec,
                phase_state,
                self.inner.sender.clone(),
                self.inner.history.clone(),
                self.inner.settings.clone(),
                stop_rx,
            ),
        });
        *guard = Some(controller);
        Ok(run_id)
    }

    pub async fn stop_run(&self) -> anyhow::Result<()> {
        let controller = {
            let mut guard = self.current().await;
            guard.take()
        };
        if let Some(run) = controller {
            if run.stop.send(true).is_err() {
                warn!(run_id = %run.run_id, "run already stopped");
            }
            run.handle
                .await
                .map_err(|err| anyhow::anyhow!("join error: {err}"))?;
        }
        Ok(())
    }

    pub async fn status(&self) -> anyhow::Result<EngineStatus> {
        let mut guard = self.current().await;
        if let Some(run) = guard.as_ref() {
            if run.handle.is_finished() {
                let phase = run.phase.lock().await.clone();
                guard.take();
                return Ok(EngineStatus {
                    active: false,
                    run_id: Some(run.run_id.clone()),
                    phase,
                    started_at: Some(run.start_time),
                });
            }
            let phase = run.phase.lock().await.clone();
            Ok(EngineStatus {
                active: true,
                run_id: Some(run.run_id.clone()),
                phase,
                started_at: Some(run.start_time),
            })
        } else {
            Ok(EngineStatus {
                active: false,
                run_id: None,
                phase: EnginePhase::Idle,
                started_at: None,
            })
        }
    }

    pub async fn graceful_shutdown(&self) {
        if let Err(err) = self.stop_run().await {
            error!(?err, "failed to stop run during shutdown");
        }
    }
}

fn spawn_run_task(
    run_id: String,
    config: GooseRunConfig,
    start_time: DateTime<Utc>,
    metrics: Arc<Mutex<Vec<MetricSnapshot>>>,
    phase_state: Arc<Mutex<EnginePhase>>,
    sender: broadcast::Sender<StreamEnvelope>,
    history: HistoryStore,
    settings: Settings,
    mut stop_rx: tokio::sync::watch::Receiver<bool>,
) -> JoinHandle<()> {
    tokio::spawn(async move {
        info!(%run_id, "starting simulated load run");
        let mut interval = time::interval(Duration::from_secs(1));
        let mut elapsed = 0u64;
        let mut rng = rand::thread_rng();
        let mut snapshots = Vec::new();
        let total_duration = config.duration.as_secs();
        let mut current_phase = EnginePhase::Increase;
        loop {
            select! {
                _ = interval.tick() => {
                    elapsed += 1;
                    let normalized = (elapsed as f32) / total_duration.max(1) as f32;
                    current_phase = match normalized {
                        x if x < 0.25 => EnginePhase::Increase,
                        x if x < 0.75 => EnginePhase::Maintain,
                        x if x < 0.95 => EnginePhase::Decrease,
                        _ => EnginePhase::Shutdown,
                    };
                    {
                        let mut guard = phase_state.lock().await;
                        *guard = current_phase.clone();
                    }
                    let snapshot = MetricSnapshot {
                        timestamp: Utc::now(),
                        cpu_usage: (30.0 + rng.gen::<f32>() * 40.0).min(100.0),
                        ram_usage: (20.0 + rng.gen::<f32>() * 60.0).min(100.0),
                        throughput_rps: rng.gen_range(50.0..500.0),
                        error_rate: rng.gen_range(0.0..5.0),
                        status_codes: default_status_distribution(),
                        latency_p50_ms: rng.gen_range(50.0..120.0),
                        latency_p90_ms: rng.gen_range(120.0..250.0),
                        latency_p95_ms: rng.gen_range(150.0..300.0),
                        latency_p99_ms: rng.gen_range(200.0..450.0),
                        latency_p999_ms: rng.gen_range(350.0..600.0),
                        network_in_kbps: rng.gen_range(1000.0..5000.0),
                        network_out_kbps: rng.gen_range(1500.0..6000.0),
                        phase: current_phase.clone(),
                    };
                    if let Err(err) = sender.send(StreamEnvelope::Metrics(snapshot.clone())) {
                        warn!(?err, "no metrics subscribers");
                    }
                    if let Err(err) = sender.send(StreamEnvelope::Status(StatusEnvelope {
                        run_id: run_id.clone(),
                        phase: current_phase.clone(),
                        active_users: config.users,
                        duration_seconds: elapsed,
                    })) {
                        warn!(?err, "no status subscribers");
                    }
                    snapshots.push(snapshot);
                    if elapsed % 5 == 0 {
                        let log = LogEvent {
                            timestamp: Utc::now(),
                            level: LogLevel::Info,
                            message: format!("run {} is healthy at {} seconds", run_id, elapsed),
                        };
                        let _ = sender.send(StreamEnvelope::Log(log));
                    }
                    if elapsed >= total_duration {
                        break;
                    }
                }
                _ = stop_rx.changed() => {
                    if *stop_rx.borrow() {
                        current_phase = EnginePhase::Shutdown;
                        let mut guard = phase_state.lock().await;
                        *guard = current_phase.clone();
                        break;
                    }
                }
            }
        }

        let phase = if elapsed >= total_duration {
            EnginePhase::Completed
        } else {
            EnginePhase::Shutdown
        };

        {
            let mut guard = phase_state.lock().await;
            *guard = phase.clone();
        }

        let run_status = if phase == EnginePhase::Completed {
            RunStatus::Completed
        } else {
            RunStatus::Cancelled
        };

        if let Err(err) =
            history.update_metrics(&run_id, &snapshots, run_status.clone(), Some(Utc::now()))
        {
            error!(?err, "failed to persist metrics");
        }

        if let Err(err) = generate_reports(&settings, &run_id, &snapshots, &config.reports.formats)
        {
            error!(?err, "failed to generate reports");
        }

        if let Err(err) = sender.send(StreamEnvelope::Status(StatusEnvelope {
            run_id: run_id.clone(),
            phase,
            active_users: 0,
            duration_seconds: elapsed,
        })) {
            warn!(?err, "status broadcast failed");
        }

        info!(%run_id, "load run finished");
    })
}

fn default_status_distribution() -> std::collections::BTreeMap<String, u64> {
    let mut map = std::collections::BTreeMap::new();
    map.insert("200".to_string(), 950);
    map.insert("400".to_string(), 20);
    map.insert("500".to_string(), 5);
    map
}

fn generate_reports(
    settings: &Settings,
    run_id: &str,
    snapshots: &[MetricSnapshot],
    formats: &[ReportFormat],
) -> anyhow::Result<()> {
    let dir = ensure_report_directories(&settings.persistence.report_dir, run_id)?;
    for format in formats {
        match format {
            ReportFormat::Json => {
                let path = dir.join("report.json");
                std::fs::write(&path, serde_json::to_string_pretty(&snapshots)?)?;
            }
            ReportFormat::Csv => {
                let path = dir.join("report.csv");
                let mut writer = csv::Writer::from_path(path)?;
                for snapshot in snapshots {
                    writer.serialize(snapshot)?;
                }
                writer.flush()?;
            }
            ReportFormat::Html => {
                let path = dir.join("report.html");
                std::fs::write(&path, render_html_report(run_id, snapshots)?)?;
            }
        }
    }
    Ok(())
}

fn render_html_report(run_id: &str, snapshots: &[MetricSnapshot]) -> anyhow::Result<String> {
    let template = include_str!("../../templates/report.html");
    let body = serde_json::to_string(&snapshots)?;
    Ok(template
        .replace("{{run_id}}", run_id)
        .replace("{{metrics}}", &body)
        .replace("{{generated}}", &Utc::now().to_rfc3339()))
}
