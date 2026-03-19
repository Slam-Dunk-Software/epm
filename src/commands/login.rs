use anyhow::{bail, Result};
use serde::Deserialize;

use crate::credentials;

#[derive(Debug, PartialEq)]
pub enum PollStatus {
    Pending,
    Complete(String),
    NotFound,
}

#[derive(Deserialize)]
struct PollResponse {
    status: String,
    token: Option<String>,
}

pub fn parse_poll_response(body: &str) -> Result<PollStatus> {
    let resp: PollResponse = serde_json::from_str(body)?;
    match resp.status.as_str() {
        "pending" => Ok(PollStatus::Pending),
        "complete" => {
            let token = resp.token.ok_or_else(|| anyhow::anyhow!("complete response missing token"))?;
            Ok(PollStatus::Complete(token))
        }
        "not_found" => Ok(PollStatus::NotFound),
        other => bail!("unexpected poll status: {other}"),
    }
}

pub fn run_with_token(registry: &str, token: &str, creds_path: Option<&std::path::Path>) -> Result<()> {
    if let Some(path) = creds_path {
        credentials::save_to(path, registry, token)?;
    } else {
        credentials::save(registry, token)?;
    }
    println!("\x1b[32m✓\x1b[0m Token saved. You can now run \x1b[36mepm publish\x1b[0m without --token.");
    Ok(())
}

pub async fn run(registry: &str, token_flag: Option<&str>) -> Result<()> {
    if let Some(token) = token_flag {
        return run_with_token(registry, token, None);
    }

    // Generate a random session key
    let session_key = generate_session_key();
    let auth_url = format!("{registry}/auth/start?session_key={session_key}");

    println!("Opening browser for GitHub authentication...");
    println!("If the browser doesn't open, navigate to:\n  \x1b[36m{auth_url}\x1b[0m\n");

    let _ = open::that(&auth_url);

    println!("Waiting for authentication... (Ctrl+C to cancel)");
    poll_for_token(registry, &session_key, None).await
}

pub async fn poll_for_token(
    registry: &str,
    session_key: &str,
    creds_path: Option<&std::path::Path>,
) -> Result<()> {
    let client = reqwest::Client::new();
    let poll_url = format!("{registry}/api/v1/auth/poll/{session_key}");

    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(300); // 5-minute timeout

    loop {
        if std::time::Instant::now() >= deadline {
            bail!("Login timed out after 5 minutes.\nTo set a token manually: epm login --token <token>");
        }

        let resp = client
            .get(&poll_url)
            .send()
            .await
            .map_err(|e| anyhow::anyhow!("failed to poll registry: {e}"))?;

        let body = resp.text().await?;
        match parse_poll_response(&body)? {
            PollStatus::Pending => {
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
            }
            PollStatus::Complete(token) => {
                if let Some(path) = creds_path {
                    credentials::save_to(path, registry, &token)?;
                } else {
                    credentials::save(registry, &token)?;
                }
                println!("\x1b[32m✓\x1b[0m Authenticated! Token saved to ~/.epm/credentials.");
                println!("  You can now run \x1b[36mepm publish\x1b[0m without --token.");
                return Ok(());
            }
            PollStatus::NotFound => {
                bail!("Session expired or not found. Run epm login again.");
            }
        }
    }
}

fn generate_session_key() -> String {
    use rand::Rng;
    let mut rng = rand::thread_rng();
    let bytes: [u8; 16] = rng.gen();
    hex::encode(bytes)
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn parse_pending_response() {
        let body = r#"{"status":"pending"}"#;
        assert_eq!(parse_poll_response(body).unwrap(), PollStatus::Pending);
    }

    #[test]
    fn parse_complete_response() {
        let body = r#"{"status":"complete","token":"abc123def456"}"#;
        assert_eq!(
            parse_poll_response(body).unwrap(),
            PollStatus::Complete("abc123def456".to_string())
        );
    }

    #[test]
    fn parse_not_found_response() {
        let body = r#"{"status":"not_found"}"#;
        assert_eq!(parse_poll_response(body).unwrap(), PollStatus::NotFound);
    }

    #[test]
    fn parse_complete_missing_token_errors() {
        let body = r#"{"status":"complete"}"#;
        assert!(parse_poll_response(body).is_err());
    }

    #[test]
    fn parse_unknown_status_errors() {
        let body = r#"{"status":"unknown_thing"}"#;
        assert!(parse_poll_response(body).is_err());
    }

    #[test]
    fn run_with_token_writes_credentials() {
        let dir = TempDir::new().unwrap();
        let creds_path = dir.path().join(".epm").join("credentials");

        run_with_token("https://epm.dev", "my_test_token", Some(&creds_path)).unwrap();

        let loaded = credentials::load_from(&creds_path, "https://epm.dev").unwrap();
        assert_eq!(loaded, Some("my_test_token".to_string()));
    }

    #[test]
    fn run_with_token_prints_success_message() {
        let dir = TempDir::new().unwrap();
        let creds_path = dir.path().join(".epm").join("credentials");
        // Just verify it doesn't error
        assert!(run_with_token("https://epm.dev", "tok", Some(&creds_path)).is_ok());
    }
}
