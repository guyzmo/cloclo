use clap::{Parser, Subcommand};

/// Cloclo — le proxy magnifique for Claude Code.
///
/// Multi-profile authentication proxy and launcher.
#[derive(Debug, Parser)]
#[command(name = "cloclo", version, about)]
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
    Stop,

    /// Switch the active profile on a running proxy.
    Switch {
        /// Name of the profile to switch to.
        profile: String,
    },

    /// Show status of the running proxy.
    Status,

    /// List all configured profiles.
    Profiles,

    /// Launch Claude Code with a specific profile through the proxy.
    Launch {
        /// Profile to use.
        #[arg(short, long)]
        profile: Option<String>,

        /// Additional arguments passed through to Claude Code.
        #[arg(trailing_var_arg = true, allow_hyphen_values = true)]
        claude_args: Vec<String>,
    },
}
