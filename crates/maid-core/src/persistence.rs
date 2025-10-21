use crate::config::GooseRunConfig;
use crate::metrics::MetricSnapshot;
use anyhow::Context;
use chrono::{DateTime, Utc};
use parking_lot::Mutex;
use rusqlite::{params, Connection};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::fs;
use std::path::Path;
use std::sync::Arc;

#[derive(Clone)]
pub struct HistoryStore {
    conn: Arc<Mutex<Connection>>,
}

impl HistoryStore {
    pub fn new<P: AsRef<Path>>(path: P) -> anyhow::Result<Self> {
        if let Some(parent) = path.as_ref().parent() {
            fs::create_dir_all(parent)?;
        }
        let conn = Connection::open(path)?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS goose_runs (
                run_id TEXT PRIMARY KEY,
                plugin_id TEXT NOT NULL,
                config_json TEXT NOT NULL,
                metrics_json TEXT,
                start_time TEXT NOT NULL,
                end_time TEXT,
                status TEXT NOT NULL
            );",
        )?;
        Ok(Self {
            conn: Arc::new(Mutex::new(conn)),
        })
    }

    pub fn insert_run(&self, record: &RunRecord) -> anyhow::Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "INSERT INTO goose_runs (run_id, plugin_id, config_json, metrics_json, start_time, end_time, status)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                record.run_id,
                record.plugin_id,
                serde_json::to_string(&record.config_json)?,
                record
                    .metrics_json
                    .as_ref()
                    .map(serde_json::to_string)
                    .transpose()?,
                record.start_time.to_rfc3339(),
                record.end_time.map(|dt| dt.to_rfc3339()),
                record.status.as_str(),
            ],
        )?;
        Ok(())
    }

    pub fn update_metrics(
        &self,
        run_id: &str,
        metrics: &[MetricSnapshot],
        status: RunStatus,
        end_time: Option<DateTime<Utc>>,
    ) -> anyhow::Result<()> {
        let conn = self.conn.lock();
        conn.execute(
            "UPDATE goose_runs SET metrics_json = ?1, status = ?2, end_time = COALESCE(?3, end_time) WHERE run_id = ?4",
            params![
                serde_json::to_string(metrics)?,
                status.as_str(),
                end_time.map(|dt| dt.to_rfc3339()),
                run_id,
            ],
        )?;
        Ok(())
    }

    pub fn list_runs(&self, limit: usize) -> anyhow::Result<Vec<RunRecord>> {
        let conn = self.conn.lock();
        let mut stmt = conn.prepare(
            "SELECT run_id, plugin_id, config_json, metrics_json, start_time, end_time, status
             FROM goose_runs ORDER BY start_time DESC LIMIT ?1",
        )?;
        let iter = stmt.query_map([limit as i64], |row| {
            let config_json: String = row.get(2)?;
            let metrics_json: Option<String> = row.get(3)?;
            Ok(RunRecord {
                run_id: row.get(0)?,
                plugin_id: row.get(1)?,
                config_json: serde_json::from_str(&config_json).context("invalid config json")?,
                metrics_json: metrics_json
                    .map(|value| serde_json::from_str(&value).context("invalid metrics json"))
                    .transpose()?,
                start_time: DateTime::parse_from_rfc3339(&row.get::<_, String>(4)?)?
                    .with_timezone(&Utc),
                end_time: row
                    .get::<_, Option<String>>(5)?
                    .map(|value| {
                        DateTime::parse_from_rfc3339(&value).map(|dt| dt.with_timezone(&Utc))
                    })
                    .transpose()?,
                status: RunStatus::from_str(&row.get::<_, String>(6)?),
            })
        })?;
        let mut runs = Vec::new();
        for result in iter {
            runs.push(result?);
        }
        Ok(runs)
    }

    pub fn truncate_history(&self, max_history: usize) -> anyhow::Result<()> {
        if max_history == 0 {
            return Ok(());
        }
        let conn = self.conn.lock();
        conn.execute(
            "DELETE FROM goose_runs WHERE run_id NOT IN (
                SELECT run_id FROM goose_runs ORDER BY start_time DESC LIMIT ?1
            )",
            params![max_history as i64],
        )?;
        Ok(())
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct RunRecord {
    pub run_id: String,
    pub plugin_id: String,
    pub config_json: GooseRunConfig,
    pub metrics_json: Option<Vec<MetricSnapshot>>,
    pub start_time: DateTime<Utc>,
    pub end_time: Option<DateTime<Utc>>,
    pub status: RunStatus,
}

impl RunRecord {
    pub fn summary(&self) -> serde_json::Value {
        json!({
            "runId": self.run_id,
            "pluginId": self.plugin_id,
            "startTime": self.start_time,
            "endTime": self.end_time,
            "status": self.status.as_str(),
        })
    }
}

#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum RunStatus {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
}

impl RunStatus {
    pub fn as_str(&self) -> &'static str {
        match self {
            RunStatus::Pending => "pending",
            RunStatus::Running => "running",
            RunStatus::Completed => "completed",
            RunStatus::Failed => "failed",
            RunStatus::Cancelled => "cancelled",
        }
    }

    pub fn from_str(value: &str) -> Self {
        match value {
            "running" => RunStatus::Running,
            "completed" => RunStatus::Completed,
            "failed" => RunStatus::Failed,
            "cancelled" => RunStatus::Cancelled,
            _ => RunStatus::Pending,
        }
    }
}

pub fn ensure_report_directories<P: AsRef<Path>>(
    base: P,
    run_id: &str,
) -> anyhow::Result<std::path::PathBuf> {
    let dir = base.as_ref().join(run_id);
    fs::create_dir_all(&dir)?;
    Ok(dir)
}
