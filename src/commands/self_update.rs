use anyhow::Result;

const INSTALL_URL: &str =
    "https://raw.githubusercontent.com/Slam-Dunk-Software/epm/main/install.sh";

pub async fn run() -> Result<()> {
    println!("\x1b[2mChecking for updates...\x1b[0m");

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .user_agent(format!("epm/{}", env!("CARGO_PKG_VERSION")))
        .build()?;

    let resp = client
        .get("https://api.github.com/repos/Slam-Dunk-Software/epm/releases/latest")
        .send()
        .await?;

    let json: serde_json::Value = resp.json().await?;
    let latest = json["tag_name"]
        .as_str()
        .unwrap_or("")
        .trim_start_matches('v');

    let current = env!("CARGO_PKG_VERSION");

    if latest.is_empty() {
        anyhow::bail!("Could not determine latest version. Check https://github.com/Slam-Dunk-Software/epm/releases");
    }

    let latest_ver = semver::Version::parse(latest)?;
    let current_ver = semver::Version::parse(current)?;

    if latest_ver <= current_ver {
        println!("\x1b[32m✓\x1b[0m Already up to date \x1b[2m(v{current})\x1b[0m");
        return Ok(());
    }

    println!("\x1b[2mUpdating epm\x1b[0m \x1b[1mv{current}\x1b[0m \x1b[2m→\x1b[0m \x1b[1mv{latest}\x1b[0m\x1b[2m...\x1b[0m");

    let status = std::process::Command::new("sh")
        .args(["-c", &format!("curl -fsSL {INSTALL_URL} | sh -s -- --quiet")])
        .status()?;

    if !status.success() {
        anyhow::bail!("Update failed. Try manually: curl -fsSL {INSTALL_URL} | sh");
    }

    println!("\n\x1b[2mRun \x1b[0m\x1b[36mepm --version\x1b[0m\x1b[2m to confirm. You may need to open a new terminal.\x1b[0m");
    Ok(())
}
