use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{bail, Context, Result};

use crate::client::RegistryClient;
use crate::commands::install::{parse_spec, select_latest_version};
use crate::models::{AdoptionMeta, AdoptionRecord, Version};

pub fn vendor_dir(name: &str) -> Result<PathBuf> {
    Ok(std::env::current_dir()
        .context("could not determine current directory")?
        .join("vendor")
        .join(name))
}

pub fn write_adoption_record(
    vendor_path: &Path,
    name: &str,
    git_url: &str,
    version: &str,
    commit_sha: &str,
) -> Result<()> {
    let record = AdoptionRecord {
        adoption: AdoptionMeta {
            name: name.to_string(),
            upstream_git_url: git_url.to_string(),
            adopted_version: version.to_string(),
            adopted_commit: commit_sha.to_string(),
        },
    };
    let toml_str = toml::to_string_pretty(&record).context("failed to serialize .adopted.toml")?;
    std::fs::write(vendor_path.join(".adopted.toml"), toml_str)
        .context("failed to write .adopted.toml")
}

pub fn read_adoption_record(vendor_path: &Path) -> Result<AdoptionRecord> {
    let path = vendor_path.join(".adopted.toml");
    let raw = std::fs::read_to_string(&path)
        .with_context(|| format!("could not read '{}'", path.display()))?;
    toml::from_str(&raw).with_context(|| format!("failed to parse '{}'", path.display()))
}

pub async fn run(client: &RegistryClient, spec: &str) -> Result<()> {
    let (name, pinned_version) = parse_spec(spec);

    let vendor_path = vendor_dir(name)?;
    if vendor_path.exists() {
        bail!("vendor/{name} already exists — use `epm sync {name}` to check for upstream changes");
    }

    let pkg = client.get_package(name).await?;
    let version: Version = if let Some(ver) = pinned_version {
        pkg.versions
            .into_iter()
            .find(|v| v.version == ver)
            .ok_or_else(|| anyhow::anyhow!("version '{ver}' of package '{name}' not found"))?
    } else {
        select_latest_version(pkg.versions)
            .ok_or_else(|| anyhow::anyhow!("no non-yanked versions available for '{name}'"))?
    };

    let vendor_str = vendor_path
        .to_str()
        .context("vendor path contains non-UTF-8 characters")?;

    println!("Adopting {name}@{} into vendor/{name}/ ...", version.version);

    let clone_status = Command::new("git")
        .args(["clone", &version.git_url, vendor_str])
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .status()
        .context("failed to run git clone")?;

    if !clone_status.success() {
        bail!("git clone failed");
    }

    let checkout_status = Command::new("git")
        .args(["-C", vendor_str, "checkout", &version.commit_sha])
        .stdout(Stdio::null())
        .stderr(Stdio::inherit())
        .status()
        .context("failed to run git checkout")?;

    if !checkout_status.success() {
        std::fs::remove_dir_all(&vendor_path).ok();
        bail!("git checkout {} failed", version.commit_sha);
    }

    // Gate: only EPS packages (with eps.toml) can be adopted
    if !vendor_path.join("eps.toml").exists() {
        std::fs::remove_dir_all(&vendor_path).ok();
        bail!("'{name}' does not have an eps.toml — only EPS packages can be adopted");
    }

    write_adoption_record(
        &vendor_path,
        name,
        &version.git_url,
        &version.version,
        &version.commit_sha,
    )?;

    println!("Adopted {name}@{} into vendor/{name}/", version.version);
    println!("The source is now yours — vibe it up.");
    println!("Run `epm sync {name}` to check for upstream changes later.");

    Ok(())
}
