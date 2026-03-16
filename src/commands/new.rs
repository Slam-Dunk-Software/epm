use std::process::{Command, Stdio};

use anyhow::{bail, Context, Result};

use crate::client::RegistryClient;
use crate::commands::install::{check_platform, parse_spec, select_latest_version};

pub async fn run(client: &RegistryClient, spec: &str, dir: Option<&str>) -> Result<()> {
    let (name, pinned_version) = parse_spec(spec);

    let pkg = match client.get_package(name).await {
        Ok(p) => p,
        Err(e) if e.to_string().contains("not found") => {
            eprintln!("error: package '{name}' not found");
            suggest_typo(client, name).await;
            std::process::exit(1);
        }
        Err(e) => return Err(e),
    };

    check_platform(&pkg.platforms, name)?;

    let version = if let Some(ver) = pinned_version {
        pkg.versions
            .into_iter()
            .find(|v| v.version == ver)
            .ok_or_else(|| anyhow::anyhow!("version '{ver}' of '{name}' not found"))?
    } else {
        select_latest_version(pkg.versions)
            .ok_or_else(|| anyhow::anyhow!("no installable versions available for '{name}'"))?
    };

    let dest = dir.unwrap_or(name);
    let dest_path = std::path::Path::new(dest);

    if dest_path.exists() {
        bail!("destination '{}' already exists", dest_path.display());
    }

    println!("Creating {dest} from {name}@{} ...", version.version);

    // Clone the harness
    let clone_ok = Command::new("git")
        .args(["clone", "--quiet", &version.git_url, dest])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .context("failed to run git clone")?
        .success();

    if !clone_ok {
        bail!("git clone failed");
    }

    // Checkout the exact published commit (suppress detached HEAD advice)
    let checkout_ok = Command::new("git")
        .args(["-C", dest, "-c", "advice.detachedHead=false", "checkout", &version.commit_sha])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .context("failed to run git checkout")?
        .success();

    if !checkout_ok {
        bail!("git checkout {} failed", version.commit_sha);
    }

    // Strip upstream history — fresh slate
    std::fs::remove_dir_all(dest_path.join(".git"))
        .context("failed to remove upstream .git")?;

    // New git repo
    Command::new("git")
        .args(["init", dest])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;

    Command::new("git")
        .args(["-C", dest, "add", "."])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;

    Command::new("git")
        .args([
            "-C", dest,
            "commit", "-m",
            &format!("Initial commit (epm new {name}@{})", version.version),
        ])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;

    client.track_install(name, &version.version).await;

    println!("\n✓ Ready at ./{dest}/");
    println!("  cd {dest} && cat CUSTOMIZE.md");

    Ok(())
}

async fn suggest_typo(client: &RegistryClient, name: &str) {
    let Ok(packages) = client.list_packages().await else { return };

    let best = packages
        .iter()
        .map(|p| (strsim::levenshtein(name, &p.name), &p.name))
        .filter(|(d, _)| *d <= 2)
        .min_by_key(|(d, _)| *d);

    if let Some((_, suggestion)) = best {
        eprintln!("\n  Did you mean '{suggestion}'?");
    }
}
