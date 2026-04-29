use axum::Router;
use tower_http::trace::TraceLayer;

/// Adds request logging middleware to the router.
pub fn with_logging<S: Clone + Send + Sync + 'static>(router: Router<S>) -> Router<S> {
    router.layer(TraceLayer::new_for_http())
}
