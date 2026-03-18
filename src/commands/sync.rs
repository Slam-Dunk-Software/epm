use std::process::{Command, Stdio};

use anyhow::{bail, Context, Result};

use crate::client::RegistryClient;
use crate::commands::adopt::{vendor_dir, write_adoption_record, read_adoption_record};
use crate::commands::install::select_latest_version;

pub async fn run(client: &RegistryClient, name: &str, wipe: bool) -> Result<()> {
    let vendor_path = vendor_dir(name)?;

    if !vendor_path.exists() {
        bail!("vendor/{name} has not been adopted — run `epm adopt {name}` first");
    }

    let adopted_toml = vendor_path.join(".adopted.toml");
    if !adopted_toml.exists() {
        bail!(
            "vendor/{name}/.adopted.toml not found — this package was not adopted via epm adopt"
        );
    }

    let record = read_adoption_record(&vendor_path)?;
    let pkg = client.get_package(name).await?;
    if pkg.is_epm_core() { crate::commands::guard_epm_core(name); }
    let latest = select_latest_version(pkg.versions)
        .ok_or_else(|| anyhow::anyhow!("no non-yanked versions available for '{name}'"))?;

    if wipe {
        println!("Wiping vendor/{name}/ and re-fetching {name}@{} ...", latest.version);
        std::fs::remove_dir_all(&vendor_path).context("failed to remove vendor directory")?;

        let vendor_str = vendor_path
            .to_str()
            .context("vendor path contains non-UTF-8 characters")?;

        let clone_status = Command::new("git")
            .args(["clone", &latest.git_url, vendor_str])
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .status()
            .context("failed to run git clone")?;

        if !clone_status.success() {
            bail!("git clone failed");
        }

        let checkout_status = Command::new("git")
            .args(["-C", vendor_str, "checkout", &latest.commit_sha])
            .stdout(Stdio::null())
            .stderr(Stdio::inherit())
            .status()
            .context("failed to run git checkout")?;

        if !checkout_status.success() {
            bail!("git checkout {} failed", latest.commit_sha);
        }

        write_adoption_record(
            &vendor_path,
            name,
            &latest.git_url,
            &latest.version,
            &latest.commit_sha,
        )?;

        println!("Re-adopted {name}@{} into vendor/{name}/", latest.version);
        println!("Time to vibe it up again.");
    } else {
        let short = |s: &str| s[..s.len().min(8)].to_string();

        if record.adoption.adopted_commit == latest.commit_sha {
            println!(
                "{name} is up to date ({}@{})",
                latest.version,
                short(&latest.commit_sha)
            );
        } else {
            println!("{name} has upstream changes:");
            println!(
                "  adopted:  {}@{}",
                record.adoption.adopted_version,
                short(&record.adoption.adopted_commit)
            );
            println!(
                "  upstream: {}@{}",
                latest.version,
                short(&latest.commit_sha)
            );
            println!();
            println!("Run `epm sync {name} --wipe` to replace with fresh upstream.");
            println!("Warning: this will discard your local vibe work — re-vibe after.");
        }
    }

    Ok(())
}
