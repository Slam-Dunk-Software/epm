mod client;
mod commands;
mod models;

use anyhow::Result;
use clap::{Parser, Subcommand};

use client::RegistryClient;
use commands::mcp::McpCommands;
use commands::runtime::RuntimeCommands;

const REGISTRY: &str = "https://epm.dev";

#[derive(Parser)]
#[command(name = "epm", about = "Extremely Personal Manager — EPS registry client")]
struct Cli {
    /// Publish token (overrides EPM_PUBLISH_TOKEN env var)
    #[arg(long, global = true)]
    token: Option<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// Search for packages (lists all if no query given)
    Search {
        /// Optional search query (filters name and description)
        query: Option<String>,
    },
    /// Show details for a package
    Info {
        /// Package name
        name: String,
    },
    /// Install a package (use name@version to pin a specific version)
    Install {
        /// Package spec: <name> or <name@version>
        spec: String,
    },
    /// List all locally installed packages
    List,
    /// Uninstall a package (use name@version to target a specific version)
    Uninstall {
        /// Package spec: <name> or <name@version>
        spec: String,
    },
    /// Upgrade a package to the latest version from the registry
    Upgrade {
        /// Package name
        name: String,
    },
    /// Publish a package to the registry from an eps.toml manifest
    Publish {
        /// Path to eps.toml manifest
        #[arg(long, default_value = "eps.toml")]
        manifest: std::path::PathBuf,
    },
    /// Scaffold a new EPS package in a new directory
    Init {
        /// Package name (a directory with this name will be created)
        name: String,
        /// Short description written into eps.toml
        #[arg(long, short = 'd')]
        description: Option<String>,
        /// Skip running git init
        #[arg(long)]
        no_git: bool,
    },
    /// Adopt an EPS package into vendor/<name>/ as first-class source code
    Adopt {
        /// Package spec: <name> or <name@version>
        spec: String,
    },
    /// Check for upstream changes on an adopted package
    Sync {
        /// Package name
        name: String,
        /// Replace vendor copy with fresh upstream (discards local vibe work)
        #[arg(long)]
        wipe: bool,
    },
    /// Manage the EPS runtime (epc + observatory)
    Runtime {
        #[command(subcommand)]
        command: RuntimeCommands,
    },
    /// Install, list, and remove MCP servers in ~/.claude.json
    Mcp {
        #[command(subcommand)]
        command: McpCommands,
    },
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();
    let token = cli.token
        .clone()
        .or_else(|| std::env::var("EPM_PUBLISH_TOKEN").ok());
    let registry = std::env::var("EPM_REGISTRY").unwrap_or_else(|_| REGISTRY.to_string());
    let client = RegistryClient::new(&registry, token);

    match &cli.command {
        Commands::Search { query } => {
            commands::search::run(&client, query.as_deref()).await?;
        }
        Commands::Info { name } => {
            commands::info::run(&client, name).await?;
        }
        Commands::Install { spec } => {
            commands::install::run(&client, spec).await?;
        }
        Commands::List => {
            commands::list::run()?;
        }
        Commands::Uninstall { spec } => {
            commands::uninstall::run(spec)?;
        }
        Commands::Upgrade { name } => {
            commands::upgrade::run(&client, name).await?;
        }
        Commands::Publish { manifest } => {
            commands::publish::run(&client, manifest).await?;
        }
        Commands::Init { name, description, no_git } => {
            commands::init::run(name, description.as_deref(), *no_git)?;
        }
        Commands::Adopt { spec } => {
            commands::adopt::run(&client, spec).await?;
        }
        Commands::Sync { name, wipe } => {
            commands::sync::run(&client, name, *wipe).await?;
        }
        Commands::Runtime { command } => {
            commands::runtime::run(command, &client).await?;
        }
        Commands::Mcp { command } => {
            commands::mcp::run(command, &client).await?;
        }
    }

    Ok(())
}
