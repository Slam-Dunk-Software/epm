use std::path::Path;

use anyhow::{bail, Result};

use crate::services::state::ServicesFile;

pub fn run(name: &str) -> Result<()> {
    run_with_state(name, &ServicesFile::default_path()?)
}

pub fn run_with_state(name: &str, state_path: &Path) -> Result<()> {
    let mut services = ServicesFile::load_from(state_path)?;

    let entry = match services.services.get(name) {
        Some(e) => e.clone(),
        None => bail!("no service named '{name}' is registered"),
    };

    std::process::Command::new("kill")
        .args(["--", &format!("-{}", entry.pid)])
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .ok();

    for pid in ServicesFile::pids_on_port(entry.port) {
        std::process::Command::new("kill")
            .arg(pid.to_string())
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .status()
            .ok();
    }

    services.remove(name);
    services.save()?;

    println!("\x1b[31m✕\x1b[0m \x1b[1m{name}\x1b[0m \x1b[2mstopped\x1b[0m");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::services::state::{ServiceEntry, ServicesFile};
    use tempfile::TempDir;

    fn sample_entry(pid: u32) -> ServiceEntry {
        ServiceEntry {
            dir: "/tmp/pkg".to_string(),
            port: 9000,
            pid,
            started: "2026-02-28T00:00:00Z".to_string(),
            log_file: "/tmp/.epc/logs/pkg.log".to_string(),
        }
    }

    #[test]
    fn stop_nonexistent_service_errors() {
        let dir = TempDir::new().unwrap();
        let state_path = dir.path().join("services.toml");
        let err = run_with_state("no_such_service", &state_path).unwrap_err();
        assert!(err.to_string().contains("no service named"));
    }

    #[test]
    fn stop_removes_entry_from_state_file() {
        let dir = TempDir::new().unwrap();
        let state_path = dir.path().join("services.toml");

        let child = std::process::Command::new("sleep")
            .arg("60")
            .spawn()
            .expect("failed to spawn sleep");
        let pid = child.id();

        let mut sf = ServicesFile::load_from(&state_path).unwrap();
        sf.insert("test_svc".to_string(), sample_entry(pid));
        sf.save().unwrap();

        run_with_state("test_svc", &state_path).unwrap();

        let loaded = ServicesFile::load_from(&state_path).unwrap();
        assert!(!loaded.services.contains_key("test_svc"));
    }
}
