use anyhow::Result;

use crate::{models::EpsManifest, services::state::ServicesFile, services::tailscale};

#[derive(Debug, PartialEq)]
enum Level {
    Pass,
    Warn,
    Fail,
}

struct Finding {
    level: Level,
    check: &'static str,
    detail: String,
}

impl Finding {
    fn pass(check: &'static str, detail: impl Into<String>) -> Self {
        Self { level: Level::Pass, check, detail: detail.into() }
    }
    fn warn(check: &'static str, detail: impl Into<String>) -> Self {
        Self { level: Level::Warn, check, detail: detail.into() }
    }
    fn fail(check: &'static str, detail: impl Into<String>) -> Self {
        Self { level: Level::Fail, check, detail: detail.into() }
    }
}

fn bound_address(port: u16) -> Option<String> {
    let out = std::process::Command::new("lsof")
        .args(["-i", &format!(":{port}"), "-n", "-P", "-sTCP:LISTEN"])
        .output()
        .ok()?;

    let stdout = String::from_utf8_lossy(&out.stdout);
    for line in stdout.lines().skip(1) {
        if let Some(name) = line
            .split_whitespace()
            .rev()
            .find(|t| t.contains(':') && !t.starts_with('('))
        {
            if let Some(addr) = name.rsplit_once(':').map(|(host, _)| host) {
                return Some(addr.to_string());
            }
        }
    }
    None
}

fn check_version_sync(dir: &str, eps_version: &str) -> Option<Finding> {
    #[derive(serde::Deserialize)]
    struct CargoPackage { version: String }
    #[derive(serde::Deserialize)]
    struct CargoToml { package: CargoPackage }

    let path = std::path::Path::new(dir).join("Cargo.toml");
    let raw = std::fs::read_to_string(&path).ok()?;
    let cargo: CargoToml = toml::from_str(&raw).ok()?;

    if cargo.package.version == eps_version {
        Some(Finding::pass("version sync", format!("eps.toml and Cargo.toml both at v{eps_version}")))
    } else {
        Some(Finding::warn(
            "version sync",
            format!("eps.toml is v{eps_version} but Cargo.toml is v{}", cargo.package.version),
        ))
    }
}

fn check_start_command(start: &str, tailscale_ip: &str) -> Vec<Finding> {
    if start.contains("0.0.0.0") {
        return vec![Finding::fail(
            "start command",
            "binds to 0.0.0.0 — exposes service on all interfaces, not just Tailscale. \
             Use HOST=$(tailscale ip -4) instead.",
        )];
    }
    if start.contains("tailscale ip") || start.contains(tailscale_ip) {
        return vec![Finding::pass("start command", "binds to Tailscale IP only")];
    }
    vec![Finding::warn(
        "start command",
        "no HOST binding set — relies on binary default (typically 127.0.0.1). \
         Add HOST=$(tailscale ip -4) to make it accessible on your Tailnet.",
    )]
}

fn check_live_binding(port: u16, tailscale_ip: &str) -> Finding {
    match bound_address(port) {
        None => Finding::warn(
            "live binding",
            format!("could not determine bound address for port {port}"),
        ),
        Some(addr) if addr == "*" || addr == "0.0.0.0" => Finding::fail(
            "live binding",
            format!("port {port} is bound to {addr} — visible on all network interfaces"),
        ),
        Some(addr) if addr == "127.0.0.1" || addr == "localhost" => Finding::warn(
            "live binding",
            format!("port {port} is bound to {addr} — localhost only, not reachable on Tailnet"),
        ),
        Some(addr) if addr == tailscale_ip => Finding::pass(
            "live binding",
            format!("port {port} bound to Tailscale IP ({addr}) only"),
        ),
        Some(addr) => Finding::warn(
            "live binding",
            format!("port {port} is bound to {addr} — expected Tailscale IP {tailscale_ip}"),
        ),
    }
}

