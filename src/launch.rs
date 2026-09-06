use colored::Colorize;

use crate::chanson::switching_quote;
use crate::config::{daemon_secret_path, load_config, ClocloConfig};
use crate::error::ClocloError;

/// Async HTTP client for control API calls.
fn control_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()
        .unwrap()
}

/// Resolves the proxy port: from CLOCLO_PORT env var, or from config.
fn resolve_port() -> Result<u16, ClocloError> {
    if let Ok(port_str) = std::env::var("CLOCLO_PORT") {
        port_str.parse().map_err(|_| {
            ClocloError::Config(format!("Invalid CLOCLO_PORT: {}", port_str))
        })
    } else {
        let config = load_config()?;
        Ok(config.general.port)
    }
}

/// Resolves the proxy secret: from CLOCLO_SECRET env var, or from the daemon's
/// per-port secret file on disk. Never sends an unauthenticated request if
/// neither is available.
pub(crate) fn resolve_secret(port: u16) -> Result<String, ClocloError> {
    if let Ok(secret) = std::env::var("CLOCLO_SECRET") {
        Ok(secret)
    } else {
        std::fs::read_to_string(daemon_secret_path(port))
            .map(|s| s.trim().to_string())
            .map_err(|_| {
                ClocloError::Auth(
                    "no cloclo secret found (set CLOCLO_SECRET or start the daemon with `cloclo start`)"
                        .into(),
                )
            })
    }
}

/// Launches Claude Code with a per-session proxy.
///
/// Starts a proxy on a random port, runs Claude Code pointing at it,
/// and shuts everything down when Claude exits or the process is interrupted.
pub async fn launch_claude(
    profile: Option<&str>,
    claude_args: &[String],
) -> Result<(), ClocloError> {
    let config = load_config()?;
    let profile_name = profile
        .unwrap_or(&config.general.default_profile)
        .to_string();

    if !config.profiles.contains_key(&profile_name) {
        return Err(ClocloError::ProfileNotFound(profile_name));
    }

    let claude_bin = config
        .general
        .claude_bin
        .clone()
        .unwrap_or_else(|| std::path::PathBuf::from("claude"));

    // Shutdown channel: fires when we want the proxy to stop.
    // Both the `axum::serve` graceful-shutdown future AND the `/_cloclo/stop`
    // handler share this single channel (the sender is passed into build_router
    // so that the HTTP stop endpoint can also trigger shutdown).
    let (shutdown_tx, shutdown_rx) = tokio::sync::watch::channel(false);

    // Start a per-session proxy on a random port.
    let (port, secret, proxy_handle) = crate::proxy::server::spawn_session(
        config,
        &profile_name,
        shutdown_tx.clone(), // router's stop endpoint writes to this
        shutdown_rx,         // axum::serve reads from this
    ).await?;

    eprintln!(
        "{} cloclo session on port {} — profile {}",
        "->".bold().green(),
        port.to_string().bold(),
        profile_name.bold().cyan(),
    );

    let proxy_url = format!("http://127.0.0.1:{}", port);

    // Use tokio::process::Command so we don't block the runtime.
    let mut child = tokio::process::Command::new(&claude_bin)
        .args(claude_args)
        .env("ANTHROPIC_BASE_URL", &proxy_url)
        .env("ANTHROPIC_API_KEY", &secret)
        .env("CLOCLO_PORT", port.to_string())
        .env("CLOCLO_SECRET", &secret)
        .env_remove("ANTHROPIC_AUTH_TOKEN")
        .stdin(std::process::Stdio::inherit())
        .stdout(std::process::Stdio::inherit())
        .stderr(std::process::Stdio::inherit())
        .spawn()
        .map_err(|e| {
            ClocloError::Subprocess(format!("Failed to launch Claude Code: {}", e))
        })?;

    // Wait for either: claude exits, or we get SIGINT/SIGTERM.
    let status = tokio::select! {
        status = child.wait() => {
            status.map_err(|e| ClocloError::Subprocess(format!("Wait failed: {}", e)))?
        }
        _ = tokio::signal::ctrl_c() => {
            // Got Ctrl+C: SIGINT is delivered to the entire foreground process
            // group, so the child may already be dead by the time we get here.
            // `child.kill()` returns an error on an already-exited process; we
            // intentionally discard it.  We still call `child.wait()` to reap
            // the zombie regardless.
            let _ = child.kill().await;
            let _ = child.wait().await;
            // Signal the proxy to stop and wait for it to fully shut down so
            // that in-flight SSE streams are drained before we exit.
            let _ = shutdown_tx.send(true);
            let _ = proxy_handle.await;
            return Ok(());
        }
    };

    // Claude exited normally — shut down the proxy and wait for it to drain.
    let _ = shutdown_tx.send(true);
    let _ = proxy_handle.await;

    if !status.success() {
        return Err(ClocloError::Subprocess(format!(
            "Claude Code exited with status: {}",
            status
        )));
    }

    Ok(())
}

