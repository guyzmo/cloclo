use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

/// Top-level application errors (for CLI, config loading, etc.)
#[derive(Debug, thiserror::Error)]
pub enum ClocloError {
    #[error("Configuration error: {0}")]
    Config(String),

    #[error("Profile not found: {0}")]
    ProfileNotFound(String),

    #[error("Authentication error: {0}")]
    Auth(String),

    #[error("Proxy error: {0}")]
    Proxy(String),

    #[error("Subprocess error: {0}")]
    Subprocess(String),

    #[error("Daemon error: {0}")]
    Daemon(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error(transparent)]
    Other(#[from] anyhow::Error),
}

/// Proxy-specific errors that implement `IntoResponse` for Axum handlers,
/// returning Anthropic-shaped JSON error bodies.
#[derive(Debug, thiserror::Error)]
pub enum ProxyError {
    #[error("Upstream request failed: {0}")]
    Upstream(String),

    #[error("Authentication error: {0}")]
    Auth(String),

    #[error("Bad request: {0}")]
    BadRequest(String),

    #[error("Profile not found: {0}")]
    ProfileNotFound(String),

    #[error("Internal error: {0}")]
    Internal(String),
}

impl IntoResponse for ProxyError {
    fn into_response(self) -> Response {
        let (status, error_type, message) = match &self {
            ProxyError::Upstream(msg) => (StatusCode::BAD_GATEWAY, "upstream_error", msg.clone()),
            ProxyError::Auth(msg) => (StatusCode::UNAUTHORIZED, "authentication_error", msg.clone()),
            ProxyError::BadRequest(msg) => (StatusCode::BAD_REQUEST, "invalid_request_error", msg.clone()),
            ProxyError::ProfileNotFound(msg) => (StatusCode::NOT_FOUND, "not_found_error", msg.clone()),
            ProxyError::Internal(msg) => (StatusCode::INTERNAL_SERVER_ERROR, "api_error", msg.clone()),
        };

        let body = json!({
            "type": "error",
            "error": {
                "type": error_type,
                "message": message,
            }
        });

        (status, axum::Json(body)).into_response()
    }
}

impl From<reqwest::Error> for ProxyError {
    fn from(err: reqwest::Error) -> Self {
        ProxyError::Upstream(err.to_string())
    }
}

impl From<anyhow::Error> for ProxyError {
    fn from(err: anyhow::Error) -> Self {
        ProxyError::Internal(err.to_string())
    }
}
