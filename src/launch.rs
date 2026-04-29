use colored::Colorize;

use crate::chanson::switching_quote;
use crate::config::{load_config, ClocloConfig};
use crate::error::ClocloError;

/// Async HTTP client for control API calls (avoids reqwest::blocking panic inside tokio).
fn control_client() -> reqwest::Client {
    reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()
        .unwrap()
}

/// Launches Claude Code with the proxy configured, optionally starting the proxy first.
pub async fn launch_claude(
    profile: Option<&str>,
    claude_args: &[String],
) -> Result<(), ClocloError> {
    let config = load_config()?;
    let profile_name = profile
        .unwrap_or(&config.general.default_profile)
        .to_string();

    // Verify the profile exists.
    if !config.profiles.contains_key(&profile_name) {
        return Err(ClocloError::ProfileNotFound(profile_name));
    }

    let port = config.general.port;
    let claude_bin = config
        .general
        .claude_bin
        .as_deref()
        .unwrap_or_else(|| std::path::Path::new("claude"));

    // Check if the proxy is running; auto-start if not.
    let health_url = format!("http://127.0.0.1:{}/_cloclo/health", port);
    let client = control_client();

    let proxy_running = client
        .get(&health_url)
        .send()
        .await
        .map(|r| r.status().is_success())
        .unwrap_or(false);

    if !proxy_running {
        println!(
            "{} Proxy not running, starting daemon...",
            "->".bold().yellow()
        );
        let pid_path = crate::daemon::pid_file_path(config.general.pid_file.as_ref());
        crate::daemon::start_daemon(Some(&profile_name), Some(port), &pid_path)?;
        // Wait for proxy to become ready.
        let mut ready = false;
        for _ in 0..20 {
            tokio::time::sleep(std::time::Duration::from_millis(250)).await;
            if client
                .get(&health_url)
                .send()
                .await
                .map(|r| r.status().is_success())
                .unwrap_or(false)
            {
                println!("{} Proxy is ready!", "->".bold().green());
                ready = true;
                break;
            }
        }
        if !ready {
            println!(
                "{} Proxy may not be ready yet, launching Claude anyway...",
                "->".bold().yellow()
            );
        }
    }

    // Switch to the requested profile.
    let switch_url = format!("http://127.0.0.1:{}/_cloclo/switch", port);
    let _ = client
        .post(&switch_url)
        .json(&serde_json::json!({ "profile": profile_name }))
        .send()
        .await;

    println!(
        "{} {} {} {} {}",
        "->".bold().green(),
        "Comme d'habitude —".italic(),
        "launching Claude Code with profile".bold().green(),
        profile_name.bold().cyan(),
        "via le proxy magnifique!".italic()
    );

    // Set the API base URL to point at our proxy.
    let proxy_url = format!("http://127.0.0.1:{}", port);

    let mut cmd = std::process::Command::new(claude_bin);
    cmd.args(claude_args);
    cmd.env("ANTHROPIC_BASE_URL", &proxy_url);
    // Set a dummy API key that passes Claude Code's format validation.
    // The proxy handles real auth; this value is never sent to Anthropic.
    cmd.env("ANTHROPIC_API_KEY", "sk-cloclo-proxy");
    // Remove any conflicting auth token env var.
    cmd.env_remove("ANTHROPIC_AUTH_TOKEN");

    let status = cmd.status().map_err(|e| {
        ClocloError::Subprocess(format!("Failed to launch Claude Code: {}", e))
    })?;

    if !status.success() {
        return Err(ClocloError::Subprocess(format!(
            "Claude Code exited with status: {}",
            status
        )));
    }

    Ok(())
}

/// Switches the profile on a running proxy via the control API.
pub async fn switch_profile_cli(profile: &str) -> Result<(), ClocloError> {
    let config = load_config()?;
    let port = config.general.port;
    let url = format!("http://127.0.0.1:{}/_cloclo/switch", port);

    let client = control_client();
    let resp = client
        .post(&url)
        .json(&serde_json::json!({ "profile": profile }))
        .send()
        .await
        .map_err(|e| {
            ClocloError::Proxy(format!(
                "Failed to contact proxy (is it running?): {}",
                e
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

/// Shows the status of the running proxy.
pub async fn show_status() -> Result<(), ClocloError> {
    let config = load_config()?;
    let port = config.general.port;
    let url = format!("http://127.0.0.1:{}/_cloclo/status", port);

    let client = control_client();
    let resp = client.get(&url).send().await.map_err(|e| {
        ClocloError::Proxy(format!(
            "Failed to contact proxy (is it running?): {}",
            e
        ))
    })?;

    if resp.status().is_success() {
        let body: serde_json::Value = resp.json().await.map_err(|e| {
            ClocloError::Proxy(format!("Invalid response: {}", e))
        })?;
        println!("{}", "Cloclo Proxy Status".bold().underline());
        println!(
            "  Active profile:  {}",
            body["active_profile"]
                .as_str()
                .unwrap_or("?")
                .bold()
                .cyan()
        );
        println!(
            "  Upstream URL:    {}",
            body["upstream_url"].as_str().unwrap_or("?")
        );
        println!(
            "  Requests:        {}",
            body["stats"]["requests_forwarded"]
        );
        println!(
            "  Profile switches: {}",
            body["stats"]["profile_switches"]
        );
        if let Some(uptime) = body["stats"]["uptime_secs"].as_u64() {
            println!("  Uptime:          {}s", uptime);
        }
        if let Some(profiles) = body["profiles"].as_array() {
            println!("  Profiles:        {}", profiles.len());
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
