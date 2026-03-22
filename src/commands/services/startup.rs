use std::path::PathBuf;

use anyhow::Result;

use crate::{models::EpsManifest, services::state::{RegistryFile, ServicesFile}};

pub async fn run() -> Result<()> {
    let registry = RegistryFile::load()?;

    if registry.services.is_empty() {
        println!("No services in ~/.epc/registry.toml. Nothing to start.");
        println!("Run `epm services start` inside a project directory to register a service.");
        return Ok(());
    }

    wait_for_tailscale(30).await;

    let total = registry.services.len();
    println!(
        "Starting {total} registered service{}...\n",
        if total == 1 { "" } else { "s" }
    );

    let mut started = 0usize;
    let mut skipped = 0usize;
    let mut failed = 0usize;

    let mut names: Vec<_> = registry.services.keys().cloned().collect();
    names.sort();

    for name in &names {
        let entry = &registry.services[name];
        let dir = PathBuf::from(&entry.dir);

        if !dir.exists() {
            eprintln!(
                "  \x1b[33m⚠ {name}: directory not found ({}) — skipping\x1b[0m",
                dir.display()
            );
            failed += 1;
            continue;
        }

        let port = EpsManifest::from_file(&dir.join("eps.toml"))
            .ok()
            .and_then(|m| m.service)
            .and_then(|s| s.port);

        if let Some(port) = port {
            if ServicesFile::is_port_listening(port) {
                println!("  \x1b[2m↓ {name} already running on :{port}\x1b[0m");
                skipped += 1;
                continue;
            }
        }

        if !startup_enabled(&dir) {
            println!("  \x1b[2m– {name} startup = false, skipping\x1b[0m");
            skipped += 1;
            continue;
        }

        match crate::commands::services::start::run(None, Some(&dir)).await {
            Ok(()) => started += 1,
            Err(e) => {
                eprintln!("  \x1b[31m✗ {name}: {e}\x1b[0m");
                failed += 1;
            }
        }
    }

    println!(
        "\nStartup complete: {started} started, {skipped} already running, {failed} failed"
    );
    Ok(())
}

fn startup_enabled(dir: &std::path::Path) -> bool {
    let Ok(manifest) = EpsManifest::from_file(&dir.join("eps.toml")) else {
        return true;
    };
    match &manifest.service {
        Some(svc) => svc.startup,
        None => true,
    }
}

async fn wait_for_tailscale(max_seconds: u64) {
    for _ in 0..max_seconds {
        let ready = tokio::process::Command::new("tailscale")
            .args(["status", "--json"])
            .output()
            .await
            .map(|o| o.status.success())
            .unwrap_or(false);
        if ready {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_secs(1)).await;
    }
    eprintln!(
        "\x1b[33mWarning: Tailscale not ready after {max_seconds}s — \
         services that bind to the Tailscale IP may start on localhost instead\x1b[0m"
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;
    use tempfile::TempDir;

    fn make_pkg(dir: &TempDir, toml: &str) -> std::path::PathBuf {
        let path = dir.path().join("eps.toml");
        let mut f = std::fs::File::create(&path).unwrap();
        f.write_all(toml.as_bytes()).unwrap();
        dir.path().to_path_buf()
    }

    #[test]
    fn startup_enabled_when_no_eps_toml() {
        let dir = TempDir::new().unwrap();
        assert!(startup_enabled(dir.path()));
    }

    #[test]
    fn startup_enabled_when_no_service_block() {
        let dir = TempDir::new().unwrap();
        make_pkg(&dir, r#"
            [package]
            name = "lib_pkg"
            version = "0.1.0"
            description = "x"
            authors = []
            license = "MIT"
            platforms = []
            repository = ""
        "#);
        assert!(startup_enabled(dir.path()));
    }

    #[test]
    fn startup_disabled_when_set_to_false() {
        let dir = TempDir::new().unwrap();
        make_pkg(&dir, r#"
            [package]
            name = "svc"
            version = "0.1.0"
            description = "x"
            authors = []
            license = "MIT"
            platforms = []
            repository = ""

            [service]
            enabled = true
            startup = false
            start = "./run.sh"
            port = 9000
        "#);
        assert!(!startup_enabled(dir.path()));
    }

    #[test]
    fn startup_enabled_by_default() {
        let dir = TempDir::new().unwrap();
        make_pkg(&dir, r#"
            [package]
            name = "svc"
            version = "0.1.0"
            description = "x"
            authors = []
            license = "MIT"
            platforms = []
            repository = ""

            [service]
            enabled = true
            start = "./run.sh"
            port = 9000
        "#);
        assert!(startup_enabled(dir.path()));
    }
}
