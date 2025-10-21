use crate::auth::{AuthError, AuthService, AuthenticatedUser, Scope};
use crate::config::{GooseRunConfig, Settings};
use crate::metrics::StreamEnvelope;
use crate::persistence::{HistoryStore, RunRecord};
use crate::plugin::PluginRegistry;
use crate::runner::{EngineHandle, EngineStatus};
use axum::extract::ws::{Message, WebSocket};
use axum::extract::{State, WebSocketUpgrade};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Extension, Json, Router};
use chrono::{DateTime, Utc};
use futures::StreamExt;
use schemars::schema_for;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;
use tracing::{info, warn};

#[derive(Clone)]
pub struct AppState {
    settings: Settings,
    engine: EngineHandle,
    history: HistoryStore,
    plugins: PluginRegistry,
    auth: Arc<AuthService>,
}

impl AppState {
    pub fn new(
        settings: Settings,
        engine: EngineHandle,
        history: HistoryStore,
        plugins: PluginRegistry,
    ) -> anyhow::Result<Self> {
        let auth = Arc::new(AuthService::new(&settings)?);
        Ok(Self {
            settings,
            engine,
            history,
            plugins,
            auth,
        })
    }

    pub fn settings(&self) -> &Settings {
        &self.settings
    }

    pub fn engine(&self) -> &EngineHandle {
        &self.engine
    }

    pub fn history(&self) -> &HistoryStore {
        &self.history
    }

    #[allow(dead_code)]
    pub fn plugins(&self) -> &PluginRegistry {
        &self.plugins
    }

    pub fn auth(&self) -> Arc<AuthService> {
        self.auth.clone()
    }
}

pub fn build_router(state: Arc<AppState>) -> Router {
    let websocket_path = state.settings().server.websocket_path.clone();
    Router::new()
        .route("/api/goose/run", post(start_run))
        .route("/api/goose/stop", post(stop_run))
        .route("/api/goose/status", get(status))
        .route("/api/goose/history", get(history))
        .route("/api/goose/schema", get(schema))
        .route(&websocket_path, get(stream))
        .with_state(state.clone())
        .layer(Extension(state.auth()))
}

#[derive(Debug, Deserialize)]
struct RunRequest {
    config: GooseRunConfig,
}

#[derive(Debug, Serialize)]
struct RunResponse {
    run_id: String,
    status: EngineStatus,
}

async fn start_run(
    State(state): State<Arc<AppState>>,
    AuthenticatedUser(user): AuthenticatedUser,
    Json(payload): Json<RunRequest>,
) -> Result<Json<RunResponse>, AuthError> {
    user.ensure_scope(Scope::WriteLoadTests)?;
    let run_id = state
        .engine()
        .start_run(payload.config)
        .await
        .map_err(|err| AuthError::Upstream(err.to_string()))?;
    let status = state
        .engine()
        .status()
        .await
        .map_err(|err| AuthError::Upstream(err.to_string()))?;
    Ok(Json(RunResponse { run_id, status }))
}

async fn stop_run(
    State(state): State<Arc<AppState>>,
    AuthenticatedUser(user): AuthenticatedUser,
) -> Result<Response, AuthError> {
    user.ensure_scope(Scope::WriteLoadTests)?;
    state
        .engine()
        .stop_run()
        .await
        .map_err(|err| AuthError::Upstream(err.to_string()))?;
    Ok(StatusCode::ACCEPTED.into_response())
}

#[derive(Debug, Serialize)]
struct StatusResponse {
    status: EngineStatus,
}

async fn status(
    State(state): State<Arc<AppState>>,
    AuthenticatedUser(user): AuthenticatedUser,
) -> Result<Json<StatusResponse>, AuthError> {
    user.ensure_scope(Scope::ReadLoadTests)?;
    let status = state
        .engine()
        .status()
        .await
        .map_err(|err| AuthError::Upstream(err.to_string()))?;
    Ok(Json(StatusResponse { status }))
}

#[derive(Debug, Serialize)]
struct HistoryResponse {
    runs: Vec<HistoryItem>,
}

#[derive(Debug, Serialize)]
struct HistoryItem {
    run_id: String,
    plugin_id: String,
    start_time: DateTimeWrapper,
    end_time: Option<DateTimeWrapper>,
    status: String,
}

#[derive(Debug, Serialize)]
struct SchemaResponse {
    schema: schemars::schema::RootSchema,
}

async fn history(
    State(state): State<Arc<AppState>>,
    AuthenticatedUser(user): AuthenticatedUser,
) -> Result<Json<HistoryResponse>, AuthError> {
    user.ensure_scope(Scope::ReadLoadTests)?;
    let runs = state
        .history()
        .list_runs(100)
        .map_err(|err| AuthError::Upstream(err.to_string()))?;
    let items = runs.into_iter().map(|run| HistoryItem::from(run)).collect();
    Ok(Json(HistoryResponse { runs: items }))
}

async fn schema(State(state): State<Arc<AppState>>) -> Json<SchemaResponse> {
    let schema = schema_for!(GooseRunConfig);
    Json(SchemaResponse { schema })
}

async fn stream(
    State(state): State<Arc<AppState>>,
    AuthenticatedUser(user): AuthenticatedUser,
    ws: WebSocketUpgrade,
) -> Result<Response, AuthError> {
    user.ensure_scope(Scope::ReadLoadTests)?;
    let tx = state.engine().metrics_sender();
    Ok(ws.on_upgrade(move |socket| handle_ws(socket, tx)))
}

async fn handle_ws(stream: WebSocket, sender: broadcast::Sender<StreamEnvelope>) {
    let mut rx = BroadcastStream::new(sender.subscribe());
    let (mut sink, _) = stream.split();
    while let Some(message) = rx.next().await {
        match message {
            Ok(envelope) => {
                let payload = match serde_json::to_string(&envelope) {
                    Ok(json) => json,
                    Err(err) => {
                        warn!(?err, "failed to serialize stream envelope");
                        continue;
                    }
                };
                if sink.send(Message::Text(payload)).await.is_err() {
                    break;
                }
            }
            Err(err) => {
                warn!(?err, "broadcast stream error");
                break;
            }
        }
    }
    info!("websocket stream closed");
}

impl From<RunRecord> for HistoryItem {
    fn from(record: RunRecord) -> Self {
        Self {
            run_id: record.run_id,
            plugin_id: record.plugin_id,
            start_time: DateTimeWrapper(record.start_time),
            end_time: record.end_time.map(DateTimeWrapper),
            status: record.status.as_str().to_string(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
struct DateTimeWrapper(#[serde(with = "chrono::serde::ts_seconds")] DateTime<Utc>);
