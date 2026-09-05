use axum::extract::{Request, State};
use axum::http::Method;
use axum::middleware::Next;
use axum::response::Response;
use axum::Router;
use tower_http::trace::TraceLayer;

use crate::error::ProxyError;
use crate::proxy::state::Alexandrie;

/// Adds request logging to the router.
pub fn with_logging<S: Clone + Send + Sync + 'static>(router: Router<S>) -> Router<S> {
    router.layer(TraceLayer::new_for_http())
}

/// Requires a matching `x-api-key` (or `authorization: Bearer <secret>`) on every
/// request except the unauthenticated health check.
pub async fn require_secret(
    State(alexandrie): State<Alexandrie>,
    req: Request,
    next: Next,
) -> Result<Response, ProxyError> {
    if req.method() == Method::GET && req.uri().path() == "/_cloclo/health" {
        return Ok(next.run(req).await);
    }

    let provided = req
        .headers()
        .get("x-api-key")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.to_string())
        .or_else(|| {
            req.headers()
                .get("authorization")
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.strip_prefix("Bearer "))
                .map(|s| s.to_string())
        });

    let secret = alexandrie.read().await.secret.clone();
    let ok = match &provided {
        Some(p) => constant_time_eq(p.as_bytes(), secret.as_bytes()),
        None => false,
    };

    if !ok {
        return Err(ProxyError::Auth("invalid or missing x-api-key".into()));
    }

    Ok(next.run(req).await)
}

/// Constant-time comparison to avoid a timing oracle on a localhost secret.
fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    a.iter().zip(b.iter()).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}
