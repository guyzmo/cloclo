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
        .route("/model", get(control::get_model).post(control::set_model))
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

/// Creates the shared proxy state from a config and profile name.
fn create_state(config: ClocloConfig, profile_name: &str) -> Result<ProxyState, ClocloError> {
    let (auth, upstream_url) = resolve_profile(&config, profile_name)?;
    Ok(ProxyState {
        active_profile: profile_name.to_string(),
        active_auth: auth,
        upstream_url,
        model_override: None,
        port: 0, // set after binding
        config,
        client: reqwest::Client::new(),
        managed_subprocess: None,
        stats: SessionStats {
            requests_forwarded: 0,
            profile_switches: 0,
            started_at: Some(std::time::Instant::now()),
        },
    })
}

/// Starts the proxy server and runs until shutdown is signaled.
/// Used by `cloclo start --foreground`.
pub async fn run(config: ClocloConfig, profile_name: &str, port: u16) -> Result<(), ClocloError> {
    let bind_addr = format!("{}:{}", config.general.bind, port);
    let mut state = create_state(config, profile_name)?;

    let listener = TcpListener::bind(&bind_addr).await.map_err(|e| {
        ClocloError::Proxy(format!("Failed to bind to {}: {}", bind_addr, e))
    })?;

    let actual_port = listener.local_addr()
        .map(|a| a.port()).unwrap_or(port);
    state.port = actual_port;

    let alexandrie: Alexandrie = Arc::new(RwLock::new(state));
    let (shutdown_tx, mut shutdown_rx) = tokio::sync::watch::channel(false);
    let app = build_router(alexandrie, shutdown_tx);

    eprintln!("{}", startup_banner(actual_port, profile_name));
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

/// Starts the proxy on port 0 (OS-assigned) and returns the actual port.
/// The server runs until `shutdown_rx` receives `true`.
/// Used by `cloclo launch` for per-session proxies.
pub async fn spawn_session(
    config: ClocloConfig,
    profile_name: &str,
    shutdown_rx: tokio::sync::watch::Receiver<bool>,
) -> Result<u16, ClocloError> {
    let mut state = create_state(config, profile_name)?;

    // Bind to port 0 — the OS assigns a free port.
    let listener = TcpListener::bind("127.0.0.1:0").await.map_err(|e| {
        ClocloError::Proxy(format!("Failed to bind: {}", e))
    })?;

    let port = listener.local_addr()
        .map_err(|e| ClocloError::Proxy(format!("Failed to get local addr: {}", e)))?
        .port();
    state.port = port;

    let alexandrie: Alexandrie = Arc::new(RwLock::new(state));
    let (shutdown_tx, _) = tokio::sync::watch::channel(false);
    let app = build_router(alexandrie, shutdown_tx);

    info!("Session proxy listening on 127.0.0.1:{}", port);

    // Spawn the server in a background task; it dies when shutdown_rx fires.
    let mut shutdown = shutdown_rx;
    tokio::spawn(async move {
        axum::serve(listener, app)
            .with_graceful_shutdown(async move {
                let _ = shutdown.wait_for(|&v| v).await;
            })
            .await
            .ok();
    });

    Ok(port)
}
