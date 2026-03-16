//! `epm mcp` — install, list, and remove MCP servers.
//!
//! MCP servers are EPS packages with an `[mcp]` section in their eps.toml.
//! Installing one builds the binary (via the package's install hook) and
//! registers it in `~/.claude.json` under `mcpServers`.

use std::path::PathBuf;

use anyhow::{bail, Context, Result};
use clap::Subcommand;
use serde_json::Value;

use crate::client::RegistryClient;
use crate::commands::{install, list::list_installed_versions, uninstall};

#[derive(Subcommand)]
pub enum McpCommands {
    /// Install an MCP server package and register it with Claude
    Install {
        /// Package name (must have an [mcp] section in its eps.toml)
        name: String,
    },
    /// List all MCP servers registered in ~/.claude.json
    List,
    /// Unregister an MCP server from ~/.claude.json and uninstall its package
    Remove {
        /// Server name as it appears in ~/.claude.json
        name: String,
    },
}

pub async fn run(cmd: &McpCommands, client: &RegistryClient) -> Result<()> {
    match cmd {
        McpCommands::Install { name } => run_install(client, name).await,
        McpCommands::List => run_list(),
        McpCommands::Remove { name } => run_remove(name),
    }
}

// ── install ───────────────────────────────────────────────────────────────────

async fn run_install(client: &RegistryClient, name: &str) -> Result<()> {
    // 1. Normal package install (runs build hook)
    install::run(client, name).await?;

    // 2. Find the installed version
    let pkg_root = packages_dir()?.join(name);
    let versions = list_installed_versions(&pkg_root)?;
    let version = versions
        .last()
        .ok_or_else(|| anyhow::anyhow!("install succeeded but no version found for '{name}'"))?
        .clone();

    let install_dir = pkg_root.join(&version);

    // 3. Read the [mcp] section from eps.toml
    let manifest = read_manifest(&install_dir)
        .with_context(|| format!("could not read eps.toml for '{name}@{version}'"))?;

    let mcp = manifest.mcp;
    let binary_name = mcp.binary.as_deref().unwrap_or(name);

    // 4. Find the binary (release build first, then debug)
    let binary_path = find_binary(&install_dir, binary_name)?;

    // 5. Patch ~/.claude.json
    register_mcp_server(name, &binary_path, &mcp.args, &mcp.env)?;

    println!("\n✓ {name} registered as MCP server");
    println!("  binary: {}", binary_path.display());
    println!("\nRestart Claude to load the new server.");
    Ok(())
}

fn find_binary(install_dir: &std::path::Path, binary_name: &str) -> Result<PathBuf> {
    let release = install_dir.join("target/release").join(binary_name);
    if release.exists() {
        return Ok(release);
    }
    let debug = install_dir.join("target/debug").join(binary_name);
    if debug.exists() {
        return Ok(debug);
    }
    bail!(
        "binary '{binary_name}' not found in target/release or target/debug under {}.\n\
         Make sure the package's install hook builds the binary (e.g. `cargo build --release`).",
        install_dir.display()
    )
}

fn register_mcp_server(
    name: &str,
    binary_path: &std::path::Path,
    args: &[String],
    env: &std::collections::HashMap<String, String>,
) -> Result<()> {
    let claude_json_path = claude_json_path()?;

    let mut root: Value = if claude_json_path.exists() {
        let raw = std::fs::read_to_string(&claude_json_path)
            .context("could not read ~/.claude.json")?;
        serde_json::from_str(&raw).unwrap_or(serde_json::json!({}))
    } else {
        serde_json::json!({})
    };

    let servers = root
        .as_object_mut()
        .context("~/.claude.json is not a JSON object")?
        .entry("mcpServers")
        .or_insert(serde_json::json!({}));

    let entry = serde_json::json!({
        "command": binary_path.to_string_lossy(),
        "args": args,
        "env": env,
    });

    servers
        .as_object_mut()
        .context("mcpServers is not a JSON object")?
        .insert(name.to_string(), entry);

    let out = serde_json::to_string_pretty(&root).context("failed to serialize ~/.claude.json")?;

    if let Some(parent) = claude_json_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&claude_json_path, out).context("could not write ~/.claude.json")?;

    Ok(())
}

// ── list ──────────────────────────────────────────────────────────────────────

fn run_list() -> Result<()> {
    let path = claude_json_path()?;
    if !path.exists() {
        println!("No MCP servers registered (~/.claude.json not found).");
        return Ok(());
    }

    let raw = std::fs::read_to_string(&path).context("could not read ~/.claude.json")?;
    let root: Value = serde_json::from_str(&raw).context("~/.claude.json is not valid JSON")?;

    let servers = match root.get("mcpServers").and_then(|s| s.as_object()) {
        Some(s) if !s.is_empty() => s,
        _ => {
            println!("No MCP servers registered.");
            return Ok(());
        }
    };

    println!("Registered MCP servers\n");
    for (name, entry) in servers {
        let cmd = entry.get("command").and_then(|c| c.as_str()).unwrap_or("(unknown)");
        println!("  {name}");
        println!("    command: {cmd}");
    }
    Ok(())
}

// ── remove ────────────────────────────────────────────────────────────────────

fn run_remove(name: &str) -> Result<()> {
    // 1. Remove from ~/.claude.json
    let path = claude_json_path()?;
    if !path.exists() {
        bail!("~/.claude.json not found — nothing to remove");
    }

    let raw = std::fs::read_to_string(&path).context("could not read ~/.claude.json")?;
    let mut root: Value = serde_json::from_str(&raw).context("~/.claude.json is not valid JSON")?;

    let removed = root
        .as_object_mut()
        .and_then(|r| r.get_mut("mcpServers"))
        .and_then(|s| s.as_object_mut())
        .map(|s| s.remove(name).is_some())
        .unwrap_or(false);

    if !removed {
        bail!("'{name}' is not registered as an MCP server");
    }

    let out = serde_json::to_string_pretty(&root)?;
    std::fs::write(&path, out)?;

    // 2. Uninstall the package from ~/.epm/packages/ (best-effort)
    match uninstall::run(name) {
        Ok(()) => {}
        Err(e) if e.to_string().contains("not installed") => {}
        Err(e) => eprintln!("warning: could not uninstall package: {e}"),
    }

    println!("Removed '{name}'.");
    println!("Restart Claude to apply the change.");
    Ok(())
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn claude_json_path() -> Result<PathBuf> {
    Ok(dirs::home_dir()
        .context("could not determine home directory")?
        .join(".claude.json"))
}

fn packages_dir() -> Result<PathBuf> {
    Ok(dirs::home_dir()
        .context("could not determine home directory")?
        .join(".epm")
        .join("packages"))
}

fn read_manifest(install_dir: &std::path::Path) -> Result<crate::models::EpsManifest> {
    let path = install_dir.join("eps.toml");
    let raw = std::fs::read_to_string(&path)?;
    Ok(toml::from_str(&raw)?)
}
