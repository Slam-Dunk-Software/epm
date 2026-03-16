use anyhow::Result;

const INSTALL_URL: &str =
    "https://raw.githubusercontent.com/Slam-Dunk-Software/epm/main/install.sh";

pub async fn run() -> Result<()> {
    println!("Checking for updates...");

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
        println!("Already up to date (v{current}).");
        return Ok(());
    }

    println!("Updating epm v{current} → v{latest}...");

    let status = std::process::Command::new("sh")
        .arg("-c")
        .arg(format!("curl -fsSL {INSTALL_URL} | sh"))
        .status()?;

    if !status.success() {
        anyhow::bail!("Update failed. Try manually: curl -fsSL {INSTALL_URL} | sh");
    }

    println!("\n✓ epm v{latest} installed.");
    Ok(())
}
