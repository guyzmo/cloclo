use std::sync::Arc;
use tokio::sync::RwLock;

use crate::config::ClocloConfig;

/// The shared application state — named after Alexandrie, Alexandra,
/// Claude Francois' most famous song.
pub type Alexandrie = Arc<RwLock<ProxyState>>;

/// Core proxy state, protected behind an `RwLock`.
pub struct ProxyState {
    /// Name of the currently active profile.
    pub active_profile: String,
    /// Resolved authentication credentials for the active profile.
    pub active_auth: ResolvedAuth,
    /// The upstream URL to forward requests to.
    pub upstream_url: String,
    /// If set, overrides the model field in request bodies before forwarding.
    pub model_override: Option<String>,
    /// The port this proxy session is listening on.
    pub port: u16,
    /// The full configuration (for profile switching).
    pub config: ClocloConfig,
    /// HTTP client reused across requests.
    pub client: reqwest::Client,
    /// A managed subprocess, if the active profile requires one.
    pub managed_subprocess: Option<ManagedChild>,
    /// Runtime statistics.
    pub stats: SessionStats,
}

/// Resolved authentication — ready to inject into outgoing requests.
#[derive(Debug, Clone)]
pub enum ResolvedAuth {
    /// Anthropic API key, sent as `x-api-key` header.
    ApiKey(String),
    /// Bearer token (OAuth / SSO), sent as `Authorization: Bearer <token>`.
    BearerToken(String),
    /// Passthrough to an upstream proxy, optionally with a token.
    Passthrough { token: Option<String> },
}

/// A managed child process (e.g., a local LLM proxy).
pub struct ManagedChild {
    pub child: tokio::process::Child,
    pub profile_name: String,
    pub port: u16,
}

/// Runtime statistics for the proxy session.
#[derive(Debug, Clone, Default)]
pub struct SessionStats {
    pub requests_forwarded: u64,
    pub profile_switches: u64,
    pub started_at: Option<std::time::Instant>,
}