/// Switches the profile on a running proxy via the control API.
/// Reads the port from CLOCLO_PORT env var or config.
pub async fn switch_profile_cli(profile: &str) -> Result<(), ClocloError> {
    let port = resolve_port()?;
    let secret = resolve_secret(port)?;
    let url = format!("http://127.0.0.1:{}/_cloclo/switch", port);

    let client = control_client();
    let resp = client
        .post(&url)
        .header("x-api-key", &secret)
        .json(&serde_json::json!({ "profile": profile }))
        .send()
        .await
        .map_err(|e| {
            ClocloError::Proxy(format!(
                "Failed to contact proxy on port {} (is it running?): {}",
                port, e
            ))
        })?;

    if resp.status().is_success() {
        let body: serde_json::Value = resp.json().await.map_err(|e| {
            ClocloError::Proxy(format!("Invalid response: {}", e))
        })?;
        println!("{} {}", "->".bold().green(), switching_quote());
        println!(
            "Switched from '{}' to '{}'",
            body["previous"].as_str().unwrap_or("?"),
            body["current"].as_str().unwrap_or("?"),
        );
    } else {
        let body = resp.text().await.unwrap_or_default();
        return Err(ClocloError::Proxy(format!("Switch failed: {}", body)));
    }

    Ok(())
}

/// Sets or clears the model override on a running proxy.
pub async fn set_model_cli(model: Option<&str>) -> Result<(), ClocloError> {
    let port = resolve_port()?;
    let secret = resolve_secret(port)?;
    let url = format!("http://127.0.0.1:{}/_cloclo/model", port);

    let client = control_client();
    let resp = client
        .post(&url)
        .header("x-api-key", &secret)
        .json(&serde_json::json!({ "model": model }))
        .send()
        .await
        .map_err(|e| {
            ClocloError::Proxy(format!(
                "Failed to contact proxy on port {} (is it running?): {}",
                port, e
            ))
        })?;

    if resp.status().is_success() {
        let body: serde_json::Value = resp.json().await.map_err(|e| {
            ClocloError::Proxy(format!("Invalid response: {}", e))
        })?;
        let effective = body["effective_model"].as_str().unwrap_or("?");
        if model.is_some() {
            println!("{} Model set to {}", "->".bold().green(), effective.bold().cyan());
        } else {
            println!("{} Model override cleared (using {})", "->".bold().green(), effective);
        }
    } else {
        let body = resp.text().await.unwrap_or_default();
        return Err(ClocloError::Proxy(format!("Failed: {}", body)));
    }

    Ok(())
}

/// Shows the status of the running proxy.
pub async fn show_status() -> Result<(), ClocloError> {
    let port = resolve_port()?;
    let secret = resolve_secret(port)?;
    let url = format!("http://127.0.0.1:{}/_cloclo/status", port);

    let client = control_client();
    let resp = client.get(&url).header("x-api-key", &secret).send().await.map_err(|e| {
        ClocloError::Proxy(format!(
            "Failed to contact proxy on port {} (is it running?): {}",
            port, e
        ))
    })?;

    if resp.status().is_success() {
        let body: serde_json::Value = resp.json().await.map_err(|e| {
            ClocloError::Proxy(format!("Invalid response: {}", e))
        })?;
        println!("{}", "cloclo status".bold().underline());
        println!(
            "  Profile:   {}",
            body["active_profile"].as_str().unwrap_or("?").bold().cyan()
        );
        println!(
            "  Upstream:  {}",
            body["upstream_url"].as_str().unwrap_or("?")
        );
        println!("  Port:      {}", body["port"]);
        if let Some(model) = body["model_override"].as_str() {
            println!("  Model:     {} (override)", model.bold().yellow());
        } else if let Some(model) = body["model"].as_str() {
            println!("  Model:     {}", model);
        }
        println!(
            "  Requests:  {}",
            body["stats"]["requests_forwarded"]
        );
        if let Some(uptime) = body["stats"]["uptime_secs"].as_u64() {
            println!("  Uptime:    {}s", uptime);
        }
    } else {
        let body = resp.text().await.unwrap_or_default();
        return Err(ClocloError::Proxy(format!("Status query failed: {}", body)));
    }

    Ok(())
}

/// Lists all configured profiles.
pub fn list_profiles(config: &ClocloConfig) {
    println!("{}", "Configured Profiles".bold().underline());
    for (name, profile) in &config.profiles {
        let marker = if name == &config.general.default_profile {
            " (default)".dimmed().to_string()
        } else {
            String::new()
        };
        println!(
            "  {} — {}{}",
            name.bold().cyan(),
            profile.display_name(),
            marker
        );
    }
}
