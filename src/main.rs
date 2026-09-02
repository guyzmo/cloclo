use clap::Parser;
use colored::Colorize;
use tracing_subscriber::EnvFilter;

use cloclo::cli::{Cli, Commands, DesktopCommands};
use cloclo::config::{config_path, init_config, load_config};
use cloclo::daemon::{pid_file_path, start_daemon, stop_daemon};
use cloclo::desktop::{launch_desktop, list_desktop_profiles};
use cloclo::launch::{launch_claude, list_profiles, set_model_cli, show_status, switch_profile_cli};
use cloclo::login::login;
use cloclo::proxy::server;

#[tokio::main]
async fn main() {
    // Initialize tracing.
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env())
        .init();

    if let Err(e) = run().await {
        eprintln!("{} {}", "Error:".bold().red(), e);
        std::process::exit(1);
    }
}

async fn run() -> anyhow::Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Init { force } => {
            init_config(force).map_err(anyhow::Error::from)?;
            let path = config_path();
            println!(
                "{} Configuration created at {}",
                "->".bold().green(),
                path.display()
            );
            println!("  Edit the file to configure your profiles, then run:");
            println!("  {} to start the proxy", "cloclo start".bold());
            println!("  {} to launch Claude", "cloclo launch".bold());
            Ok(())
        }

        Commands::Start {
            profile,
            port,
            foreground,
        } => {
            let config = load_config().map_err(anyhow::Error::from)?;
            let profile_name = profile
                .as_deref()
                .unwrap_or(&config.general.default_profile)
                .to_string();
            let port = port.unwrap_or(config.general.port);

            if foreground {
                server::run(config, &profile_name, port)
                    .await
                    .map_err(anyhow::Error::from)?;
            } else {
                let pid_path = pid_file_path(config.general.pid_file.as_ref());
                start_daemon(Some(&profile_name), Some(port), &pid_path)
                    .map_err(anyhow::Error::from)?;
            }
            Ok(())
        }

        Commands::Stop => {
            let config = load_config().map_err(anyhow::Error::from)?;
            let pid_path = pid_file_path(config.general.pid_file.as_ref());
            stop_daemon(&pid_path).await.map_err(anyhow::Error::from)?;
            Ok(())
        }

        Commands::Login { profile, token } => {
            let config = load_config().map_err(anyhow::Error::from)?;
            login(&config, &profile, token).await.map_err(anyhow::Error::from)?;
            Ok(())
        }

        Commands::Switch { profile } => {
            switch_profile_cli(&profile).await.map_err(anyhow::Error::from)?;
            Ok(())
        }

        Commands::Status => {
            show_status().await.map_err(anyhow::Error::from)?;
            Ok(())
        }

        Commands::Profiles => {
            let config = load_config().map_err(anyhow::Error::from)?;
            list_profiles(&config);
            Ok(())
        }

        Commands::Launch {
            profile,
            claude_args,
        } => {
            launch_claude(profile.as_deref(), &claude_args).await.map_err(anyhow::Error::from)?;
            Ok(())
        }

        Commands::Model { model } => {
            set_model_cli(model.as_deref()).await.map_err(anyhow::Error::from)?;
            Ok(())
        }

        Commands::Desktop(DesktopCommands::Launch { profile }) => {
            let config = load_config().map_err(anyhow::Error::from)?;
            launch_desktop(&config, &profile).await.map_err(anyhow::Error::from)?;
            Ok(())
        }

        Commands::Desktop(DesktopCommands::List) => {
            let config = load_config().map_err(anyhow::Error::from)?;
            list_desktop_profiles(&config);
            Ok(())
        }
    }
}
