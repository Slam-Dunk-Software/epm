pub mod audit;
pub mod install_startup;
pub mod logs;
pub mod observatory;
pub mod prune;
pub mod ps;
pub mod remove;
pub mod restart;
pub mod start;
pub mod startup;
pub mod stop;
pub mod sync;

use anyhow::Result;
use clap::Subcommand;

#[derive(Subcommand)]
pub enum ServicesCommands {
    /// Start an EPS service from a project directory (or installed package).
    /// Run from inside a project directory with no arguments to serve it locally.
    Start {
        /// Package name (looks up in ~/.epm/packages/). Omit when inside a project
        /// directory — epm will detect the eps.toml and serve it automatically.
        spec: Option<String>,
        /// Path to a local EPS directory (skips epm lookup).
        /// Defaults to the current directory if it contains an eps.toml.
        #[arg(long)]
        local: Option<std::path::PathBuf>,
    },
    /// List running services with their ports and Tailscale URLs
    Ps,
    /// Tail logs for a running service
    Logs {
        /// Service name
        name: String,
    },
    /// Stop a running service
    Stop {
        /// Service name
        name: String,
    },
    /// Fully remove a service: stop it, delete its log, and purge it from the Observatory database
    Remove {
        /// Service name
        name: String,
    },
    /// Remove all services whose project directory no longer exists
    Prune,
    /// Repair services.toml from the persistent registry.
    /// Re-registers any service in ~/.epm/services/registry.toml that is currently
    /// running (port listening) but missing from services.toml.
    Sync,
    /// Stop and restart a running service (picks up source changes)
    Restart {
        /// Service name
        name: String,
    },
    /// Restart all registered services that are not already running.
    /// Waits for Tailscale to be ready before deploying.
    Startup,
    /// Install a macOS LaunchAgent so services restart automatically on login.
    /// Creates ~/Library/LaunchAgents/com.eps.epm-startup.plist and loads it.
    /// macOS only.
    #[cfg(target_os = "macos")]
    InstallStartup,
    /// Check all running services for insecure network bindings
    Audit,
    /// Manage the Observatory monitoring database
    Observatory {
        #[command(subcommand)]
        command: ObservatoryCommands,
    },
}

#[derive(Subcommand)]
pub enum ObservatoryCommands {
    /// Remove one or more stale service entries from the Observatory database.
    Rm {
        /// One or more service names to remove
        #[arg(required = true)]
        names: Vec<String>,
    },
}

pub async fn run(cmd: &ServicesCommands) -> Result<()> {
    match cmd {
        ServicesCommands::Start { spec, local } => {
            start::run(spec.as_deref(), local.as_deref()).await?
        }
        ServicesCommands::Ps => ps::run().await?,
        ServicesCommands::Logs { name } => logs::run(name).await?,
        ServicesCommands::Stop { name } => stop::run(name)?,
        ServicesCommands::Remove { name } => remove::run(name)?,
        ServicesCommands::Prune => prune::run()?,
        ServicesCommands::Sync => sync::run()?,
        ServicesCommands::Restart { name } => restart::run(name).await?,
        ServicesCommands::Startup => startup::run().await?,
        #[cfg(target_os = "macos")]
        ServicesCommands::InstallStartup => install_startup::run()?,
        ServicesCommands::Audit => audit::run().await?,
        ServicesCommands::Observatory { command } => match command {
            ObservatoryCommands::Rm { names } => observatory::run(names)?,
        },
    }
    Ok(())
}
