use std::fs::File;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};

use crate::{
    models::EpsManifest,
    services::state::{RegistryFile, ServiceEntry, ServicesFile},
    services::tailscale,
};

pub fn resolve_package_dir_inner(
    spec: Option<&str>,
    local: Option<&Path>,
    packages_base: &Path,
) -> Result<PathBuf> {
    if let Some(path) = local {
        let abs = path.canonicalize().with_context(|| {
            format!("could not resolve local path '{}'", path.display())
        })?;
        return Ok(abs);
    }

    if spec.is_none() {
        let cwd = std::env::current_dir().context("could not determine current directory")?;
        if cwd.join("eps.toml").exists() {
            return Ok(cwd);
        }
        bail!(
            "no eps.toml found in the current directory\n\
             \n\
             Usage:\n\
             \tepm services start                    # inside a project directory\n\
             \tepm services start --local <path>     # explicit local path\n\
             \tepm services start <package-name>     # installed via epm"
        );
    }

    let spec = spec.unwrap();

    if spec.starts_with('.') || spec.starts_with('/') || spec.starts_with('~') {
        let path = Path::new(spec);
        let abs = path
            .canonicalize()
            .with_context(|| format!("could not resolve local path '{spec}'"))?;
        return Ok(abs);
    }

    let base = packages_base.join(spec);
    if !base.exists() {
        bail!(
            "package '{spec}' is not installed\n\
             \n\
             Did you mean to deploy a local project? Try:\n\
             \tepm services start --local <path>   # e.g. epm services start --local ./my-project\n\
             \tepm services start                  # from inside the project directory\n\
             \n\
             Or install it first with:  epm install {spec}"
        );
    }

    let mut versions: Vec<PathBuf> = std::fs::read_dir(&base)?
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_dir())
        .collect();

    if versions.is_empty() {
        bail!("no installed versions found for '{spec}' in {}", base.display());
    }

    versions.sort();
    Ok(versions.pop().unwrap())
}

pub fn resolve_package_dir(spec: Option<&str>, local: Option<&Path>) -> Result<PathBuf> {
    let packages_base = dirs::home_dir()
        .context("could not determine home directory")?
        .join(".epm")
        .join("packages");
    resolve_package_dir_inner(spec, local, &packages_base)
}

