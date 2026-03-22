use std::path::PathBuf;

use anyhow::Result;

use crate::{
    models::EpsManifest,
    services::state::{RegistryFile, ServiceEntry, ServicesFile},
};

/// Repair services.toml from the persistent registry.
///
/// Scans every project dir recorded in ~/.epc/registry.toml, checks which
/// services are actually listening on their declared port, and writes a fresh
/// entry into services.toml for each one that is. Useful after services.toml
/// is wiped or after processes started outside epm.
pub fn run() -> Result<()> {
    let registry = RegistryFile::load()?;

    if registry.services.is_empty() {
        println!("~/.epc/registry.toml is empty — nothing to sync.");
        println!("Run `epm services serve` inside a project directory to register a service.");
        return Ok(());
    }

    let mut services = ServicesFile::load()?;
    let log_base = dirs::home_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join(".epc")
        .join("logs");

    let mut synced = 0usize;
    let mut already_ok = 0usize;
    let mut stopped = 0usize;
    let mut missing = 0usize;

    let mut names: Vec<_> = registry.services.keys().cloned().collect();
    names.sort();

    for name in &names {
        let entry = &registry.services[name];
        let dir = PathBuf::from(&entry.dir);

        if !dir.exists() {
            eprintln!("  \x1b[33m⚠ {name}: directory not found ({}) — skipping\x1b[0m", dir.display());
            missing += 1;
            continue;
        }

        let Ok(manifest) = EpsManifest::from_file(&dir.join("eps.toml")) else {
            eprintln!("  \x1b[33m⚠ {name}: could not read eps.toml — skipping\x1b[0m");
            missing += 1;
            continue;
        };

        let Some(svc) = manifest.service else {
            continue; // not a service EPS
        };

        let Some(port) = svc.port else {
            continue; // no port declared
        };

        if !ServicesFile::is_port_listening(port) {
            println!("  \x1b[2m– {name}: not running on :{port}\x1b[0m");
            stopped += 1;
            continue;
        }

        if let Some(existing) = services.services.get(name.as_str()) {
            if existing.port == port {
                println!("  \x1b[2m✓ {name}: already tracked on :{port}\x1b[0m");
                already_ok += 1;
                continue;
            }
        }

        let pid = ServicesFile::pids_on_port(port).into_iter().next().unwrap_or(0);
        let log_file = log_base.join(format!("{name}.log"));

        services.insert(name.clone(), ServiceEntry {
            dir: entry.dir.clone(),
            port,
            pid,
            started: chrono::Utc::now().to_rfc3339(),
            log_file: log_file.to_string_lossy().to_string(),
        });

        println!("  \x1b[32m↑ {name}\x1b[0m re-registered on :{port} (pid {pid})");
        synced += 1;
    }

    if synced > 0 {
        services.save()?;
    }

    println!(
        "\nSync complete: {synced} re-registered, {already_ok} already tracked, \
         {stopped} not running, {missing} missing"
    );
    Ok(())
}
