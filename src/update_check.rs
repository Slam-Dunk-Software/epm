use std::time::{SystemTime, UNIX_EPOCH};

const CHECK_INTERVAL_SECS: u64 = 60 * 60 * 24; // 24 hours
const CURRENT_VERSION: &str = env!("CARGO_PKG_VERSION");
const RELEASES_URL: &str =
    "https://api.github.com/repos/Slam-Dunk-Software/epm/releases/latest";

pub async fn check_and_warn() {
    if !should_check() {
        return;
    }
    if let Some(latest) = fetch_latest_version().await {
        record_check();
        if is_newer(&latest, CURRENT_VERSION) {
            eprintln!(
                "\n\x1b[33m╭─ update available: epm v{latest}\x1b[0m"
            );
            eprintln!(
                "\x1b[33m╰─ curl -fsSL https://raw.githubusercontent.com/Slam-Dunk-Software/epm/main/install.sh | sh\x1b[0m"
            );
        }
    }
}

fn stamp_path() -> Option<std::path::PathBuf> {
    dirs::home_dir().map(|h| h.join(".epm").join(".update_check"))
}

fn should_check() -> bool {
    let Some(path) = stamp_path() else { return true };
    let Ok(meta) = std::fs::metadata(&path) else { return true };
    let Ok(modified) = meta.modified() else { return true };
    let Ok(elapsed) = SystemTime::now().duration_since(modified) else { return true };
    elapsed.as_secs() >= CHECK_INTERVAL_SECS
}

fn record_check() {
    let Some(path) = stamp_path() else { return };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    let ts = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|d| d.as_secs().to_string())
        .unwrap_or_default();
    let _ = std::fs::write(path, ts);
}

async fn fetch_latest_version() -> Option<String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .user_agent(format!("epm/{CURRENT_VERSION}"))
        .build()
        .ok()?;

    let resp = client.get(RELEASES_URL).send().await.ok()?;
    let json: serde_json::Value = resp.json().await.ok()?;
    let tag = json["tag_name"].as_str()?;
    Some(tag.trim_start_matches('v').to_string())
}

fn is_newer(latest: &str, current: &str) -> bool {
    semver::Version::parse(latest).ok().zip(semver::Version::parse(current).ok())
        .map(|(l, c)| l > c)
        .unwrap_or(false)
}
