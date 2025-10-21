use crate::config::Settings;
use crate::db::{ApiKeyRecord, AuditEvent, Database, NewUser, User};
use crate::security::{merge_scopes, verify_password, JwtSigner};
use axum::extract::{Path, State};
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use axum::Extension;
use axum::{Json, Router};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error;
use tracing::warn;
use validator::Validate;

#[derive(Clone)]
pub struct AppState {
    settings: Settings,
    db: Database,
    signer: JwtSigner,
}

impl AppState {
    pub fn new(settings: Settings, db: Database) -> Self {
        let signer = JwtSigner::new(&settings.security);
        Self {
            settings,
            db,
            signer,
        }
    }

    pub fn db(&self) -> &Database {
        &self.db
    }

    pub fn signer(&self) -> &JwtSigner {
        &self.signer
    }

    pub fn settings(&self) -> &Settings {
        &self.settings
    }
}

pub fn build_router(state: Arc<AppState>) -> Router {
    Router::new()
        .route("/api/auth/register", post(register))
        .route("/api/auth/login", post(login))
        .route("/api/auth/logout", post(logout))
        .route("/api/auth/me", get(me))
        .route("/api/auth/introspect", post(introspect))
        .route("/api/apikeys", post(create_api_key).get(list_api_keys))
        .route("/api/apikeys/:id", delete(delete_api_key))
        .route("/api/apikeys/:id/rotate", post(rotate_api_key))
        .with_state(state.clone())
        .layer(axum::Extension(state))
}

#[derive(Debug, Deserialize, Validate)]
struct RegisterRequest {
    #[validate(email)]
    email: String,
    #[validate(length(min = 12))]
    password: String,
    #[serde(default)]
    display_name: Option<String>,
    #[serde(default)]
    scopes: Vec<String>,
    #[serde(default)]
    roles: Vec<String>,
}

#[derive(Debug, Serialize)]
struct UserResponse {
    id: String,
    email: String,
    display_name: Option<String>,
    scopes: Vec<String>,
    roles: Vec<String>,
    created_at: DateTime<Utc>,
    updated_at: DateTime<Utc>,
}

async fn register(
    State(state): State<Arc<AppState>>,
    Json(request): Json<RegisterRequest>,
) -> Result<Json<UserResponse>, ApiError> {
    request.validate()?;
    if state.db().find_user_by_email(&request.email)?.is_some() {
        return Err(ApiError::Conflict("user already exists".into()));
    }
    let scopes = merge_scopes(&state.settings().security.default_scopes, &request.scopes);
    let roles = if request.roles.is_empty() {
        vec!["admin".to_string()]
    } else {
        request.roles.clone()
    };
    let user = state.db().create_user(NewUser {
        email: &request.email,
        password: &request.password,
        display_name: request.display_name.as_deref(),
        scopes: &scopes,
        roles: &roles,
    })?;
    state.db().audit(AuditEvent {
        user_id: Some(&user.id),
        event_type: "user.register",
        details: Some(serde_json::json!({ "email": user.email })),
    })?;
    Ok(Json(UserResponse::from(user)))
}

#[derive(Debug, Deserialize, Validate)]
struct LoginRequest {
    #[validate(email)]
    email: String,
    #[validate(length(min = 12))]
    password: String,
}

#[derive(Debug, Serialize)]
struct LoginResponse {
    token: String,
    expires_at: DateTime<Utc>,
    scopes: Vec<String>,
    user: UserResponse,
}

async fn login(
    State(state): State<Arc<AppState>>,
    Json(request): Json<LoginRequest>,
) -> Result<Json<LoginResponse>, ApiError> {
    request.validate()?;
    let user = state
        .db()
        .find_user_by_email(&request.email)?
        .ok_or_else(|| ApiError::Unauthorized("invalid credentials".into()))?;
    if !user.active {
        return Err(ApiError::Unauthorized("user inactive".into()));
    }
    if !verify_password(&user.password_hash, &request.password)? {
        return Err(ApiError::Unauthorized("invalid credentials".into()));
    }
    let scopes = merge_scopes(&state.settings().security.default_scopes, &user.scopes);
    let token = state.signer().issue(&user.id, &user.email, &scopes)?;
    let expires_at =
        Utc::now() + chrono::Duration::minutes(state.settings().security.jwt_expiry_minutes);
    state.db().audit(AuditEvent {
        user_id: Some(&user.id),
        event_type: "auth.login",
        details: None,
    })?;
    Ok(Json(LoginResponse {
        token,
        expires_at,
        scopes,
        user: UserResponse::from(user),
    }))
}

