use std::path::Path;
use std::process::Stdio;

use colored::Colorize;
use tokio::io::{AsyncBufReadExt, BufReader};

use crate::auth::expand_tilde;
use crate::config::{ClocloConfig, ProfileConfig};
use crate::error::ClocloError;

/// Returns the token file path for a profile, if its type uses one.
fn token_file_for<'a>(profile: &'a ProfileConfig) -> Result<&'a Path, ClocloError> {
    match profile {
        ProfileConfig::OAuth { token_file, .. } => Ok(token_file),
        ProfileConfig::EnterpriseSso { token_file, .. } => Ok(token_file),
        ProfileConfig::Proxy { .. } => Err(ClocloError::Config(
            "Proxy profiles don't use a token file — configure `auth_token` instead.".to_string(),
        )),
    }
}

/// Logs into a `claude.ai` account for the given profile and stores the
/// resulting OAuth token in the profile's `token_file`.
///
/// If `token` is provided, it's stored directly (no browser flow). Otherwise
/// this runs `claude setup-token`, mirrors its output live, and tries to
/// detect the printed token automatically.
pub async fn login(
    config: &ClocloConfig,
    profile_name: &str,
    token: Option<String>,
) -> Result<(), ClocloError> {
    let profile = config
        .profiles
        .get(profile_name)
        .ok_or_else(|| ClocloError::ProfileNotFound(profile_name.to_string()))?;
    let token_file = token_file_for(profile)?;

    let token = match token {
        Some(t) => t,
        None => run_setup_token(config).await?,
    };

    let expanded = expand_tilde(token_file);
    if let Some(parent) = expanded.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&expanded, format!("{}\n", token.trim()))?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&expanded, std::fs::Permissions::from_mode(0o600))?;
    }

    println!(
        "{} Saved token for profile '{}' to {}",
        "->".bold().green(),
        profile_name.bold().cyan(),
        expanded.display()
    );

    Ok(())
}

/// Runs `claude setup-token`, mirroring its output to the terminal so the
/// user can complete the browser login, and tries to detect the token it
/// prints on success.
async fn run_setup_token(config: &ClocloConfig) -> Result<String, ClocloError> {
    let claude_bin = config
        .general
        .claude_bin
        .clone()
        .unwrap_or_else(|| std::path::PathBuf::from("claude"));

    println!(
        "{} Running `{} setup-token` — complete the browser login for the account you want on this profile.",
        "->".bold().green(),
        claude_bin.display()
    );

    let mut child = tokio::process::Command::new(&claude_bin)
        .arg("setup-token")
        .stdin(Stdio::inherit())
        .stdout(Stdio::piped())
        .stderr(Stdio::inherit())
        .spawn()
        .map_err(|e| ClocloError::Subprocess(format!("Failed to run `claude setup-token`: {}", e)))?;

    let stdout = child.stdout.take().ok_or_else(|| {
        ClocloError::Subprocess("Failed to capture `claude setup-token` output".to_string())
    })?;
    let mut lines = BufReader::new(stdout).lines();

    let mut detected_token: Option<String> = None;
    while let Some(line) = lines
        .next_line()
        .await
        .map_err(|e| ClocloError::Subprocess(format!("Failed reading setup-token output: {}", e)))?
    {
        println!("{}", line);
        if looks_like_token(line.trim()) {
            detected_token = Some(line.trim().to_string());
        }
    }

    let status = child
        .wait()
        .await
        .map_err(|e| ClocloError::Subprocess(format!("Wait failed: {}", e)))?;
    if !status.success() {
        return Err(ClocloError::Subprocess(format!(
            "`claude setup-token` exited with status: {}",
            status
        )));
    }

    detected_token.ok_or_else(|| {
        ClocloError::Auth(
            "Could not detect an OAuth token in `claude setup-token` output. \
             Copy it from above and rerun with `cloclo login <profile> --token <TOKEN>`."
                .to_string(),
        )
    })
}

/// Heuristic for spotting the bare token line among `claude setup-token`'s
/// other output (instructions, prompts, blank lines).
fn looks_like_token(line: &str) -> bool {
    if line.is_empty() || line.contains(' ') {
        return false;
    }
    if line.starts_with("sk-ant-") {
        return true;
    }
    line.len() >= 40
        && line
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
}
