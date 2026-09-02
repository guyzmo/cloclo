use clap::{Parser, Subcommand};

/// Cloclo — le proxy magnifique for Claude Code.
///
/// Multi-profile authentication proxy and launcher.
#[derive(Debug, Parser)]
#[command(
    name = "cloclo",
    version,
    about,
    long_about = "cloclo — multi-profile proxy for Claude Code\n\
        \n\
        Manage personal and enterprise claude.ai accounts (OAuth tokens),\n\
        and third-party proxies (copilot-api, etc.) from one config.\n\
        Each `cloclo launch` spawns its own proxy on a random port.\n\
        Switch profiles or models mid-session with `cloclo sw` / `cloclo m`."
)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Commands,
}

#[derive(Debug, Subcommand)]
pub enum Commands {
    /// Initialize a default configuration file.
    Init {
        /// Overwrite existing config if present.
        #[arg(long)]
        force: bool,
    },

    /// Start the proxy server.
    #[command(alias = "up")]
    Start {
        /// Profile to activate on startup.
        #[arg(short, long)]
        profile: Option<String>,

        /// Port to listen on (overrides config).
        #[arg(short = 'P', long)]
        port: Option<u16>,

        /// Run in the foreground instead of daemonizing.
        #[arg(long)]
        foreground: bool,
    },

    /// Stop a running proxy daemon.
    #[command(alias = "down")]
    Stop,

    /// Switch the active profile on a running proxy.
    #[command(alias = "sw")]
    Switch {
        /// Name of the profile to switch to.
        profile: String,
    },

    /// Show status of the running proxy.
    Status,

    /// List all configured profiles.
    #[command(alias = "ls")]
    Profiles,

    /// Launch Claude Code with a per-session proxy.
    #[command(alias = "go")]
    Launch {
        /// Profile to use.
        #[arg(short, long)]
        profile: Option<String>,

        /// Additional arguments passed through to Claude Code.
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        claude_args: Vec<String>,
    },

    /// Set or clear the model override on the running proxy.
    ///
    /// Use from within a Claude Code session to switch models on the fly.
    /// Run with no argument to clear the override.
    #[command(alias = "m")]
    Model {
        /// Model name (e.g. "claude-opus-4-6"). Omit to clear the override.
        model: Option<String>,
    },
}
