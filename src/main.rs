mod client;
mod commands;
mod installed;
mod models;
mod update_check;

use anyhow::Result;
use clap::{Parser, Subcommand};

use client::RegistryClient;
use commands::mcp::McpCommands;
use commands::runtime::RuntimeCommands;
use commands::skills::SkillsCommands;

const REGISTRY: &str = "https://epm.dev";

#[derive(Parser)]
#[command(name = "epm", version, about = "Extremely Personal Manager. A package manager for extremely personal software.")]
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
    /// Create a new project from an EPS harness
    New {
        /// Harness name (e.g. todo, crm) — optionally with @version
        spec: String,
        /// Directory name to create (defaults to the harness name)
        dir: Option<String>,
        /// Scaffold from a maintained tool package anyway (advanced)
        #[arg(long)]
        force: bool,
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
    /// Install, list, and remove Claude Code skill packages
    Skills {
        #[command(subcommand)]
        command: SkillsCommands,
    },
    /// Update epm to the latest version
    SelfUpdate,
    /// Remove epm and everything it installed (MCP servers, skills, packages)
    SelfUninstall {
        /// Skip confirmation prompt
        #[arg(long)]
        yes: bool,
        /// Do not remove the epm binary (useful for testing)
        #[arg(long, hide = true)]
        keep_binary: bool,
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
        Commands::New { spec, dir, force } => {
            commands::new::run(&client, spec, dir.as_deref(), *force).await?;
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
        Commands::Skills { command } => {
            commands::skills::run(command, &client).await?;
        }
        Commands::SelfUpdate => {
            commands::self_update::run().await?;
        }
        Commands::SelfUninstall { yes, keep_binary } => {
            commands::self_uninstall::run(*yes, *keep_binary)?;
        }
    }

    // Skip update check after self-uninstall — ~/.epm/ was just removed and
    // update_check would recreate it via record_check().
    if !matches!(&cli.command, Commands::SelfUninstall { .. }) {
        update_check::check_and_warn().await;
    }

    Ok(())
}
