use axum::extract::State;
use axum::Json;
use serde::{Deserialize, Serialize};

use crate::auth::resolve_profile;
use crate::chanson::switching_quote;
use crate::error::ProxyError;
use crate::proxy::state::Alexandrie;

/// Request body for switching profiles.
#[derive(Debug, Deserialize)]
pub struct SwitchRequest {
    pub profile: String,
}

/// Response body after switching profiles.
#[derive(Debug, Serialize)]
pub struct SwitchResponse {
    pub previous: String,
    pub current: String,
    pub message: String,
}

/// Response body for status queries.
#[derive(Debug, Serialize)]
pub struct StatusResponse {
    pub active_profile: String,
    pub upstream_url: String,
    pub model: Option<String>,
    pub model_override: Option<String>,
    pub port: u16,
    pub stats: StatsResponse,
    pub profiles: Vec<String>,
}

/// Serializable stats.
#[derive(Debug, Serialize)]
pub struct StatsResponse {
    pub requests_forwarded: u64,
    pub profile_switches: u64,
    pub uptime_secs: Option<u64>,
}

/// `GET /_cloclo/status` — returns the current proxy status.
pub async fn get_status(
    State(alexandrie): State<Alexandrie>,
) -> Result<Json<StatusResponse>, ProxyError> {
    let state = alexandrie.read().await;
    let uptime = state.stats.started_at.map(|s| s.elapsed().as_secs());
    let profiles: Vec<String> = state.config.profiles.keys().cloned().collect();
    let model = state.config.profiles.get(&state.active_profile).and_then(|p| {
        match p {
            crate::config::ProfileConfig::ApiKey { model, .. } => model.clone(),
            crate::config::ProfileConfig::OAuth { model, .. } => model.clone(),
            crate::config::ProfileConfig::EnterpriseSso { model, .. } => model.clone(),
            crate::config::ProfileConfig::Proxy { model, .. } => model.clone(),
        }
    });

    Ok(Json(StatusResponse {
        active_profile: state.active_profile.clone(),
        upstream_url: state.upstream_url.clone(),
        model,
        model_override: state.model_override.clone(),
        port: state.port,
        stats: StatsResponse {
            requests_forwarded: state.stats.requests_forwarded,
            profile_switches: state.stats.profile_switches,
            uptime_secs: uptime,
        },
        profiles,
    }))
}

/// `POST /_cloclo/switch` — switches the active profile.
pub async fn switch_profile(
    State(alexandrie): State<Alexandrie>,
    Json(req): Json<SwitchRequest>,
) -> Result<Json<SwitchResponse>, ProxyError> {
    let mut state = alexandrie.write().await;

    let (auth, upstream_url) = resolve_profile(&state.config, &req.profile)
        .map_err(|e| ProxyError::ProfileNotFound(e.to_string()))?;

    let previous = state.active_profile.clone();

    // Handle subprocess lifecycle
    let new_profile_cfg = state.config.profiles.get(&req.profile).cloned();
    let needs_subprocess = matches!(
        &new_profile_cfg,
        Some(crate::config::ProfileConfig::Proxy { subprocess: Some(_), .. })
    );

    // Kill existing subprocess if switching away from it
    if let Some(ref mut existing) = state.managed_subprocess {
        if existing.profile_name != req.profile {
            let _ = existing.child.kill().await;
            state.managed_subprocess = None;
        }
    }

    // Spawn new subprocess if needed
    if needs_subprocess && state.managed_subprocess.is_none() {
        if let Some(crate::config::ProfileConfig::Proxy { subprocess: Some(ref sub_cfg), .. }) = new_profile_cfg {
            match crate::subprocess::spawn_and_wait_healthy(sub_cfg, &req.profile).await {
                Ok(child) => {
                    state.managed_subprocess = Some(child);
                }
                Err(e) => {
                    return Err(ProxyError::Internal(format!("Failed to start subprocess: {}", e)));
                }
            }
        }
    }

    state.active_profile = req.profile.clone();
    state.active_auth = auth;
    state.upstream_url = upstream_url;
    state.stats.profile_switches += 1;

    let message = switching_quote();

    Ok(Json(SwitchResponse {
        previous,
        current: req.profile,
        message,
    }))
}

