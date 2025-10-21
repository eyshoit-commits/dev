use crate::config::Settings;
use axum::extract::FromRequestParts;
use axum::http::{header, HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use chrono::{DateTime, Utc};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use thiserror::Error;
use tracing::instrument;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthContext {
    pub subject: String,
    pub scopes: Vec<String>,
    pub expires_at: Option<DateTime<Utc>>,
}

impl AuthContext {
    pub fn ensure_scope(&self, scope: Scope) -> Result<(), AuthError> {
        let target = scope.as_str();
        if self.scopes.iter().any(|s| s == target || s == "*") {
            Ok(())
        } else {
            Err(AuthError::Forbidden(scope.as_str().to_string()))
        }
    }

    pub fn system() -> Self {
        Self {
            subject: "system".to_string(),
            scopes: vec![
                Scope::ReadLoadTests.as_str().to_string(),
                Scope::WriteLoadTests.as_str().to_string(),
            ],
            expires_at: None,
        }
    }
}

#[derive(Debug, Clone)]
pub struct AuthService {
    required: bool,
    client: Client,
    introspection_url: Option<String>,
}

impl AuthService {
    pub fn new(settings: &Settings) -> anyhow::Result<Self> {
        let client = Client::builder().build()?;
        let introspection_url = settings
            .plugin_bus
            .api_keys_endpoint
            .clone()
            .or_else(|| settings.security.api_keys_url.clone())
            .map(|base| format!("{}/api/auth/introspect", base.trim_end_matches('/')));
        Ok(Self {
            required: settings.security.require_authentication,
            client,
            introspection_url,
        })
    }

    #[instrument(skip(self, headers))]
    pub async fn authenticate(&self, headers: &HeaderMap) -> Result<AuthContext, AuthError> {
        if let Some(auth_header) = headers.get(header::AUTHORIZATION) {
            let value = auth_header
                .to_str()
                .map_err(|_| AuthError::Invalid("authorization header not valid utf8".into()))?;
            if let Some(token) = value.strip_prefix("Bearer ") {
                return self.introspect(token, CredentialType::Bearer).await;
            }
        }

        if let Some(api_key) = headers
            .get("x-api-key")
            .and_then(|value| value.to_str().ok())
        {
            return self.introspect(api_key, CredentialType::ApiKey).await;
        }

        if self.required {
            Err(AuthError::Unauthorized)
        } else {
            Ok(AuthContext::system())
        }
    }

    async fn introspect(
        &self,
        credential: &str,
        kind: CredentialType,
    ) -> Result<AuthContext, AuthError> {
        let Some(endpoint) = &self.introspection_url else {
            if self.required {
                return Err(AuthError::Unauthorized);
            }
            return Ok(AuthContext::system());
        };

        let request = IntrospectRequest {
            credential: credential.to_string(),
            kind,
        };

        let response = self
            .client
            .post(endpoint)
            .json(&request)
            .send()
            .await
            .map_err(|err| AuthError::Upstream(err.to_string()))?;

        if response.status().is_success() {
            let payload: IntrospectResponse = response
                .json()
                .await
                .map_err(|err| AuthError::Upstream(err.to_string()))?;
            if payload.active {
                return Ok(AuthContext {
                    subject: payload.subject,
                    scopes: payload.scopes,
                    expires_at: payload.expires_at,
                });
            }
            Err(AuthError::Unauthorized)
        } else {
            Err(AuthError::Unauthorized)
        }
    }
}

#[derive(Clone, Copy, Debug)]
pub enum Scope {
    ReadLoadTests,
    WriteLoadTests,
}

impl Scope {
    pub fn as_str(&self) -> &'static str {
        match self {
            Scope::ReadLoadTests => "read:loadtests",
            Scope::WriteLoadTests => "write:loadtests",
        }
    }
}

#[derive(Debug, Error)]
pub enum AuthError {
    #[error("unauthorized")]
    Unauthorized,
    #[error("forbidden: missing scope {0}")]
    Forbidden(String),
    #[error("invalid credentials: {0}")]
    Invalid(String),
    #[error("upstream auth error: {0}")]
    Upstream(String),
}

impl IntoResponse for AuthError {
    fn into_response(self) -> Response {
        match self {
            AuthError::Unauthorized => (StatusCode::UNAUTHORIZED, "unauthorized").into_response(),
            AuthError::Forbidden(scope) => (StatusCode::FORBIDDEN, scope).into_response(),
            AuthError::Invalid(reason) => (StatusCode::BAD_REQUEST, reason).into_response(),
            AuthError::Upstream(reason) => (
                StatusCode::BAD_GATEWAY,
                format!("auth upstream error: {reason}"),
            )
                .into_response(),
        }
    }
}

pub struct AuthenticatedUser(pub AuthContext);

#[axum::async_trait]
impl<S> FromRequestParts<S> for AuthenticatedUser
where
    S: Send + Sync,
{
    type Rejection = AuthError;

    async fn from_request_parts(
        parts: &mut axum::http::request::Parts,
        state: &S,
    ) -> Result<Self, Self::Rejection> {
        let auth = parts
            .extensions
            .get::<Arc<AuthService>>()
            .cloned()
            .ok_or(AuthError::Unauthorized)?;
        let headers = parts.headers.clone();
        let ctx = auth.authenticate(&headers).await?;
        Ok(Self(ctx))
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct IntrospectRequest {
    credential: String,
    kind: CredentialType,
}

#[derive(Debug, Serialize, Deserialize)]
struct IntrospectResponse {
    active: bool,
    subject: String,
    scopes: Vec<String>,
    expires_at: Option<DateTime<Utc>>,
}

#[derive(Debug, Serialize, Deserialize, Clone, Copy)]
#[serde(rename_all = "snake_case")]
enum CredentialType {
    ApiKey,
    Bearer,
}
