use std::time::Duration;

use anyhow::{bail, Context, Result};
use reqwest::Client;

use crate::models::{Package, PackageWithVersions, PublishRequest, PublishedVersion, Version};

pub struct RegistryClient {
    base_url: String,
    client:   Client,
    token:    Option<String>,
}

impl RegistryClient {
    pub fn new(base_url: &str, token: Option<String>) -> Self {
        Self {
            base_url: base_url.trim_end_matches('/').to_string(),
            client:   Client::builder()
                .timeout(Duration::from_secs(30))
                .build()
                .expect("failed to build HTTP client"),
            token,
        }
    }

    pub async fn list_packages(&self) -> Result<Vec<Package>> {
        let url = format!("{}/api/v1/packages", self.base_url);
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .with_context(|| format!("failed to reach registry at {url}"))?;

        if !resp.status().is_success() {
            bail!("registry returned {}", resp.status());
        }

        resp.json::<Vec<Package>>()
            .await
            .context("failed to parse package list")
    }

    pub async fn get_package(&self, name: &str) -> Result<PackageWithVersions> {
        let url = format!("{}/api/v1/packages/{name}", self.base_url);
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .with_context(|| format!("failed to reach registry at {url}"))?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            bail!("package '{name}' not found");
        }
        if !resp.status().is_success() {
            bail!("registry returned {}", resp.status());
        }

