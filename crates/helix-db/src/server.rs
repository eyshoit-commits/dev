use std::{collections::HashMap, iter::FromIterator, net::SocketAddr, sync::Arc};

use axum::{
    extract::State,
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use serde::{Deserialize, Serialize};
use tokio::net::TcpListener;
use tower_http::trace::TraceLayer;
use tracing::{info, instrument};
use uuid::Uuid;

use crate::{
    auth::AuthManager,
    config::{HelixConfig, PluginConfig},
    document::DocumentRecord,
    error::{HelixError, HelixResult},
    helixql::{HelixQlEngine, HelixQlRequest},
    metrics::MetricsService,
    plugin::PluginBus,
    storage::StorageEngine,
    telemetry::{TelemetryEvent, TelemetryHub},
    vector::VectorIndex,
};

#[derive(Clone)]
pub struct HelixServer {
    config: HelixConfig,
    auth: AuthManager,
    storage: Arc<StorageEngine>,
    telemetry: TelemetryHub,
    metrics: MetricsService,
    plugin_bus: PluginBus,
    vector: VectorIndex,
    ql_engine: HelixQlEngine,
}

#[derive(Clone)]
struct ApiState {
    auth: AuthManager,
    storage: Arc<StorageEngine>,
    telemetry: TelemetryHub,
    metrics: MetricsService,
    plugin_bus: PluginBus,
    vector: VectorIndex,
    ql_engine: HelixQlEngine,
}

impl HelixServer {
    pub fn new(config: HelixConfig) -> HelixResult<Self> {
        let storage = Arc::new(StorageEngine::new(&config)?);
        let telemetry = TelemetryHub::new(1024);
        let metrics = MetricsService::initialize(&config.telemetry)?;
        let plugin_bus = PluginBus::new();
        let vector = VectorIndex::new(storage.clone());
        let ql_engine = HelixQlEngine::new(vector.clone(), telemetry.clone());
        let auth = AuthManager::from_config(&config)?;
        Ok(Self {
            config,
            auth,
            storage,
            telemetry,
            metrics,
            plugin_bus,
            vector,
            ql_engine,
        })
    }

    #[instrument(skip(self))]
    pub async fn run(self) -> HelixResult<()> {
        let addr: SocketAddr = self
            .config
            .rest
            .bind_addr
            .parse()
            .map_err(|err| HelixError::Configuration(err.to_string()))?;
        let state = ApiState {
            auth: self.auth.clone(),
            storage: self.storage.clone(),
            telemetry: self.telemetry.clone(),
            metrics: self.metrics.clone(),
            plugin_bus: self.plugin_bus.clone(),
            vector: self.vector.clone(),
            ql_engine: self.ql_engine,
        };
        let app = Router::new()
            .route("/api/braindb/query", post(query_handler))
            .route("/api/braindb/documents", post(documents_handler))
            .route("/api/braindb/metrics", get(metrics_handler))
            .route(
                "/api/braindb/plugins/register",
                post(plugin_register_handler),
            )
            .with_state(state)
            .layer(TraceLayer::new_for_http());

        info!(?addr, "HelixDB listening");
        let listener = TcpListener::bind(addr)
            .await
            .map_err(|err| HelixError::Internal(err.to_string()))?;
        axum::serve(listener, app)
            .await
            .map_err(|err| HelixError::Internal(err.to_string()))
    }

    pub fn plugin_bus(&self) -> PluginBus {
        self.plugin_bus.clone()
    }

    pub fn storage(&self) -> Arc<StorageEngine> {
        self.storage.clone()
    }

    pub fn vector(&self) -> VectorIndex {
        self.vector.clone()
    }
}

#[derive(Debug, Deserialize)]
struct DocumentPayload {
    pub id: Option<String>,
    pub body: serde_json::Value,
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
    #[serde(default)]
    pub embedding: Option<Vec<f32>>,
}

#[derive(Debug, Deserialize)]
struct PluginRegistrationRequest {
    pub name: String,
    pub base_url: String,
    #[serde(default)]
    pub api_key: Option<String>,
    #[serde(default)]
    pub jwt: Option<String>,
    #[serde(default)]
    pub capabilities: Vec<String>,
    #[serde(default)]
    pub feature_flags: Vec<String>,
}

async fn query_handler(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(request): Json<HelixQlRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    authorize(&state, &headers, "query.read").await?;
    let response = state.ql_engine.execute(request).await?;
    Ok(Json(response))
}

async fn documents_handler(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(payload): Json<DocumentPayload>,
) -> Result<Json<DocumentRecord>, ApiError> {
    authorize(&state, &headers, "documents.write").await?;
    let mut document = if let Some(id) = payload.id {
        DocumentRecord::new(id, payload.body)
    } else {
        DocumentRecord::new(Uuid::new_v4().to_string(), payload.body)
    };
    if let Some(vector) = payload.embedding {
        document.embedding = Some(vector.clone());
        let metadata = serde_json::Map::from_iter(
            payload.metadata.iter().map(|(k, v)| (k.clone(), v.clone())),
        );
        state
            .vector
            .upsert_vector(&crate::vector::VectorRecord::new(
                document.id.clone(),
                vector,
                serde_json::Value::Object(metadata),
            ))?;
    }
    document.metadata = payload.metadata.clone();
    state.storage.insert_document(&document)?;
    state.telemetry.publish(TelemetryEvent::DocumentInserted {
        id: document.id.clone(),
        timestamp: chrono::Utc::now(),
    })?;
    Ok(Json(document))
}

async fn metrics_handler(State(state): State<ApiState>) -> Result<Response, ApiError> {
    authorize(&state, &HeaderMap::new(), "metrics.read")
        .await
        .ok();
    let body = state.metrics.render();
    Ok((StatusCode::OK, body).into_response())
}

async fn plugin_register_handler(
    State(state): State<ApiState>,
    headers: HeaderMap,
    Json(request): Json<PluginRegistrationRequest>,
) -> Result<Json<serde_json::Value>, ApiError> {
    authorize(&state, &headers, "plugins.register").await?;
    state.plugin_bus.record_event("plugin.registered").await;
    state.telemetry.publish(TelemetryEvent::PluginRegistered {
        name: request.name.clone(),
        timestamp: chrono::Utc::now(),
    })?;
    let mut extra = HashMap::new();
    extra.insert(
        "capabilities".to_string(),
        serde_json::Value::Array(
            request
                .capabilities
                .iter()
                .cloned()
                .map(serde_json::Value::String)
                .collect(),
        ),
    );
    extra.insert(
        "feature_flags".to_string(),
        serde_json::Value::Array(
            request
                .feature_flags
                .iter()
                .cloned()
                .map(serde_json::Value::String)
                .collect(),
        ),
    );
    let config = PluginConfig {
        base_url: request.base_url,
        api_key: request.api_key,
        jwt: request.jwt,
        extra,
    };
    info!(plugin = request.name, "registered external plugin");
    Ok(Json(serde_json::json!({
        "status": "registered",
        "config": config,
    })))
}

async fn authorize(state: &ApiState, headers: &HeaderMap, scope: &str) -> Result<(), ApiError> {
    if let Some(key) = headers.get("x-api-key").and_then(|v| v.to_str().ok()) {
        state
            .auth
            .authenticate_api_key(key, scope)
            .await
            .map_err(ApiError::from)
    } else if let Some(auth_header) = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
    {
        if let Some(token) = auth_header.strip_prefix("Bearer ") {
            state
                .auth
                .authenticate_jwt(token, scope)
                .await
                .map(|_| ())
                .map_err(ApiError::from)
        } else {
            Err(ApiError::from(HelixError::Authentication))
        }
    } else if scope == "metrics.read" {
        Ok(())
    } else {
        Err(ApiError::from(HelixError::Authentication))
    }
}

#[derive(Debug, Serialize)]
struct ApiErrorBody {
    error: String,
}

struct ApiError(HelixError);

impl From<HelixError> for ApiError {
    fn from(value: HelixError) -> Self {
        Self(value)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = self.0.status_code();
        let body = Json(ApiErrorBody {
            error: self.0.to_string(),
        });
        (status, body).into_response()
    }
}