async fn logout(
    State(state): State<Arc<AppState>>,
    AuthenticatedAccount(account): AuthenticatedAccount,
) -> Result<Response, ApiError> {
    state.db().audit(AuditEvent {
        user_id: Some(&account.user.id),
        event_type: "auth.logout",
        details: None,
    })?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

async fn me(
    State(state): State<Arc<AppState>>,
    AuthenticatedAccount(account): AuthenticatedAccount,
) -> Result<Json<UserResponse>, ApiError> {
    state.db().audit(AuditEvent {
        user_id: Some(&account.user.id),
        event_type: "auth.me",
        details: None,
    })?;
    Ok(Json(UserResponse::from(account.user)))
}

#[derive(Debug, Deserialize)]
struct IntrospectRequest {
    credential: String,
    kind: CredentialKind,
}

#[derive(Debug, Serialize)]
struct IntrospectResponse {
    active: bool,
    subject: String,
    scopes: Vec<String>,
    expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "snake_case")]
enum CredentialKind {
    ApiKey,
    Bearer,
}

async fn introspect(
    State(state): State<Arc<AppState>>,
    Json(request): Json<IntrospectRequest>,
) -> Result<Json<IntrospectResponse>, ApiError> {
    match request.kind {
        CredentialKind::Bearer => {
            let claims = state
                .signer()
                .verify(&request.credential)
                .map_err(|_| ApiError::Unauthorized("invalid token".into()))?;
            let expires = DateTime::<Utc>::from_timestamp(claims.exp, 0);
            Ok(Json(IntrospectResponse {
                active: true,
                subject: claims.sub,
                scopes: claims.scopes(),
                expires_at: expires,
            }))
        }
        CredentialKind::ApiKey => {
            let hash = crate::security::hash_api_key(&request.credential);
            let Some(record) = state.db().find_api_key_by_hash(&hash)? else {
                return Ok(Json(IntrospectResponse {
                    active: false,
                    subject: "".to_string(),
                    scopes: vec![],
                    expires_at: None,
                }));
            };
            if record.revoked {
                return Ok(Json(IntrospectResponse {
                    active: false,
                    subject: record.user_id,
                    scopes: vec![],
                    expires_at: record.expires_at,
                }));
            }
            if let Some(expiry) = record.expires_at {
                if expiry < Utc::now() {
                    return Ok(Json(IntrospectResponse {
                        active: false,
                        subject: record.user_id.clone(),
                        scopes: vec![],
                        expires_at: Some(expiry),
                    }));
                }
            }
            state.db().touch_api_key(&record.id)?;
            Ok(Json(IntrospectResponse {
                active: true,
                subject: record.user_id,
                scopes: record.scopes,
                expires_at: record.expires_at,
            }))
        }
    }
}

#[derive(Debug, Deserialize, Validate)]
struct CreateApiKeyRequest {
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    scopes: Vec<String>,
    #[serde(default)]
    prefix: Option<String>,
    #[serde(default)]
    expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize)]
struct ApiKeyResponse {
    id: String,
    name: Option<String>,
    prefix: String,
    scopes: Vec<String>,
    created_at: DateTime<Utc>,
    expires_at: Option<DateTime<Utc>>,
    revoked: bool,
    key: Option<String>,
}

async fn create_api_key(
    State(state): State<Arc<AppState>>,
    AuthenticatedAccount(account): AuthenticatedAccount,
    Json(request): Json<CreateApiKeyRequest>,
) -> Result<Json<ApiKeyResponse>, ApiError> {
    let prefix = request
        .prefix
        .clone()
        .unwrap_or_else(|| state.settings().security.default_api_key_prefix.clone());
    let scopes = merge_scopes(&account.scopes, &request.scopes);
    let record = state.db().create_api_key(
        &account.user.id,
        request.name.as_deref(),
        &scopes,
        &prefix,
        request.expires_at,
    )?;
    state.db().audit(AuditEvent {
        user_id: Some(&account.user.id),
        event_type: "apikey.create",
        details: Some(serde_json::json!({ "apiKeyId": record.id })),
    })?;
    Ok(Json(ApiKeyResponse::from(record)))
}

async fn list_api_keys(
    State(state): State<Arc<AppState>>,
    AuthenticatedAccount(account): AuthenticatedAccount,
) -> Result<Json<Vec<ApiKeyResponse>>, ApiError> {
    let keys = state
        .db()
        .list_api_keys(&account.user.id)?
        .into_iter()
        .map(ApiKeyResponse::from)
        .collect();
    Ok(Json(keys))
}

async fn delete_api_key(
    State(state): State<Arc<AppState>>,
    AuthenticatedAccount(account): AuthenticatedAccount,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    state.db().revoke_api_key(&id)?;
    state.db().audit(AuditEvent {
        user_id: Some(&account.user.id),
        event_type: "apikey.revoke",
        details: Some(serde_json::json!({ "apiKeyId": id })),
    })?;
    Ok(StatusCode::NO_CONTENT.into_response())
}