pub async fn run() -> Result<()> {
    let services = ServicesFile::load()?;

    if services.services.is_empty() {
        println!("\x1b[2mNo services registered.\x1b[0m");
        return Ok(());
    }

    let tailscale_ip = tailscale::ip().await?;
    let mut any_fail = false;
    let mut any_warn = false;

    let mut names: Vec<&String> = services.services.keys().collect();
    names.sort();

    for name in names {
        let entry = &services.services[name];
        let running = ServicesFile::is_port_listening(entry.port);

        println!("\x1b[1m{name}\x1b[0m \x1b[2m(port {})\x1b[0m", entry.port);

        if running {
            print_finding(&Finding::pass("port", format!("something is listening on :{}", entry.port)));
        } else {
            print_finding(&Finding::warn("port", format!("nothing listening on :{} — service may be down", entry.port)));
        }

        if running {
            let f = check_live_binding(entry.port, &tailscale_ip);
            track_level(&f.level, &mut any_fail, &mut any_warn);
            print_finding(&f);
        }

        let eps_path = std::path::Path::new(&entry.dir).join("eps.toml");
        match EpsManifest::from_file(&eps_path) {
            Err(e) => {
                let f = Finding::warn("eps.toml", format!("could not read manifest: {e}"));
                track_level(&f.level, &mut any_fail, &mut any_warn);
                print_finding(&f);
            }
            Ok(manifest) => {
                if let Some(ref svc) = manifest.service {
                    if let Some(ref start) = svc.start {
                        for f in check_start_command(start, &tailscale_ip) {
                            track_level(&f.level, &mut any_fail, &mut any_warn);
                            print_finding(&f);
                        }
                    }
                } else {
                    print_finding(&Finding::warn("eps.toml", "no [service] block found"));
                }

                if let Some(f) = check_version_sync(&entry.dir, &manifest.package.version) {
                    track_level(&f.level, &mut any_fail, &mut any_warn);
                    print_finding(&f);
                }
            }
        }

        println!();
    }

    if any_fail {
        println!("\x1b[31m✗ FAIL\x1b[0m — one or more services have binding issues.");
    } else if any_warn {
        println!("\x1b[33m! WARN\x1b[0m — review the warnings above.");
    } else {
        println!("\x1b[32m✓\x1b[0m all services passed.");
    }

    Ok(())
}

fn print_finding(f: &Finding) {
    let (color, symbol) = match f.level {
        Level::Pass => ("\x1b[32m", "✓"),
        Level::Warn => ("\x1b[33m", "!"),
        Level::Fail => ("\x1b[31m", "✗"),
    };
    println!("  {color}{symbol}\x1b[0m \x1b[2m{}\x1b[0m  {}", f.check, f.detail);
}

fn track_level(level: &Level, any_fail: &mut bool, any_warn: &mut bool) {
    match level {
        Level::Fail => *any_fail = true,
        Level::Warn => *any_warn = true,
        Level::Pass => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TAILSCALE_IP: &str = "100.78.103.79";

    #[test]
    fn start_with_zero_zero_is_fail() {
        let findings = check_start_command("HOST=0.0.0.0 ./serve.sh", TAILSCALE_IP);
        assert_eq!(findings[0].level, Level::Fail);
    }

    #[test]
    fn start_with_tailscale_ip_subcommand_is_pass() {
        let findings = check_start_command("HOST=$(tailscale ip -4) ./serve.sh", TAILSCALE_IP);
        assert_eq!(findings[0].level, Level::Pass);
    }

    #[test]
    fn start_with_no_host_is_warn() {
        let findings = check_start_command("./serve.sh", TAILSCALE_IP);
        assert_eq!(findings[0].level, Level::Warn);
    }

    #[test]
    fn version_sync_pass_when_matching() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(dir.path().join("Cargo.toml"), "[package]\nname = \"foo\"\nversion = \"1.2.3\"\n").unwrap();
        let f = check_version_sync(dir.path().to_str().unwrap(), "1.2.3").unwrap();
        assert_eq!(f.level, Level::Pass);
    }

    #[test]
    fn version_sync_warn_when_mismatched() {
        let dir = tempfile::TempDir::new().unwrap();
        std::fs::write(dir.path().join("Cargo.toml"), "[package]\nname = \"foo\"\nversion = \"1.2.3\"\n").unwrap();
        let f = check_version_sync(dir.path().to_str().unwrap(), "1.2.4").unwrap();
        assert_eq!(f.level, Level::Warn);
    }
}
