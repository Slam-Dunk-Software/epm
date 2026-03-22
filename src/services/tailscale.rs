use anyhow::Result;
use serde::Deserialize;

#[derive(Deserialize)]
struct TailscaleStatus {
    #[serde(rename = "Self")]
    self_node: Option<SelfNode>,
}

#[derive(Deserialize)]
struct SelfNode {
    #[serde(rename = "TailscaleIPs")]
    tailscale_ips: Option<Vec<String>>,
}

/// Returns the Tailscale IPv4 address for this machine (e.g. `100.78.103.79`).
/// Falls back to `"localhost"` if Tailscale is unavailable or not connected.
pub async fn ip() -> Result<String> {
    let output = tokio::process::Command::new("tailscale")
        .args(["status", "--json"])
        .output()
        .await;

    let output = match output {
        Ok(o) if o.status.success() => o,
        _ => return Ok("localhost".to_string()),
    };

    let status: TailscaleStatus = match serde_json::from_slice(&output.stdout) {
        Ok(s) => s,
        Err(_) => return Ok("localhost".to_string()),
    };

    let addr = status
        .self_node
        .and_then(|n| n.tailscale_ips)
        .and_then(|ips| ips.into_iter().find(|ip| ip.starts_with("100.")))
        .unwrap_or_else(|| "localhost".to_string());

    Ok(addr)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn ip_returns_non_empty_string() {
        let addr = ip().await.unwrap();
        assert!(!addr.is_empty());
    }

    #[tokio::test]
    async fn ip_is_tailscale_or_localhost() {
        let addr = ip().await.unwrap();
        assert!(
            addr == "localhost" || addr.starts_with("100."),
            "unexpected address: {addr}"
        );
    }
}
