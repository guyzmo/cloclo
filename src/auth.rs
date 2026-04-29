use std::path::Path;

use crate::config::{ClocloConfig, ProfileConfig, SecretSource};
use crate::error::ClocloError;
use crate::proxy::state::ResolvedAuth;

/// Default Anthropic API base URL.
const DEFAULT_BASE_URL: &str = "https://api.anthropic.com";

/// Resolves authentication credentials and upstream URL for the given profile name.
///
/// Returns `(ResolvedAuth, upstream_url)`.
pub fn resolve_profile(
    config: &ClocloConfig,
    profile_name: &str,
) -> Result<(ResolvedAuth, String), ClocloError> {
    let profile = config
        .profiles
        .get(profile_name)
        .ok_or_else(|| ClocloError::ProfileNotFound(profile_name.to_string()))?;

    match profile {
        ProfileConfig::ApiKey {
            api_key, base_url, ..
        } => {
            let key = resolve_secret(api_key)?;
            let url = base_url
                .clone()
                .unwrap_or_else(|| DEFAULT_BASE_URL.to_string());
            Ok((ResolvedAuth::ApiKey(key), url))
        }
        ProfileConfig::OAuth {
            token_file,
            base_url,
            ..
        } => {
            let token = read_token_file(token_file)?;
            let url = base_url
                .clone()
                .unwrap_or_else(|| DEFAULT_BASE_URL.to_string());
            Ok((ResolvedAuth::BearerToken(token), url))
        }
        ProfileConfig::EnterpriseSso {
            token_file,
            base_url,
            ..
        } => {
            let token = read_token_file(token_file)?;
            let url = base_url
                .clone()
                .unwrap_or_else(|| DEFAULT_BASE_URL.to_string());
            Ok((ResolvedAuth::BearerToken(token), url))
        }
        ProfileConfig::Proxy {
            upstream_url,
            auth_token,
            ..
        } => {
            let token = match auth_token {
                Some(source) => Some(resolve_secret(source)?),
                None => None,
            };
            Ok((
                ResolvedAuth::Passthrough { token },
                upstream_url.clone(),
            ))
        }
    }
}

/// Reads a bearer/OAuth token from a file, trimming whitespace.
pub fn read_token_file(path: &Path) -> Result<String, ClocloError> {
    let contents = std::fs::read_to_string(path).map_err(|e| {
        ClocloError::Auth(format!("Failed to read token file {}: {}", path.display(), e))
    })?;
    Ok(contents.trim().to_string())
}

/// Resolves a `SecretSource` to its plaintext value.
pub fn resolve_secret(source: &SecretSource) -> Result<String, ClocloError> {
    match source {
        SecretSource::Literal(value) => Ok(value.clone()),
        SecretSource::File { file } => read_token_file(file),
        SecretSource::Env { env } => std::env::var(env).map_err(|e| {
            ClocloError::Auth(format!("Environment variable '{}' not set: {}", env, e))
        }),
    }
}
