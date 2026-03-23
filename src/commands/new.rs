use std::process::{Command, Stdio};

use anyhow::{bail, Context, Result};

use crate::client::RegistryClient;
use crate::commands::install::{check_platform, parse_spec, select_latest_version};
use crate::models::EpsManifest;

fn epm_home() -> Result<std::path::PathBuf> {
    if let Ok(val) = std::env::var("EPM_HOME") {
        return Ok(std::path::PathBuf::from(val));
    }
    let home = dirs::home_dir().context("could not determine home directory")?;
    Ok(home.join("eps"))
}

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

    let dest_name = dir.unwrap_or(name);
    let dest_path = if std::path::Path::new(dest_name).is_absolute() {
        std::path::PathBuf::from(dest_name)
    } else {
        let home = epm_home()?;
        std::fs::create_dir_all(&home)
            .with_context(|| format!("failed to create EPM_HOME directory at {}", home.display()))?;
        home.join(dest_name)
    };

    if dest_path.exists() {
        bail!("destination '{}' already exists", dest_path.display());
    }

    // Check git is available before doing anything
    if Command::new("git").arg("--version").stdout(Stdio::null()).stderr(Stdio::null()).status().is_err() {
        bail!("git is required but was not found.\nInstall it from https://git-scm.com/downloads and try again.");
    }

    let dest_str = dest_path.to_string_lossy().into_owned();

    println!("\x1b[2mCreating \x1b[0m\x1b[1m{dest_name}\x1b[0m\x1b[2m from {name}@{}...\x1b[0m", version.version);

    // Clone the harness
    let clone_ok = Command::new("git")
        .args(["clone", "--quiet", &version.git_url, &dest_str])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .context("failed to run git clone")?
        .success();

    if !clone_ok {
        bail!("git clone failed — check your internet connection and try again.\nIf the problem persists, try: git clone {} {}", version.git_url, dest_str);
    }

    // Checkout the exact published commit (suppress detached HEAD advice)
    let checkout_ok = Command::new("git")
        .args(["-C", &dest_str, "-c", "advice.detachedHead=false", "checkout", &version.commit_sha])
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
                    std::fs::remove_dir_all(&dest_path).ok();
                    let alt = format!("epm install {name}");
                    bail!(
                        "'{name}' is a maintained tool, not a customizable harness.\n\
                         Use `{alt}` to install it instead.\n\
                         (Pass --force to scaffold from it anyway.)"
                    );
                }
            }
        }
    }

    // If the user gave a custom destination name, update the eps.toml package name to match.
    // e.g. `epm new shell seeing_stone` → name = "seeing_stone" not "shell"
    if dest_name != name {
        let toml_path = dest_path.join("eps.toml");
        if let Ok(contents) = std::fs::read_to_string(&toml_path) {
            let updated = contents
                .lines()
                .map(|line| {
                    let trimmed = line.trim();
                    if trimmed.starts_with("name") {
                        if let Some(rest) = trimmed.strip_prefix("name") {
                            let after_eq = rest.trim_start_matches(|c: char| c.is_whitespace() || c == '=').trim();
                            if after_eq.starts_with('"') {
                                let indent = &line[..line.len() - line.trim_start().len()];
                                return format!("{indent}name        = \"{dest_name}\"");
                            }
                        }
                    }
                    line.to_string()
                })
                .collect::<Vec<_>>()
                .join("\n");
            let _ = std::fs::write(&toml_path, updated);
        }
    }

    // Strip upstream history — fresh slate
    std::fs::remove_dir_all(dest_path.join(".git"))
        .context("failed to remove upstream .git")?;

    // New git repo
    Command::new("git")
        .args(["init", &dest_str])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;

    Command::new("git")
        .args(["-C", &dest_str, "add", "."])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()?;

    Command::new("git")
        .args([
            "-C", &dest_str,
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
        .map(|m| m.service.map(|s| s.enabled).unwrap_or(false))
        .unwrap_or(false);

    println!("\n\x1b[32m✓\x1b[0m Ready at \x1b[1m{dest_str}/\x1b[0m");
    if is_service {
        println!("\n  \x1b[2mDeploy it:\x1b[0m");
        println!("    \x1b[36mcd {dest_str} && epm services start\x1b[0m");
        println!("\n  \x1b[2mThen read\x1b[0m \x1b[1mCUSTOMIZE.md\x1b[0m \x1b[2mto make it yours.\x1b[0m");
    } else {
        println!("  \x1b[36mcd {dest_str} && cat CUSTOMIZE.md\x1b[0m");
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
