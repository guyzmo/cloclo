use std::path::PathBuf;

use tracing::info;

use crate::chanson::DEFAULT_PORT;
use crate::error::ClocloError;

/// Returns the path to the PID file, respecting config override.
pub fn pid_file_path(override_path: Option<&PathBuf>) -> PathBuf {
    if let Some(p) = override_path {
        return p.clone();
    }
    dirs::runtime_dir()
        .or_else(|| dirs::state_dir())
        .unwrap_or_else(|| {
            dirs::home_dir()
                .unwrap_or_else(|| PathBuf::from("."))
                .join(".local")
                .join("state")
        })
        .join("cloclo")
        .join("cloclo.pid")
}

/// Checks if the proxy is already running by reading the PID file
/// and checking if the process is alive.
pub fn is_proxy_running(pid_path: &PathBuf) -> Option<u32> {
    let pid_str = std::fs::read_to_string(pid_path).ok()?;
    let pid: u32 = pid_str.trim().parse().ok()?;

    // Check if the process is alive using kill -0.
    let status = std::process::Command::new("kill")
        .args(["-0", &pid.to_string()])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .ok()?;

    if status.success() {
        Some(pid)
    } else {
        // Stale PID file — clean up.
        let _ = std::fs::remove_file(pid_path);
        None
    }
}

/// Starts the proxy as a background daemon by re-executing self with `--foreground`.
pub fn start_daemon(
    profile: Option<&str>,
    port: Option<u16>,
    pid_path: &PathBuf,
) -> Result<(), ClocloError> {
    if let Some(pid) = is_proxy_running(pid_path) {
        return Err(ClocloError::Daemon(format!(
            "Proxy already running with PID {}",
            pid
        )));
    }

    let exe = std::env::current_exe().map_err(|e| {
        ClocloError::Daemon(format!("Failed to determine current executable: {}", e))
    })?;

    let mut cmd = std::process::Command::new(exe);
    cmd.arg("start").arg("--foreground");

    if let Some(p) = profile {
        cmd.args(["--profile", p]);
    }
    if let Some(port_val) = port {
        cmd.args(["-P", &port_val.to_string()]);
    }

    // Detach the child process.
    cmd.stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .stdin(std::process::Stdio::null());

    let child = cmd.spawn().map_err(|e| {
        ClocloError::Daemon(format!("Failed to spawn daemon: {}", e))
    })?;

    // Write PID file.
    if let Some(parent) = pid_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(pid_path, child.id().to_string())?;

    let port_display = port.unwrap_or(DEFAULT_PORT);
    info!("Daemon started with PID {} on port {}", child.id(), port_display);
    println!(
        "Cloclo daemon started (PID {}) on port {}",
        child.id(),
        port_display
    );

    Ok(())
}

/// Stops a running daemon by sending SIGTERM via `kill`.
pub fn stop_daemon(pid_path: &PathBuf) -> Result<(), ClocloError> {
    let pid = is_proxy_running(pid_path).ok_or_else(|| {
        ClocloError::Daemon("No running proxy found.".to_string())
    })?;

    let status = std::process::Command::new("kill")
        .args([&pid.to_string()])
        .status()
        .map_err(|e| ClocloError::Daemon(format!("Failed to send signal: {}", e)))?;

    if status.success() {
        let _ = std::fs::remove_file(pid_path);
        println!("Proxy (PID {}) stopped.", pid);
        Ok(())
    } else {
        Err(ClocloError::Daemon(format!(
            "Failed to stop process with PID {}",
            pid
        )))
    }
}
