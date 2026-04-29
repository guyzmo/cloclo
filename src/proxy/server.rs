use std::sync::Arc;
use tokio::net::TcpListener;
use tokio::sync::RwLock;
use tracing::info;

use crate::auth::resolve_profile;
use crate::chanson::{farewell, startup_banner};
use crate::config::ClocloConfig;
use crate::error::ClocloError;
use crate::proxy::control;
use crate::proxy::forward;
use crate::proxy::middleware::with_logging;
use crate::proxy::state::{Alexandrie, ProxyState, SessionStats};

/// Builds the Axum router with all routes.
pub fn build_router(
    alexandrie: Alexandrie,
    shutdown_tx: tokio::sync::watch::Sender<bool>,
) -> axum::Router {
    use axum::routing::{get, post};

    let control_routes = axum::Router::new()
        .route("/status", get(control::get_status))
        .route("/switch", post(control::switch_profile))
        .route("/profiles", get(control::list_profiles))
        .route("/health", get(control::health_check))
        .with_state(alexandrie.clone());

    let stop_route = axum::Router::new()
        .route("/stop", post(control::stop_server))
        .with_state(shutdown_tx);

    let proxy_routes = axum::Router::new()
        .route("/v1/messages", post(forward::forward_messages))
        .route("/v1/messages/count_tokens", post(forward::forward_json))
        .route("/v1/models", get(forward::forward_get))
        .route("/v1/models/{model_id}", get(forward::forward_get))
        .fallback(forward::forward_fallback)
        .with_state(alexandrie);

    let app = axum::Router::new()
        .nest("/_cloclo", control_routes)
        .nest("/_cloclo", stop_route)
        .merge(proxy_routes);

    with_logging(app)
}

/// Starts the proxy server and runs until shutdown is signaled.
pub async fn run(config: ClocloConfig, profile_name: &str, port: u16) -> Result<(), ClocloError> {
    let bind_addr = format!("{}:{}", config.general.bind, port);

    // Resolve the initial profile.
    let (auth, upstream_url) = resolve_profile(&config, profile_name)?;

    let state = ProxyState {
        active_profile: profile_name.to_string(),
        active_auth: auth,
        upstream_url,
        config,
        client: reqwest::Client::new(),
        managed_subprocess: None,
        stats: SessionStats {
            requests_forwarded: 0,
            profile_switches: 0,
            started_at: Some(std::time::Instant::now()),
        },
    };

    let alexandrie: Alexandrie = Arc::new(RwLock::new(state));

    // Shutdown channel.
    let (shutdown_tx, mut shutdown_rx) = tokio::sync::watch::channel(false);

    let app = build_router(alexandrie, shutdown_tx);

    let listener = TcpListener::bind(&bind_addr).await.map_err(|e| {
        ClocloError::Proxy(format!("Failed to bind to {}: {}", bind_addr, e))
    })?;

    println!("{}", startup_banner(port, profile_name));
    info!("Proxy listening on {}", bind_addr);

    axum::serve(listener, app)
        .with_graceful_shutdown(async move {
            let _ = shutdown_rx.wait_for(|&v| v).await;
            info!("{}", farewell());
        })
        .await
        .map_err(|e| ClocloError::Proxy(format!("Server error: {}", e)))?;

    Ok(())
}
