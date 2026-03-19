//! `epm runtime` — install and manage the EPS runtime tools (epc + observatory + tree_walker).
//!
//! These are treated specially: they bypass the registry entirely and are
//! sourced directly from their canonical GitHub repos.

use std::io::Write;
use std::process::{Command, Stdio};

use anyhow::{Context, Result};
use clap::Subcommand;
use semver::Version;

const EPC_REPO:          &str = "Slam-Dunk-Software/epc";
const OBSERVATORY_REPO:  &str = "Slam-Dunk-Software/observatory";
const TREE_WALKER_REPO:  &str = "Slam-Dunk-Software/tree_walker";
const OBSERVATORY_DEST:  &str = "observatory";  // relative to home dir

#[derive(Subcommand)]
pub enum RuntimeCommands {
    /// Install epc and scaffold observatory
    Install {
        /// Only install a specific component: epc or observatory
        #[arg(value_name = "COMPONENT")]
        component: Option<String>,
    },
    /// Upgrade epc to the latest release
    Upgrade {
        /// Component to upgrade (only epc is upgradable — observatory is your harness)
        #[arg(value_name = "COMPONENT")]
        component: Option<String>,
    },
    /// Show installed versions vs latest available
    Status,
}

pub async fn run(cmd: &RuntimeCommands) -> Result<()> {
    match cmd {
        RuntimeCommands::Install { component } => run_install(component.as_deref()).await,
        RuntimeCommands::Upgrade { component } => run_upgrade(component.as_deref()).await,
        RuntimeCommands::Status => run_status().await,
    }
}

// ── install ───────────────────────────────────────────────────────────────────

async fn run_install(only: Option<&str>) -> Result<()> {
    match only {
        Some("epc")          => install_epc().await,
        Some("observatory")  => install_observatory(),
        Some("tree_walker")  => install_tree_walker().await,
        Some(other)          => anyhow::bail!(
            "unknown component '{other}' — valid options: epc, observatory, tree_walker"
        ),
        None => {
            install_epc().await?;
            println!();
            install_observatory()?;
            println!();
            install_tree_walker().await
        }
    }
}

async fn install_epc() -> Result<()> {
    let install_dir = epc_install_dir()?;
    let dest = install_dir.join("epc");

    if dest.exists() {
        // Check current version
        let current = epc_installed_version();
        println!("\x1b[32m✓\x1b[0m \x1b[1mepc\x1b[0m \x1b[2malready installed{}\x1b[0m",
            current.as_deref().map(|v| format!(" (v{v})")).unwrap_or_default()
        );
        return Ok(());
    }

    println!("\x1b[2mInstalling\x1b[0m \x1b[1mepc\x1b[0m\x1b[2m...\x1b[0m");

    let (url, version) = latest_release_download_url(EPC_REPO, epc_asset_name()).await?;
    download_binary(&url, &dest).await?;

    println!("\x1b[32m✓\x1b[0m \x1b[1mepc v{version}\x1b[0m \x1b[2minstalled to {}\x1b[0m",
        dest.display());

    Ok(())
}

fn install_observatory() -> Result<()> {
    let home = dirs::home_dir().context("could not determine home directory")?;
    let dest = home.join(OBSERVATORY_DEST);

    if dest.exists() {
        println!("\x1b[32m✓\x1b[0m \x1b[1mobservatory\x1b[0m \x1b[2malready exists at {}\x1b[0m",
            dest.display());
        return Ok(());
    }

    println!("\x1b[2mScaffolding\x1b[0m \x1b[1mobservatory\x1b[0m\x1b[2m...\x1b[0m");

    let clone_url = format!("https://github.com/{OBSERVATORY_REPO}.git");
    let dest_str = dest.to_string_lossy();

    let ok = Command::new("git")
        .args(["clone", "--quiet", "--depth", "1", &clone_url, &dest_str])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .context("failed to run git clone")?
        .success();

    if !ok {
        anyhow::bail!("git clone failed — check your internet connection");
    }

    // Strip upstream history — it's yours now
    std::fs::remove_dir_all(dest.join(".git"))
        .context("failed to remove upstream .git")?;

    Command::new("git").args(["init", &dest_str])
        .stdout(Stdio::null()).stderr(Stdio::null()).status()?;
    Command::new("git").args(["-C", &dest_str, "add", "."])
        .stdout(Stdio::null()).stderr(Stdio::null()).status()?;
    Command::new("git")
        .args(["-C", &dest_str, "commit", "-m", "Initial commit (epm runtime install)"])
        .stdout(Stdio::null()).stderr(Stdio::null()).status()?;

    println!("\x1b[32m✓\x1b[0m \x1b[1mobservatory\x1b[0m \x1b[2mready at {}\x1b[0m",
        dest.display());
    println!();
    println!("  \x1b[2mDeploy it:\x1b[0m");
    println!("    \x1b[36mcd ~/observatory && epc deploy .\x1b[0m");
    println!("  \x1b[2mThen read\x1b[0m \x1b[1mCUSTOMIZE.md\x1b[0m \x1b[2mto configure alerts.\x1b[0m");

    Ok(())
}

