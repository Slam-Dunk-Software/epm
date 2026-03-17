//! `epm self-uninstall` — remove epm and everything it installed.
//!
//! Reads `~/.epm/installed.toml` to know exactly what to clean up:
//! - Removes tracked MCP servers from `~/.claude.json`
//! - Deletes tracked skill files from `~/.claude/commands/`
//! - Removes `~/.epm/`
//! - Removes the epm binary itself (unless `--keep-binary` is passed)

use anyhow::{Context, Result};
use serde_json::Value;

use crate::installed::InstalledManifest;

pub fn run(yes: bool, keep_binary: bool) -> Result<()> {
    let home = dirs::home_dir().context("could not determine home directory")?;

    if !yes {
        eprintln!("This will remove epm and everything it installed.");
        eprintln!("Run with --yes to confirm.");
        std::process::exit(1);
    }

    let manifest = InstalledManifest::load(&home);

    let mut removed_mcps: Vec<String> = vec![];
    let mut removed_skills: Vec<String> = vec![];

    // ── remove tracked MCPs from ~/.claude.json ───────────────────────────────
    if !manifest.mcp.is_empty() {
        let claude_json = home.join(".claude.json");
        if claude_json.exists() {
            if let Ok(raw) = std::fs::read_to_string(&claude_json) {
                if let Ok(mut root) = serde_json::from_str::<Value>(&raw) {
                    if let Some(servers) = root
                        .as_object_mut()
                        .and_then(|r| r.get_mut("mcpServers"))
                        .and_then(|s| s.as_object_mut())
                    {
                        for entry in &manifest.mcp {
                            if servers.remove(&entry.name).is_some() {
                                removed_mcps.push(entry.name.clone());
                            }
                        }
                    }
                    if let Ok(out) = serde_json::to_string_pretty(&root) {
                        let _ = std::fs::write(&claude_json, out);
                    }
                }
            }
        }
    }

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

    // ── print summary ─────────────────────────────────────────────────────────
    if removed_mcps.is_empty() && removed_skills.is_empty() {
        println!("Removed ~/.epm/.");
    } else {
        if !removed_mcps.is_empty() {
            println!("Removed MCP server{}:", if removed_mcps.len() == 1 { "" } else { "s" });
            for name in &removed_mcps {
                println!("  {name}");
            }
        }
        if !removed_skills.is_empty() {
            println!("Removed skill{}:", if removed_skills.len() == 1 { "" } else { "s" });
            for name in &removed_skills {
                println!("  {name}");
            }
        }
        println!("Removed ~/.epm/.");
    }

    // ── remove the binary (last — we're still running from it) ────────────────
    if !keep_binary {
        if let Ok(exe) = std::env::current_exe() {
            let _ = std::fs::remove_file(&exe);
            println!("Removed {}.", exe.display());
        }
        println!("\nepm has been uninstalled.");
        if !removed_mcps.is_empty() || !removed_skills.is_empty() {
            println!("If you have any Claude Code instances running, you'll need to restart them.");
        }
    }

    Ok(())
}
