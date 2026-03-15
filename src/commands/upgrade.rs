use anyhow::{anyhow, Context, Result};

use crate::client::RegistryClient;
use crate::commands::install::{check_platform, install_version, select_latest_version};
use crate::commands::list::list_installed_versions;

pub async fn run(client: &RegistryClient, name: &str) -> Result<()> {
    let pkg = client.get_package(name).await?;
    check_platform(&pkg.platforms, name)?;
    let latest = select_latest_version(pkg.versions)
        .ok_or_else(|| anyhow!("no non-yanked versions for '{name}'"))?;

    let install_root = dirs::home_dir()
        .context("could not determine home directory")?
        .join(".epm")
        .join("packages")
        .join(name);

    let target = install_root.join(&latest.version);

    if target.exists() {
        println!("{name}@{} is already up to date", latest.version);
        return Ok(());
    }

    if let Ok(installed) = list_installed_versions(&install_root) {
        if !installed.is_empty() {
            println!(
                "Upgrading {name}: {} → {}",
                installed.join(", "),
                latest.version
            );
        }
    }

    install_version(client, name, &latest).await
}
