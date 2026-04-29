use std::time::Duration;

use tokio::process::Command;
use tracing::{info, warn};

use crate::config::SubprocessConfig;
use crate::error::ClocloError;
use crate::proxy::state::ManagedChild;

/// Spawns a subprocess and waits until it's healthy (responds on its health port).
///
/// Returns a `ManagedChild` that can be stored in the proxy state.
pub async fn spawn_and_wait_healthy(
    subprocess_config: &SubprocessConfig,
    profile_name: &str,
) -> Result<ManagedChild, ClocloError> {
    info!(
        "Spawning subprocess: {} {:?}",
        subprocess_config.command, subprocess_config.args
    );

    let child = Command::new(&subprocess_config.command)
        .args(&subprocess_config.args)
        .kill_on_drop(true)
        .spawn()
        .map_err(|e| {
            ClocloError::Subprocess(format!(
                "Failed to spawn '{}': {}",
                subprocess_config.command, e
            ))
        })?;

    // Poll the health port until it responds or we time out.
    let timeout = Duration::from_secs(subprocess_config.startup_timeout_secs);
    let start = std::time::Instant::now();
    let health_url = format!("http://127.0.0.1:{}/health", subprocess_config.health_port);

    let client = reqwest::Client::new();

    loop {
        if start.elapsed() > timeout {
            return Err(ClocloError::Subprocess(format!(
                "Subprocess did not become healthy within {} seconds",
                subprocess_config.startup_timeout_secs
            )));
        }

        match client.get(&health_url).send().await {
            Ok(resp) if resp.status().is_success() => {
                info!("Subprocess is healthy on port {}", subprocess_config.health_port);
                break;
            }
            Ok(_) => {
                warn!("Subprocess health check returned non-success, retrying...");
            }
            Err(_) => {
                // Not ready yet.
            }
        }

        tokio::time::sleep(Duration::from_millis(250)).await;
    }

    Ok(ManagedChild {
        child,
        profile_name: profile_name.to_string(),
        port: subprocess_config.health_port,
    })
}
