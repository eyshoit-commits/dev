use axum::http::StatusCode;
use thiserror::Error;

pub type HelixResult<T> = Result<T, HelixError>;

#[derive(Debug, Error)]
pub enum HelixError {
    #[error("configuration error: {0}")]
    Configuration(String),
    #[error("authentication failure")]
    Authentication,
    #[error("authorization failure")]
    Authorization,
    #[error("storage error: {0}")]
    Storage(String),
    #[error("query error: {0}")]
    Query(String),
    #[error("plugin error: {0}")]
    Plugin(String),
    #[error("telemetry error: {0}")]
    Telemetry(String),
    #[error("internal error: {0}")]
    Internal(String),
}

impl HelixError {
    pub fn status_code(&self) -> StatusCode {
        match self {
            HelixError::Configuration(_) => StatusCode::BAD_REQUEST,
            HelixError::Authentication => StatusCode::UNAUTHORIZED,
            HelixError::Authorization => StatusCode::FORBIDDEN,
            HelixError::Storage(_) => StatusCode::INTERNAL_SERVER_ERROR,
            HelixError::Query(_) => StatusCode::BAD_REQUEST,
            HelixError::Plugin(_) => StatusCode::BAD_GATEWAY,
            HelixError::Telemetry(_) => StatusCode::INTERNAL_SERVER_ERROR,
            HelixError::Internal(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}
