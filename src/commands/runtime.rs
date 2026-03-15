use anyhow::Result;
use clap::Subcommand;

use crate::client::RegistryClient;
use crate::commands::{install, upgrade};
use crate::commands::list::list_installed_versions;

/// The two first-class EPS runtime packages.
const RUNTIME_PACKAGES: &[(&str, &str)] = &[
    ("epc",         "Personal cloud runtime — manages your EPS services"),
    ("observatory", "Health monitoring dashboard for EPC services"),
];

#[derive(Subcommand)]
pub enum RuntimeCommands {
    /// Install epc and observatory (bootstrap the EPS runtime)
    Install {
        /// Only install a specific runtime package (epc or observatory)
        #[arg(value_name = "PACKAGE")]
        package: Option<String>,
    },
    /// Upgrade epc and observatory to the latest registry versions
    Upgrade {
        /// Only upgrade a specific runtime package (epc or observatory)
        #[arg(value_name = "PACKAGE")]
        package: Option<String>,
    },
    /// Show current status of the EPS runtime packages
    Status,
}

pub async fn run(cmd: &RuntimeCommands, client: &RegistryClient) -> Result<()> {
    match cmd {
        RuntimeCommands::Install { package } => run_install(client, package.as_deref()).await,
        RuntimeCommands::Upgrade { package } => run_upgrade(client, package.as_deref()).await,
        RuntimeCommands::Status => run_status(client).await,
    }
}

async fn run_install(client: &RegistryClient, only: Option<&str>) -> Result<()> {
    let targets = filter_targets(only)?;
    println!("EPS Runtime — installing {} package(s)\n", targets.len());

    for (name, desc) in &targets {
        let installed = installed_versions(name);
        if !installed.is_empty() {
            println!("  ✓ {name} — already installed ({})", installed.join(", "));
        } else {
            println!("  ↓ {name} — {desc}");
            install::run(client, name).await?;
        }
    }

    println!();
    print_next_steps();
    Ok(())
}

async fn run_upgrade(client: &RegistryClient, only: Option<&str>) -> Result<()> {
    let targets = filter_targets(only)?;
    println!("EPS Runtime — upgrading {} package(s)\n", targets.len());

    for (name, _) in &targets {
        println!("  ↑ {name}");
        upgrade::run(client, name).await?;
    }

    println!("\nDone. Restart any running services to pick up new binaries.");
    Ok(())
}

async fn run_status(client: &RegistryClient) -> Result<()> {
    println!("EPS Runtime\n");

    for (name, desc) in RUNTIME_PACKAGES {
        let installed = installed_versions(name);
        let local = if installed.is_empty() {
            "not installed".to_string()
        } else {
            installed.last().unwrap().clone()
        };

        let latest = match client.get_package(name).await {
            Ok(pkg) => pkg
                .versions
                .iter()
                .filter(|v| !v.yanked)
                .map(|v| v.version.clone())
                .last()
                .unwrap_or_else(|| "unknown".into()),
            Err(_) => "unavailable".into(),
        };

        let status = if local == "not installed" {
            format!("✗ not installed  (latest: {latest})")
        } else if local == latest || latest == "unavailable" {
            format!("✓ {local}")
        } else {
            format!("↑ {local}  →  {latest} available  (run: epm runtime upgrade {name})")
        };

        println!("  {name:<14} {status}");
        println!("  {:<14} {desc}\n", "");
    }

    Ok(())
}

fn installed_versions(name: &str) -> Vec<String> {
    let pkg_root = match dirs::home_dir() {
        Some(h) => h.join(".epm").join("packages").join(name),
        None => return vec![],
    };
    list_installed_versions(&pkg_root).unwrap_or_default()
}

fn filter_targets(only: Option<&str>) -> Result<Vec<(&'static str, &'static str)>> {
    match only {
        None => Ok(RUNTIME_PACKAGES.to_vec()),
        Some(name) => RUNTIME_PACKAGES
            .iter()
            .find(|(n, _)| *n == name)
            .map(|&t| vec![t])
            .ok_or_else(|| anyhow::anyhow!(
                "unknown runtime package '{name}'. Valid options: {}",
                RUNTIME_PACKAGES.iter().map(|(n, _)| *n).collect::<Vec<_>>().join(", ")
            )),
    }
}

fn print_next_steps() {
    println!("Next steps:");
    println!("  epc ps                    — list running services");
    println!("  epc deploy observatory    — start the monitoring dashboard");
    println!("  observatory tui           — open the terminal UI");
}