pub async fn run(spec: Option<&str>, local: Option<&Path>) -> Result<()> {
    let is_local = local.is_some() || spec.is_none();
    let pkg_dir = resolve_package_dir(spec, local)?;
    let manifest = EpsManifest::from_file(&pkg_dir.join("eps.toml"))?;
    let svc = manifest.require_service()?;
    let name = &manifest.package.name;

    if is_local {
        eprintln!("\x1b[2mdeploying {} locally from {}\x1b[0m", name, pkg_dir.display());
        eprintln!("\x1b[2m(this runs on your machine — it is not a cloud deployment)\x1b[0m");
    }

    let mut services = ServicesFile::load()?;
    if let Some(existing) = services.services.get(name) {
        if ServicesFile::is_port_listening(existing.port) {
            bail!("'{name}' is already running on port {}", existing.port);
        }
        services.remove(name);
    }

    let port = if ServicesFile::is_port_listening(svc.port) {
        let assigned = ServicesFile::find_available_port(svc.port + 1)
            .with_context(|| format!("port {} is in use and no free port found nearby", svc.port))?;
        eprintln!(
            "\x1b[33m⚠\x1b[0m  port {} in use — assigning port {} instead",
            svc.port, assigned
        );
        let toml_path = pkg_dir.join("eps.toml");
        if let Ok(contents) = std::fs::read_to_string(&toml_path) {
            let updated = regex_replace_port(&contents, svc.port, assigned);
            let _ = std::fs::write(&toml_path, updated);
        }
        assigned
    } else {
        svc.port
    };

    let log_dir = crate::services::state::services_state_dir()?.join("logs");
    std::fs::create_dir_all(&log_dir)?;
    let log_path = log_dir.join(format!("{name}.log"));
    let log_file = File::create(&log_path)
        .with_context(|| format!("failed to create log file {}", log_path.display()))?;
    let log_stderr = log_file.try_clone()?;

    let mut child = tokio::process::Command::new("bash")
        .arg("-c")
        .arg(&svc.start)
        .current_dir(&pkg_dir)
        .env("PORT", port.to_string())
        .process_group(0)
        .stdout(log_file)
        .stderr(log_stderr)
        .spawn()
        .with_context(|| format!("failed to spawn '{}'", svc.start))?;

    let pid = child.id().context("failed to get PID of spawned process")?;

    tokio::time::sleep(std::time::Duration::from_millis(1500)).await;

    if let Ok(Some(_)) = child.try_wait() {
        eprintln!("\n\x1b[31m✗\x1b[0m \x1b[1m{name}\x1b[0m exited immediately — it likely crashed on startup.\n");
        if let Ok(contents) = std::fs::read_to_string(&log_path) {
            let lines: Vec<&str> = contents.lines().collect();
            let head = &lines[..lines.len().min(10)];
            for line in head {
                eprintln!("  \x1b[2m{line}\x1b[0m");
            }
            if lines.len() > 10 {
                eprintln!("  \x1b[2m... ({} more lines)\x1b[0m", lines.len() - 10);
            }
        }
        eprintln!("\n  \x1b[2mFull logs:\x1b[0m \x1b[36m{}\x1b[0m", log_path.display());
        eprintln!("  \x1b[2mFix the error above, then run\x1b[0m \x1b[36mepm services start\x1b[0m \x1b[2magain.\x1b[0m");
        std::process::exit(1);
    }

    let host = tailscale::ip().await?;

    let started = chrono::Utc::now().to_rfc3339();
    let entry = ServiceEntry {
        dir: pkg_dir.to_string_lossy().to_string(),
        port,
        pid,
        started,
        log_file: log_path.to_string_lossy().to_string(),
    };
    services.insert(name.clone(), entry);
    services.save()?;

    let mut registry = RegistryFile::load()?;
    registry.insert(name.clone(), pkg_dir.to_string_lossy().to_string());
    registry.save()?;

    println!("\n\x1b[32m✓\x1b[0m \x1b[1m{name}\x1b[0m deployed \x1b[36m→ http://{host}:{port}\x1b[0m");
    println!("  \x1b[2mpid   {pid}\x1b[0m");
    println!("  \x1b[2mlogs  {}\x1b[0m", log_path.display());

    Ok(())
}

fn regex_replace_port(contents: &str, old: u16, new: u16) -> String {
    contents
        .lines()
        .map(|line| {
            let trimmed = line.trim();
            if trimmed.starts_with("port") {
                if let Some(rest) = trimmed.strip_prefix("port") {
                    let rest = rest.trim_start_matches(|c: char| c.is_whitespace() || c == '=').trim();
                    let val = rest.split('#').next().unwrap_or("").trim();
                    if val == old.to_string().as_str() {
                        let indent = &line[..line.len() - line.trim_start().len()];
                        return format!("{indent}port = {new}");
                    }
                }
            }
            line.to_string()
        })
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn resolve_local_path_success() {
        let dir = TempDir::new().unwrap();
        let result = resolve_package_dir_inner(Some("my_pkg"), Some(dir.path()), Path::new("/unused")).unwrap();
        assert_eq!(result, dir.path().canonicalize().unwrap());
    }

    #[test]
    fn resolve_local_path_missing_errors() {
        let err = resolve_package_dir_inner(Some("x"), Some(Path::new("/no/such/dir")), Path::new("/unused")).unwrap_err();
        assert!(err.to_string().contains("could not resolve"));
    }

    #[test]
    fn resolve_installed_missing_errors() {
        let dir = TempDir::new().unwrap();
        let packages_base = dir.path().join("packages");
        std::fs::create_dir_all(&packages_base).unwrap();
        let err = resolve_package_dir_inner(Some("no_pkg"), None, &packages_base).unwrap_err();
        assert!(err.to_string().contains("not installed"));
    }

    #[test]
    fn resolve_installed_picks_latest_version() {
        let dir = TempDir::new().unwrap();
        let packages_base = dir.path().join("packages");
        let pkg_base = packages_base.join("my_pkg");
        std::fs::create_dir_all(pkg_base.join("0.1.0")).unwrap();
        std::fs::create_dir_all(pkg_base.join("0.2.0")).unwrap();
        std::fs::create_dir_all(pkg_base.join("1.0.0")).unwrap();

        let result = resolve_package_dir_inner(Some("my_pkg"), None, &packages_base).unwrap();
        assert!(result.ends_with("1.0.0"));
    }
}
