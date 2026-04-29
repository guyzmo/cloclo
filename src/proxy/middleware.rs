use axum::Router;
use tower_http::cors::{Any, CorsLayer};
use tower_http::trace::TraceLayer;

/// Adds request logging and CORS middleware to the router.
pub fn with_logging<S: Clone + Send + Sync + 'static>(router: Router<S>) -> Router<S> {
    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    router
        .layer(TraceLayer::new_for_http())
        .layer(cors)
}
