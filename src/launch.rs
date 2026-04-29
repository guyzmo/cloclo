use colored::Colorize;

use crate::chanson::switching_quote;
use crate::config::{load_config, ClocloConfig};
use crate::error::ClocloError;

/// Launches Claude Code with the proxy configured, optionally starting the proxy first.
pub fn launch_claude(
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

    println!(
        "{} Launching Claude Code with profile '{}'",
        "->".bold().green(),
        profile_name.bold()
    );

    // Set the API base URL to point at our proxy.
    let proxy_url = format!("http://127.0.0.1:{}", port);

    let status = std::process::Command::new(claude_bin)
        .args(claude_args)
        .env("ANTHROPIC_BASE_URL", &proxy_url)
        .status()
        .map_err(|e| {
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
pub fn switch_profile_cli(profile: &str) -> Result<(), ClocloError> {
    let config = load_config()?;
    let port = config.general.port;
    let url = format!("http://127.0.0.1:{}/_cloclo/switch", port);

    let client = reqwest::blocking::Client::new();
    let resp = client
        .post(&url)
        .json(&serde_json::json!({ "profile": profile }))
        .send()
        .map_err(|e| {
            ClocloError::Proxy(format!(
                "Failed to contact proxy (is it running?): {}",
                e
            ))
        })?;

    if resp.status().is_success() {
        let body: serde_json::Value = resp.json().map_err(|e| {
            ClocloError::Proxy(format!("Invalid response: {}", e))
        })?;
        println!("{} {}", "->".bold().green(), switching_quote());
        println!(
            "Switched from '{}' to '{}'",
            body["previous"].as_str().unwrap_or("?"),
            body["current"].as_str().unwrap_or("?"),
        );
    } else {
        let body = resp.text().unwrap_or_default();
        return Err(ClocloError::Proxy(format!("Switch failed: {}", body)));
    }

    Ok(())
}

/// Shows the status of the running proxy.
pub fn show_status() -> Result<(), ClocloError> {
    let config = load_config()?;
    let port = config.general.port;
    let url = format!("http://127.0.0.1:{}/_cloclo/status", port);

    let client = reqwest::blocking::Client::new();
    let resp = client.get(&url).send().map_err(|e| {
        ClocloError::Proxy(format!(
            "Failed to contact proxy (is it running?): {}",
            e
        ))
    })?;

    if resp.status().is_success() {
        let body: serde_json::Value = resp.json().map_err(|e| {
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
        let body = resp.text().unwrap_or_default();
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