async fn install_tree_walker() -> Result<()> {
    let install_dir = epc_install_dir()?; // same ~/.local/bin
    let dest = install_dir.join("tree_walker");

    if dest.exists() {
        let current = tree_walker_installed_version();
        println!("\x1b[32m✓\x1b[0m \x1b[1mtree_walker\x1b[0m \x1b[2malready installed{}\x1b[0m",
            current.as_deref().map(|v| format!(" (v{v})")).unwrap_or_default()
        );
        return Ok(());
    }

    println!("\x1b[2mInstalling\x1b[0m \x1b[1mtree_walker\x1b[0m\x1b[2m...\x1b[0m");

    let (url, version) = latest_release_download_url(TREE_WALKER_REPO, tree_walker_asset_name()).await?;
    download_binary(&url, &dest).await?;

    println!("\x1b[32m✓\x1b[0m \x1b[1mtree_walker v{version}\x1b[0m \x1b[2minstalled to {}\x1b[0m",
        dest.display());

    Ok(())
}

async fn upgrade_tree_walker() -> Result<()> {
    eprintln!("\x1b[2mChecking for tree_walker updates...\x1b[0m");

    let client = http_client()?;
    let latest = latest_version(&client, TREE_WALKER_REPO).await?;
    let current = tree_walker_installed_version();
    let current_str = current.as_deref().unwrap_or("unknown");

    if let Some(ref c) = current {
        let cv = Version::parse(c).ok();
        let lv = Version::parse(&latest).ok();
        if cv.is_some() && cv >= lv {
            println!("\x1b[32m✓\x1b[0m \x1b[1mtree_walker\x1b[0m \x1b[2malready up to date (v{c})\x1b[0m");
            return Ok(());
        }
    }

    println!("\x1b[2mUpdating tree_walker\x1b[0m \x1b[1mv{current_str}\x1b[0m \x1b[2m→\x1b[0m \x1b[1mv{latest}\x1b[0m\x1b[2m...\x1b[0m");

    let asset = tree_walker_asset_name();
    let download_url = release_asset_url(&client, TREE_WALKER_REPO, &latest, asset).await?;

    let dest = if let Ok(p) = which::which("tree_walker") { p } else { epc_install_dir()?.join("tree_walker") };
    let tmp = dest.with_extension("tmp");
    download_binary_with_client(&client, &download_url, &tmp).await?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755))?;
    }

    std::fs::rename(&tmp, &dest)
        .with_context(|| format!("failed to replace binary at {}", dest.display()))?;

    println!("\n\x1b[32m✓\x1b[0m \x1b[1mtree_walker v{latest}\x1b[0m installed.");
    Ok(())
}

// ── upgrade ───────────────────────────────────────────────────────────────────

async fn run_upgrade(only: Option<&str>) -> Result<()> {
    match only {
        Some("observatory") => {
            println!("\x1b[2mobservatory is your harness — you own the source.\x1b[0m");
            println!("\x1b[2mTo pick up changes after editing, run:\x1b[0m \x1b[36mepc restart observatory\x1b[0m");
            Ok(())
        }
        Some("tree_walker") => upgrade_tree_walker().await,
        Some("epc") | None => upgrade_epc().await,
        Some(other) => anyhow::bail!(
            "unknown component '{other}' — valid options: epc, observatory, tree_walker"
        ),
    }
}