        resp.json::<PackageWithVersions>()
            .await
            .context("failed to parse package")
    }

    pub async fn publish_package(&self, req: &PublishRequest) -> Result<PublishedVersion> {
        let url = format!("{}/api/v1/packages", self.base_url);
        let mut builder = self.client.post(&url).json(req);
        if let Some(token) = &self.token {
            builder = builder.header("authorization", format!("Bearer {token}"));
        }
        let resp = builder
            .send()
            .await
            .with_context(|| format!("failed to reach registry at {url}"))?;

        if resp.status() == reqwest::StatusCode::UNAUTHORIZED {
            bail!("publish failed: unauthorized — set EPM_PUBLISH_TOKEN or pass --token");
        }
        if resp.status() == reqwest::StatusCode::CONFLICT {
            bail!("version already exists — cannot re-publish an immutable version");
        }
        if !resp.status().is_success() {
            bail!("registry returned {}", resp.status());
        }

        resp.json::<PublishedVersion>()
            .await
            .context("failed to parse publish response")
    }

    /// Fire-and-forget install tracking — never fails the caller.
    pub async fn track_install(&self, name: &str, version: &str) {
        let url = format!("{}/api/v1/packages/{name}/installs", self.base_url);
        let body = serde_json::json!({ "version": version });
        let _ = self.client.post(&url).json(&body).send().await;
    }

    pub async fn get_version(&self, name: &str, version: &str) -> Result<Version> {
        let url = format!("{}/api/v1/packages/{name}/{version}", self.base_url);
        let resp = self
            .client
            .get(&url)
            .send()
            .await
            .with_context(|| format!("failed to reach registry at {url}"))?;

        if resp.status() == reqwest::StatusCode::NOT_FOUND {
            bail!("version '{version}' of package '{name}' not found");
        }
        if !resp.status().is_success() {
            bail!("registry returned {}", resp.status());
        }

        resp.json::<Version>()
            .await
            .context("failed to parse version")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use wiremock::matchers::{method, path};
    use wiremock::{Mock, MockServer, ResponseTemplate};

    fn pkg_json(id: i64, name: &str) -> serde_json::Value {
        json!({
            "id": id,
            "name": name,
            "description": "A test package",
            "authors": ["test"],
            "license": "MIT",
            "homepage": null,
            "repository": "https://github.com/test/test",
            "platforms": ["aarch64-apple-darwin"],
            "created_at": "2025-01-01T00:00:00",
            "updated_at": "2025-01-01T00:00:00"
        })
    }

    fn pkg_with_versions_json(id: i64, name: &str, versions: Vec<serde_json::Value>) -> serde_json::Value {
        json!({
            "id": id,
            "name": name,
            "description": "A test package",
            "authors": ["test"],
            "license": "MIT",
            "homepage": null,
            "repository": "https://github.com/test/test",
            "platforms": ["aarch64-apple-darwin"],
            "created_at": "2025-01-01T00:00:00",
            "updated_at": "2025-01-01T00:00:00",
            "versions": versions
        })
    }

    fn ver_json(id: i64, pkg_id: i64, version: &str) -> serde_json::Value {
        json!({
            "id": id,
            "package_id": pkg_id,
            "version": version,
            "git_url": "https://github.com/test/test",
            "commit_sha": "abc123",
            "manifest_hash": "def456",
            "yanked": false,
            "published_at": "2025-01-01T00:00:00",
            "system_deps": {}
        })
    }

    // --- list_packages ---

    #[tokio::test]
    async fn list_packages_returns_empty_list() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/packages"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
            .mount(&server)
            .await;

        let client = RegistryClient::new(&server.uri(), None);
        let result = client.list_packages().await.unwrap();
        assert!(result.is_empty());
    }

    #[tokio::test]
    async fn list_packages_returns_multiple_packages() {
        let server = MockServer::start().await;
        let body = json!([pkg_json(1, "tech_talker"), pkg_json(2, "pi")]);
        Mock::given(method("GET"))
            .and(path("/api/v1/packages"))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .mount(&server)
            .await;

        let client = RegistryClient::new(&server.uri(), None);
        let result = client.list_packages().await.unwrap();
        assert_eq!(result.len(), 2);
        assert_eq!(result[0].name, "tech_talker");
        assert_eq!(result[1].name, "pi");
    }

    #[tokio::test]
    async fn list_packages_server_error_returns_err() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/packages"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;

        let client = RegistryClient::new(&server.uri(), None);
        let result = client.list_packages().await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("500"));
    }

    // --- get_package ---

    #[tokio::test]
    async fn get_package_returns_package_with_versions() {
        let server = MockServer::start().await;
        let ver = ver_json(1, 1, "0.1.0");
        let body = pkg_with_versions_json(1, "tech_talker", vec![ver]);
        Mock::given(method("GET"))
            .and(path("/api/v1/packages/tech_talker"))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .mount(&server)
            .await;

        let client = RegistryClient::new(&server.uri(), None);
        let result = client.get_package("tech_talker").await.unwrap();
        assert_eq!(result.name, "tech_talker");
        assert_eq!(result.versions.len(), 1);
        assert_eq!(result.versions[0].version, "0.1.0");
    }

    #[tokio::test]
    async fn get_package_not_found_returns_descriptive_err() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/packages/no_such_pkg"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        let client = RegistryClient::new(&server.uri(), None);
        let err = client.get_package("no_such_pkg").await.unwrap_err();
        assert!(err.to_string().contains("no_such_pkg"));
        assert!(err.to_string().contains("not found"));
    }

    #[tokio::test]
    async fn get_package_server_error_returns_err() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/packages/bad_pkg"))
            .respond_with(ResponseTemplate::new(503))
            .mount(&server)
            .await;

        let client = RegistryClient::new(&server.uri(), None);
        let result = client.get_package("bad_pkg").await;
        assert!(result.is_err());
        assert!(result.unwrap_err().to_string().contains("503"));
    }

    #[tokio::test]
    async fn get_package_includes_multiple_versions() {
        let server = MockServer::start().await;
        let versions = vec![ver_json(3, 1, "0.3.0"), ver_json(2, 1, "0.2.0"), ver_json(1, 1, "0.1.0")];
        let body = pkg_with_versions_json(1, "multi_ver", versions);
        Mock::given(method("GET"))
            .and(path("/api/v1/packages/multi_ver"))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .mount(&server)
            .await;

        let client = RegistryClient::new(&server.uri(), None);
        let result = client.get_package("multi_ver").await.unwrap();
        assert_eq!(result.versions.len(), 3);
        assert_eq!(result.versions[0].version, "0.3.0");
    }

    // --- get_version ---

    #[tokio::test]
    async fn get_version_returns_version() {
        let server = MockServer::start().await;
        let body = ver_json(1, 1, "0.1.0");
        Mock::given(method("GET"))
            .and(path("/api/v1/packages/tech_talker/0.1.0"))
            .respond_with(ResponseTemplate::new(200).set_body_json(body))
            .mount(&server)
            .await;

        let client = RegistryClient::new(&server.uri(), None);
        let result = client.get_version("tech_talker", "0.1.0").await.unwrap();
        assert_eq!(result.version, "0.1.0");
        assert!(!result.yanked);
    }

    #[tokio::test]
    async fn get_version_not_found_returns_descriptive_err() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/packages/tech_talker/9.9.9"))
            .respond_with(ResponseTemplate::new(404))
            .mount(&server)
            .await;

        let client = RegistryClient::new(&server.uri(), None);
        let err = client.get_version("tech_talker", "9.9.9").await.unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("9.9.9"), "expected version in error: {msg}");
        assert!(msg.contains("tech_talker"), "expected name in error: {msg}");
        assert!(msg.contains("not found"), "expected 'not found' in error: {msg}");
    }

    #[tokio::test]
    async fn get_version_server_error_returns_err() {
        let server = MockServer::start().await;
        Mock::given(method("GET"))
            .and(path("/api/v1/packages/tech_talker/0.1.0"))
            .respond_with(ResponseTemplate::new(500))
            .mount(&server)
            .await;

        let client = RegistryClient::new(&server.uri(), None);
        let result = client.get_version("tech_talker", "0.1.0").await;
        assert!(result.is_err());
    }

    // --- base URL handling ---

    #[tokio::test]
    async fn base_url_trailing_slash_is_stripped() {
        let server = MockServer::start().await;
        // Only responds to /api/v1/packages (exact path); double-slash would miss
        Mock::given(method("GET"))
            .and(path("/api/v1/packages"))
            .respond_with(ResponseTemplate::new(200).set_body_json(json!([])))
            .mount(&server)
            .await;

        // Pass URL with trailing slash — client must strip it
        let uri_with_slash = format!("{}/", server.uri());
        let client = RegistryClient::new(&uri_with_slash, None);
        let result = client.list_packages().await;
        assert!(result.is_ok(), "expected success but got: {:?}", result.err());
    }
}