/// `GET /_cloclo/profiles` — lists all configured profiles.
pub async fn list_profiles(
    State(alexandrie): State<Alexandrie>,
) -> Result<Json<Vec<ProfileInfo>>, ProxyError> {
    let state = alexandrie.read().await;
    let profiles: Vec<ProfileInfo> = state
        .config
        .profiles
        .iter()
        .map(|(name, config)| ProfileInfo {
            name: name.clone(),
            display_name: config.display_name().to_string(),
            active: name == &state.active_profile,
        })
        .collect();
    Ok(Json(profiles))
}

/// Profile info returned by the list endpoint.
#[derive(Debug, Serialize)]
pub struct ProfileInfo {
    pub name: String,
    pub display_name: String,
    pub active: bool,
}

/// Request body for setting the model override.
#[derive(Debug, Deserialize)]
pub struct ModelRequest {
    pub model: Option<String>,
}

/// Response body for model queries.
#[derive(Debug, Serialize)]
pub struct ModelResponse {
    pub model_override: Option<String>,
    pub profile_model: Option<String>,
    pub effective_model: String,
}

/// `GET /_cloclo/model` — returns the current model configuration.
pub async fn get_model(
    State(alexandrie): State<Alexandrie>,
) -> Result<Json<ModelResponse>, ProxyError> {
    let state = alexandrie.read().await;
    let profile_model = state.config.profiles.get(&state.active_profile).and_then(|p| {
        match p {
            crate::config::ProfileConfig::ApiKey { model, .. } => model.clone(),
            crate::config::ProfileConfig::OAuth { model, .. } => model.clone(),
            crate::config::ProfileConfig::EnterpriseSso { model, .. } => model.clone(),
            crate::config::ProfileConfig::Proxy { model, .. } => model.clone(),
        }
    });
    let effective = state.model_override.clone()
        .or(profile_model.clone())
        .unwrap_or_else(|| "(upstream default)".to_string());

    Ok(Json(ModelResponse {
        model_override: state.model_override.clone(),
        profile_model,
        effective_model: effective,
    }))
}

/// `POST /_cloclo/model` — sets or clears the model override.
/// Send `{"model": "claude-opus-4-6"}` to set, `{"model": null}` to clear.
pub async fn set_model(
    State(alexandrie): State<Alexandrie>,
    Json(req): Json<ModelRequest>,
) -> Result<Json<ModelResponse>, ProxyError> {
    let mut state = alexandrie.write().await;
    state.model_override = req.model;

    let profile_model = state.config.profiles.get(&state.active_profile).and_then(|p| {
        match p {
            crate::config::ProfileConfig::ApiKey { model, .. } => model.clone(),
            crate::config::ProfileConfig::OAuth { model, .. } => model.clone(),
            crate::config::ProfileConfig::EnterpriseSso { model, .. } => model.clone(),
            crate::config::ProfileConfig::Proxy { model, .. } => model.clone(),
        }
    });
    let effective = state.model_override.clone()
        .or(profile_model.clone())
        .unwrap_or_else(|| "(upstream default)".to_string());

    Ok(Json(ModelResponse {
        model_override: state.model_override.clone(),
        profile_model,
        effective_model: effective,
    }))
}

/// `GET /_cloclo/health` — simple health check.
pub async fn health_check() -> &'static str {
    "OK"
}

/// `POST /_cloclo/stop` — requests graceful shutdown.
pub async fn stop_server(
    State(shutdown_tx): State<tokio::sync::watch::Sender<bool>>,
) -> &'static str {
    let _ = shutdown_tx.send(true);
    "Shutting down..."
}