async fn upgrade_epc() -> Result<()> {
    eprintln!("\x1b[2mChecking for epc updates...\x1b[0m");

    let client = http_client()?;
    let latest = latest_version(&client, EPC_REPO).await?;
    let current = epc_installed_version();

    let current_str = current.as_deref().unwrap_or("unknown");

    if let Some(ref c) = current {
        let cv = Version::parse(c).ok();
        let lv = Version::parse(&latest).ok();
        if cv.is_some() && cv >= lv {
            println!("\x1b[32m✓\x1b[0m \x1b[1mepc\x1b[0m \x1b[2malready up to date (v{c})\x1b[0m");
            return Ok(());
        }
    }

    println!("\x1b[2mUpdating epc\x1b[0m \x1b[1mv{current_str}\x1b[0m \x1b[2m→\x1b[0m \x1b[1mv{latest}\x1b[0m\x1b[2m...\x1b[0m");

    let asset = epc_asset_name();
    let download_url = release_asset_url(&client, EPC_REPO, &latest, asset).await?;

    // Install to wherever epc currently lives, or default
    let dest = epc_binary_path()?;
    let tmp = dest.with_extension("tmp");
    download_binary_with_client(&client, &download_url, &tmp).await?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o755))?;
    }

    std::fs::rename(&tmp, &dest)
        .with_context(|| format!("failed to replace binary at {}", dest.display()))?;

    println!("\n\x1b[32m✓\x1b[0m \x1b[1mepc v{latest}\x1b[0m installed.");
    Ok(())
}

// ── status ────────────────────────────────────────────────────────────────────

async fn run_status() -> Result<()> {
    let client = http_client()?;

    // epc
    let epc_current = epc_installed_version();
    let epc_latest = latest_version(&client, EPC_REPO).await.ok();
    print_status("epc", epc_current.as_deref(), epc_latest.as_deref(), "epm runtime upgrade epc");

    // observatory
    let obs_current = observatory_installed_version();
    let obs_latest = latest_version(&client, OBSERVATORY_REPO).await.ok();
    print_status("observatory", obs_current.as_deref(), obs_latest.as_deref(),
        "cd ~/observatory && git pull");

    // tree_walker
    let tw_current = tree_walker_installed_version();
    let tw_latest = latest_version(&client, TREE_WALKER_REPO).await.ok();
    print_status("tree_walker", tw_current.as_deref(), tw_latest.as_deref(),
        "epm runtime upgrade tree_walker");

    Ok(())
}

fn print_status(name: &str, current: Option<&str>, latest: Option<&str>, upgrade_hint: &str) {
    let current_str = current.unwrap_or("not installed");
    let latest_str = latest.unwrap_or("unavailable");

    let up_to_date = match (current, latest) {
        (Some(c), Some(l)) => {
            Version::parse(c).ok() >= Version::parse(l).ok()
        }
        _ => false,
    };

    if current.is_none() {
        let latest_display = if latest.is_some() { format!("  (latest: v{latest_str})") } else { String::new() };
        println!("\x1b[31m✕\x1b[0m \x1b[1m{name}\x1b[0m \x1b[2mnot installed{latest_display}\x1b[0m");
        println!("    \x1b[36mepm runtime install {name}\x1b[0m");
    } else if up_to_date {
        println!("\x1b[32m✓\x1b[0m \x1b[1m{name}\x1b[0m \x1b[2mv{current_str}\x1b[0m");
    } else {
        println!("\x1b[33m!\x1b[0m \x1b[1m{name}\x1b[0m \x1b[2mv{current_str}  →  v{latest_str} available\x1b[0m");
        println!("    \x1b[36m{upgrade_hint}\x1b[0m");
    }
    println!();
}

// ── GitHub helpers ────────────────────────────────────────────────────────────

fn http_client() -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(10))
        .user_agent(format!("epm/{}", env!("CARGO_PKG_VERSION")))
        .build()?)
}

