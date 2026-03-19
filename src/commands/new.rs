use std::process::{Command, Stdio};

use anyhow::{bail, Context, Result};

use crate::client::RegistryClient;
use crate::commands::install::{check_platform, parse_spec, select_latest_version};
use crate::models::EpsManifest;

pub async fn run(client: &RegistryClient, spec: &str, dir: Option<&str>, force: bool) -> Result<()> {
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

    if pkg.is_epm_core() { crate::commands::guard_epm_core(name); }
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

    // Check git is available before doing anything
    if Command::new("git").arg("--version").stdout(Stdio::null()).stderr(Stdio::null()).status().is_err() {
        bail!("git is required but was not found.\nInstall it from https://git-scm.com/downloads and try again.");
    }

    println!("\x1b[2mCreating \x1b[0m\x1b[1m{dest}\x1b[0m\x1b[2m from {name}@{}...\x1b[0m", version.version);

    // Clone the harness
    let clone_ok = Command::new("git")
        .args(["clone", "--quiet", &version.git_url, dest])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .context("failed to run git clone")?
        .success();

    if !clone_ok {
        bail!("git clone failed — check your internet connection and try again.\nIf the problem persists, try: git clone {} {}", version.git_url, dest);
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

    // Block if the package is a maintained tool, not a customizable harness
    if !force {
        if let Ok(s) = std::fs::read_to_string(dest_path.join("eps.toml")) {
            if let Ok(m) = toml::from_str::<EpsManifest>(&s) {
                if m.eps.package_type.as_deref() == Some("tool") {
                    std::fs::remove_dir_all(dest_path).ok();
                    let alt = if m.mcp.binary.is_some() {
                        format!("epm mcp install {name}")
                    } else {
                        format!("epm install {name}")
                    };
                    bail!(
                        "'{name}' is a maintained tool, not a customizable harness.\n\
                         Use `{alt}` to install it instead.\n\
                         (Pass --force to scaffold from it anyway.)"
                    );
                }
            }
        }
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

    // Read the harness's eps.toml to check if it's a deployable service
    let is_service = std::fs::read_to_string(dest_path.join("eps.toml"))
        .ok()
        .and_then(|s| toml::from_str::<EpsManifest>(&s).ok())
        .map(|m| m.service.enabled)
        .unwrap_or(false);

    println!("\n\x1b[32m✓\x1b[0m Ready at \x1b[1m./{dest}/\x1b[0m");
    if is_service {
        println!("\n  \x1b[2mDeploy it:\x1b[0m");
        println!("    \x1b[36mcd {dest} && epc deploy\x1b[0m");
        println!("\n  \x1b[2mThen read\x1b[0m \x1b[1mCUSTOMIZE.md\x1b[0m \x1b[2mto make it yours.\x1b[0m");
    } else {
        println!("  \x1b[36mcd {dest} && cat CUSTOMIZE.md\x1b[0m");
    }

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
