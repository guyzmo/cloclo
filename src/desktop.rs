use std::path::PathBuf;

use colored::Colorize;

use crate::config::ClocloConfig;
use crate::error::ClocloError;

const DEFAULT_APP_PATH: &str = "/Applications/Claude.app";

/// The data directory a desktop profile uses, whether or not it exists yet.
fn data_dir_for(config: &ClocloConfig, profile_name: &str) -> Option<PathBuf> {
    let profile = config.desktop.get(profile_name)?;
    Some(profile.data_dir.clone().unwrap_or_else(|| {
        dirs::data_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("cloclo")
            .join("desktop")
            .join(profile_name)
    }))
}

/// Launches Claude.app against the given desktop profile's isolated data
/// directory. If this is the profile's first launch, the app opens logged
/// out — log in normally in the window that appears, the session persists
/// in that profile's data dir from then on.
pub async fn launch_desktop(config: &ClocloConfig, profile_name: &str) -> Result<(), ClocloError> {
    let data_dir = data_dir_for(config, profile_name).ok_or_else(|| {
        ClocloError::ProfileNotFound(format!("desktop profile '{}'", profile_name))
    })?;
    let is_first_launch = !data_dir.exists();
    std::fs::create_dir_all(&data_dir)?;

    let app_path = config
        .general
        .desktop_app_path
        .clone()
        .unwrap_or_else(|| PathBuf::from(DEFAULT_APP_PATH));

    tokio::process::Command::new("open")
        .arg("-na")
        .arg(&app_path)
        .arg("--args")
        .arg(format!("--user-data-dir={}", data_dir.display()))
        .status()
        .await
        .map_err(|e| ClocloError::Subprocess(format!("Failed to launch {}: {}", app_path.display(), e)))?;

    if is_first_launch {
        println!(
            "{} First launch for desktop profile '{}' — log in normally in the window that just opened.",
            "->".bold().green(),
            profile_name.bold().cyan()
        );
    } else {
        println!(
            "{} Launched {} for desktop profile '{}'",
            "->".bold().green(),
            app_path.display(),
            profile_name.bold().cyan()
        );
    }

    Ok(())
}

/// Lists configured desktop profiles and whether each has completed login.
pub fn list_desktop_profiles(config: &ClocloConfig) {
    println!("{}", "Configured Desktop Profiles".bold().underline());
    for name in config.desktop.keys() {
        let profile = &config.desktop[name];
        let dir = data_dir_for(config, name).expect("profile came from config.desktop");
        let status = if dir.join("Cookies").exists() {
            "logged in".green()
        } else {
            "not logged in yet".dimmed()
        };
        println!(
            "  {} — {} ({})",
            name.bold().cyan(),
            profile.display_name,
            status
        );
    }
}