async fn latest_version(client: &reqwest::Client, repo: &str) -> Result<String> {
    let resp = client
        .get(format!("https://api.github.com/repos/{repo}/releases/latest"))
        .send().await?
        .json::<serde_json::Value>().await?;

    resp["tag_name"]
        .as_str()
        .map(|s| s.trim_start_matches('v').to_string())
        .filter(|s| !s.is_empty())
        .ok_or_else(|| anyhow::anyhow!("could not determine latest version for {repo}"))
}

async fn latest_release_download_url(repo: &str, asset: &str) -> Result<(String, String)> {
    let client = http_client()?;
    let version = latest_version(&client, repo).await?;
    let url = release_asset_url(&client, repo, &version, asset).await?;
    Ok((url, version))
}

async fn release_asset_url(client: &reqwest::Client, repo: &str, version: &str, asset: &str) -> Result<String> {
    let resp = client
        .get(format!("https://api.github.com/repos/{repo}/releases/tags/v{version}"))
        .send().await?
        .json::<serde_json::Value>().await?;

    resp["assets"]
        .as_array()
        .and_then(|a| a.iter().find(|x| x["name"].as_str() == Some(asset)))
        .and_then(|a| a["browser_download_url"].as_str())
        .map(String::from)
        .ok_or_else(|| anyhow::anyhow!("asset '{asset}' not found in release v{version} of {repo}"))
}

async fn download_binary(url: &str, dest: &std::path::Path) -> Result<()> {
    let client = http_client()?;
    download_binary_with_client(&client, url, dest).await
}

async fn download_binary_with_client(
    client: &reqwest::Client,
    url: &str,
    dest: &std::path::Path,
) -> Result<()> {
    let bytes = client.get(url).send().await?.bytes().await?;

    if let Some(parent) = dest.parent() {
        std::fs::create_dir_all(parent)?;
    }

    let mut f = std::fs::File::create(dest)
        .with_context(|| format!("failed to create {}", dest.display()))?;
    f.write_all(&bytes)?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        std::fs::set_permissions(dest, std::fs::Permissions::from_mode(0o755))?;
    }

    Ok(())
}

// ── local state helpers ───────────────────────────────────────────────────────

fn epc_asset_name() -> &'static str {
    if cfg!(target_os = "macos") {
        if cfg!(target_arch = "aarch64") { "epc-macos-aarch64" } else { "epc-macos-x86_64" }
    } else {
        "epc-linux-x86_64"
    }
}

fn epc_install_dir() -> Result<std::path::PathBuf> {
    Ok(std::path::PathBuf::from("/usr/local/bin"))
}

fn epc_binary_path() -> Result<std::path::PathBuf> {
    // If epc is already on PATH, replace it in place
    if let Ok(p) = which::which("epc") {
        return Ok(p);
    }
    Ok(epc_install_dir()?.join("epc"))
}

fn epc_installed_version() -> Option<String> {
    let out = Command::new("epc").arg("--version").output().ok()?;
    let s = String::from_utf8_lossy(&out.stdout);
    // "epc 0.1.5" → "0.1.5"
    s.trim().split_whitespace().nth(1).map(String::from)
}

fn tree_walker_asset_name() -> &'static str {
    if cfg!(target_os = "macos") {
        if cfg!(target_arch = "aarch64") { "tree_walker-macos-aarch64" } else { "tree_walker-macos-x86_64" }
    } else {
        "tree_walker-linux-x86_64"
    }
}

fn tree_walker_installed_version() -> Option<String> {
    let out = Command::new("tree_walker").arg("--version").output().ok()?;
    let s = String::from_utf8_lossy(&out.stdout);
    // "tree_walker 0.1.0" → "0.1.0"
    s.trim().split_whitespace().nth(1).map(String::from)
}

fn observatory_installed_version() -> Option<String> {
    let home = dirs::home_dir()?;
    let eps = home.join(OBSERVATORY_DEST).join("eps.toml");
    let content = std::fs::read_to_string(eps).ok()?;
    // Extract version = "x.y.z" from eps.toml
    for line in content.lines() {
        let line = line.trim();
        if line.starts_with("version") {
            if let Some(v) = line.split('"').nth(1) {
                return Some(v.to_string());
            }
        }
    }
    None
}
