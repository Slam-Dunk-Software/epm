//! `epm self-uninstall` — remove epm and everything it installed.
//!
//! Reads `~/.epm/installed.toml` to know exactly what to clean up:
//! - Deletes tracked skill files from `~/.claude/commands/`
//! - Removes `~/.epm/`
//! - Removes `~/.epc/` (services state directory)
//! - Removes the epm-startup LaunchAgent / systemd unit (and old epc-startup if present)
//! - Removes the epm binary itself (unless `--keep-binary` is passed)

use anyhow::{Context, Result};

use crate::installed::InstalledManifest;

pub fn run(yes: bool, keep_binary: bool) -> Result<()> {
    let home = dirs::home_dir().context("could not determine home directory")?;

    if !yes {
        eprintln!("\x1b[33mThis will remove epm and everything it installed.\x1b[0m");
        eprintln!("Run with \x1b[1m--yes\x1b[0m to confirm.");
        std::process::exit(1);
    }

    println!("\x1b[2mUninstalling epm...\x1b[0m");

    let manifest = InstalledManifest::load(&home);

    let mut removed_skills: Vec<String> = vec![];

    // ── remove tracked skill files ────────────────────────────────────────────
    for pkg in &manifest.skills {
        let mut any = false;
        for file in &pkg.files {
            if std::fs::remove_file(file).is_ok() {
                any = true;
            }
        }
        if any || !pkg.files.is_empty() {
            removed_skills.push(pkg.name.clone());
        }
    }

    // ── remove ~/.epm/ ────────────────────────────────────────────────────────
    let epm_dir = home.join(".epm");
    if epm_dir.exists() {
        std::fs::remove_dir_all(&epm_dir)
            .with_context(|| format!("could not remove {}", epm_dir.display()))?;
    }

    // ── remove ~/.epc/ state directory ───────────────────────────────────────
    let epc_dir = home.join(".epc");
    let removed_epc_dir = if epc_dir.exists() {
        std::fs::remove_dir_all(&epc_dir).is_ok()
    } else {
        false
    };

    // ── remove startup service (both old epc and current epm variants) ────────
    #[cfg(target_os = "macos")]
    {
        let agents_dir = home.join("Library").join("LaunchAgents");
        for label in &["com.eps.epm-startup", "com.eps.epc-startup"] {
            let plist = agents_dir.join(format!("{label}.plist"));
            if plist.exists() {
                let _ = std::process::Command::new("launchctl")
                    .args(["unload", &plist.to_string_lossy()])
                    .status();
                let _ = std::fs::remove_file(&plist);
            }
        }
    }
    #[cfg(target_os = "linux")]
    {
        let systemd_dir = home.join(".config").join("systemd").join("user");
        for unit_name in &["epm-startup", "epc-startup"] {
            let unit = systemd_dir.join(format!("{unit_name}.service"));
            if unit.exists() {
                let _ = std::process::Command::new("systemctl")
                    .args(["--user", "disable", "--now", unit_name])
                    .status();
                let _ = std::fs::remove_file(&unit);
            }
        }
    }

    // ── print summary ─────────────────────────────────────────────────────────
    println!();
    if !removed_skills.is_empty() {
        for name in &removed_skills {
            println!("\x1b[31m✕\x1b[0m Skills package \x1b[1m{name}\x1b[0m removed");
        }
    }
    println!("\x1b[31m✕\x1b[0m \x1b[2m~/.epm/\x1b[0m deleted");
    if removed_epc_dir {
        println!("\x1b[31m✕\x1b[0m \x1b[2m~/.epc/\x1b[0m deleted");
    }

    // ── remove the binary (last — we're still running from it) ────────────────
    if !keep_binary {
        if let Ok(exe) = std::env::current_exe() {
            let _ = std::fs::remove_file(&exe);
            println!("\x1b[31m✕\x1b[0m \x1b[2m{}\x1b[0m deleted", exe.display());
        }
    }

    println!();
    println!("\x1b[1mAll done. epm has left the building.\x1b[0m");
    if !removed_skills.is_empty() {
        println!("\x1b[2mIf you have any Claude Code instances running, restart them to apply the changes.\x1b[0m");
    }

    Ok(())
}