async fn rotate_api_key(
    State(state): State<Arc<AppState>>,
    AuthenticatedAccount(account): AuthenticatedAccount,
    Path(id): Path<String>,
    Json(request): Json<CreateApiKeyRequest>,
) -> Result<Json<ApiKeyResponse>, ApiError> {
    let prefix = request
        .prefix
        .clone()
        .unwrap_or_else(|| state.settings().security.default_api_key_prefix.clone());
    let record = state.db().rotate_api_key(&id, &prefix)?;
    state.db().audit(AuditEvent {
        user_id: Some(&account.user.id),
        event_type: "apikey.rotate",
        details: Some(serde_json::json!({ "apiKeyId": id })),
    })?;
    Ok(Json(ApiKeyResponse::from(record)))
}

#[derive(Clone)]
struct AuthenticatedAccountInner {
    user: User,
    scopes: Vec<String>,
}

pub struct AuthenticatedAccount(pub AuthenticatedAccountInner);

#[axum::async_trait]
impl<S> axum::extract::FromRequestParts<S> for AuthenticatedAccount
where
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        let state = parts
            .extensions
            .get::<Arc<AppState>>()
            .cloned()
            .ok_or_else(|| ApiError::Unauthorized("state missing".into()))?;
        let headers = parts.headers.clone();
        authenticate(&state, &headers).await
    }
}

async fn authenticate(
    state: &Arc<AppState>,
    headers: &HeaderMap,
) -> Result<AuthenticatedAccount, ApiError> {
    if let Some(auth) = headers.get(header::AUTHORIZATION) {
        let value = auth
            .to_str()
            .map_err(|_| ApiError::Unauthorized("invalid authorization header".into()))?;
        if let Some(token) = value.strip_prefix("Bearer ") {
            let claims = state
                .signer()
                .verify(token)
                .map_err(|_| ApiError::Unauthorized("invalid token".into()))?;
            let user = state
                .db()
                .find_user_by_id(&claims.sub)?
                .ok_or_else(|| ApiError::Unauthorized("user not found".into()))?;
            let scopes = claims.scopes();
            return Ok(AuthenticatedAccount(AuthenticatedAccountInner {
                user,
                scopes,
            }));
        }
    }

    if let Some(api_key) = headers.get("x-api-key") {
        let value = api_key
            .to_str()
            .map_err(|_| ApiError::Unauthorized("invalid api key".into()))?;
        let hash = crate::security::hash_api_key(value);
        let record = state
            .db()
            .find_api_key_by_hash(&hash)?
            .ok_or_else(|| ApiError::Unauthorized("api key not found".into()))?;
        if record.revoked {
            return Err(ApiError::Unauthorized("api key revoked".into()));
        }
        if let Some(expiry) = record.expires_at {
            if expiry < Utc::now() {
                return Err(ApiError::Unauthorized("api key expired".into()));
            }
        }
        let user = state
            .db()
            .find_user_by_id(&record.user_id)?
            .ok_or_else(|| ApiError::Unauthorized("user not found".into()))?;
        state.db().touch_api_key(&record.id)?;
        return Ok(AuthenticatedAccount(AuthenticatedAccountInner {
            user,
            scopes: record.scopes,
        }));
    }

    Err(ApiError::Unauthorized("missing credentials".into()))
}

#[derive(Debug, Error)]
pub enum ApiError {
    #[error("{0}")]
    Validation(String),
    #[error("unauthorized: {0}")]
    Unauthorized(String),
    #[error("conflict: {0}")]
    Conflict(String),
    #[error("internal error: {0}")]
    Internal(String),
}

impl From<validator::ValidationErrors> for ApiError {
    fn from(value: validator::ValidationErrors) -> Self {
        ApiError::Validation(value.to_string())
    }
}

impl From<anyhow::Error> for ApiError {
    fn from(value: anyhow::Error) -> Self {
        ApiError::Internal(value.to_string())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        match &self {
            ApiError::Validation(reason) => {
                (StatusCode::BAD_REQUEST, reason.clone()).into_response()
            }
            ApiError::Unauthorized(reason) => {
                (StatusCode::UNAUTHORIZED, reason.clone()).into_response()
            }
            ApiError::Conflict(reason) => (StatusCode::CONFLICT, reason.clone()).into_response(),
            ApiError::Internal(reason) => {
                warn!(?reason, "internal error");
                (StatusCode::INTERNAL_SERVER_ERROR, "internal error").into_response()
            }
        }
    }
}

impl From<User> for UserResponse {
    fn from(user: User) -> Self {
        Self {
            id: user.id,
            email: user.email,
            display_name: user.display_name,
            scopes: user.scopes,
            roles: user.roles,
            created_at: user.created_at,
            updated_at: user.updated_at,
        }
    }
}

impl From<ApiKeyRecord> for ApiKeyResponse {
    fn from(record: ApiKeyRecord) -> Self {
        Self {
            id: record.id,
            name: record.name,
            prefix: record.prefix,
            scopes: record.scopes,
            created_at: record.created_at,
            expires_at: record.expires_at,
            revoked: record.revoked,
            key: record.plain_key,
        }
    }
}
