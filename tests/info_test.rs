mod common;

use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::json;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

/// Full package fixture with distinct timestamps and one version.
fn test_pkg() -> serde_json::Value {
    json!({
        "id": 1,
        "name": "tech_talker",
        "description": "A transcription harness",
        "authors": ["nickagliano"],
        "license": "MIT",
        "homepage": null,
        "repository": "https://github.com/nickagliano/tech_talker",
        "platforms": ["aarch64-apple-darwin"],
        "created_at": "2024-12-01T08:00:00",
        "updated_at": "2025-01-15T16:30:00",
        "versions": [
            {
                "id": 1,
                "package_id": 1,
                "version": "0.2.0",
                "git_url": "https://github.com/nickagliano/tech_talker",
                "commit_sha": "deadbeef",
                "manifest_hash": "cafebabe",
                "yanked": false,
                "published_at": "2025-01-15T16:30:00",
                "system_deps": {}
            }
        ]
    })
}

async fn setup_server(pkg: serde_json::Value) -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/packages/tech_talker"))
        .respond_with(ResponseTemplate::new(200).set_body_json(pkg))
        .mount(&server)
        .await;
    server
}

fn epm_info(registry: &str) -> Command {
    let mut cmd = Command::cargo_bin("epm").unwrap();
    cmd.args(["--registry", registry, "info", "tech_talker"]);
    cmd
}

// ── field output tests ────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn info_shows_name() {
    let server = setup_server(test_pkg()).await;
    epm_info(&server.uri())
        .assert()
        .success()
        .stdout(predicate::str::contains("tech_talker"));
}

#[tokio::test(flavor = "multi_thread")]
async fn info_shows_description() {
    let server = setup_server(test_pkg()).await;
    epm_info(&server.uri())
        .assert()
        .success()
        .stdout(predicate::str::contains("A transcription harness"));
}

#[tokio::test(flavor = "multi_thread")]
async fn info_shows_license() {
    let server = setup_server(test_pkg()).await;
    epm_info(&server.uri())
        .assert()
        .success()
        .stdout(predicate::str::contains("MIT"));
}

#[tokio::test(flavor = "multi_thread")]
async fn info_shows_repository() {
    let server = setup_server(test_pkg()).await;
    epm_info(&server.uri())
        .assert()
        .success()
        .stdout(predicate::str::contains("https://github.com/nickagliano/tech_talker"));
}

#[tokio::test(flavor = "multi_thread")]
async fn info_shows_homepage_when_present() {
    let pkg = json!({
        "id": 1,
        "name": "tech_talker",
        "description": "A transcription harness",
        "authors": ["nickagliano"],
        "license": "MIT",
        "homepage": "https://tech-talker.example.com",
        "repository": "https://github.com/nickagliano/tech_talker",
        "platforms": ["aarch64-apple-darwin"],
        "created_at": "2024-12-01T08:00:00",
        "updated_at": "2025-01-15T16:30:00",
        "versions": []
    });
    let server = setup_server(pkg).await;
    epm_info(&server.uri())
        .assert()
        .success()
        .stdout(predicate::str::contains("https://tech-talker.example.com"));
}

#[tokio::test(flavor = "multi_thread")]
async fn info_omits_homepage_line_when_null() {
    let server = setup_server(test_pkg()).await; // homepage is null
    epm_info(&server.uri())
        .assert()
        .success()
        .stdout(predicate::str::contains("Homepage:").not());
}

#[tokio::test(flavor = "multi_thread")]
async fn info_shows_authors() {
    let server = setup_server(test_pkg()).await;
    epm_info(&server.uri())
        .assert()
        .success()
        .stdout(predicate::str::contains("nickagliano"));
}

#[tokio::test(flavor = "multi_thread")]
async fn info_shows_platforms() {
    let server = setup_server(test_pkg()).await;
    epm_info(&server.uri())
        .assert()
        .success()
        .stdout(predicate::str::contains("aarch64-apple-darwin"));
}

#[tokio::test(flavor = "multi_thread")]
async fn info_shows_created_at() {
    let server = setup_server(test_pkg()).await;
    epm_info(&server.uri())
        .assert()
        .success()
        .stdout(predicate::str::contains("2024-12-01T08:00:00"));
}

#[tokio::test(flavor = "multi_thread")]
async fn info_shows_updated_at() {
    let server = setup_server(test_pkg()).await;
    epm_info(&server.uri())
        .assert()
        .success()
        .stdout(predicate::str::contains("2025-01-15T16:30:00"));
}

// ── version list ──────────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn info_shows_version_list() {
    let server = setup_server(test_pkg()).await;
    epm_info(&server.uri())
        .assert()
        .success()
        .stdout(predicate::str::contains("0.2.0"))
        .stdout(predicate::str::contains("Versions:"));
}

#[tokio::test(flavor = "multi_thread")]
async fn info_shows_no_versions_message_when_empty() {
    let pkg = json!({
        "id": 1,
        "name": "tech_talker",
        "description": "test description",
        "authors": ["nickagliano"],
        "license": "MIT",
        "homepage": null,
        "repository": "https://github.com/test/test",
        "platforms": ["aarch64-apple-darwin"],
        "created_at": "2025-01-01T00:00:00",
        "updated_at": "2025-01-01T00:00:00",
        "versions": []
    });
    let server = setup_server(pkg).await;
    epm_info(&server.uri())
        .assert()
        .success()
        .stdout(predicate::str::contains("No published versions"));
}

// ── not found ────────────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn info_package_not_found_exits_nonzero() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/packages/tech_talker"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    Command::cargo_bin("epm")
        .unwrap()
        .args(["--registry", &server.uri(), "info", "tech_talker"])
        .assert()
        .failure();
}

#[tokio::test(flavor = "multi_thread")]
async fn info_package_not_found_shows_error_in_stderr() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/api/v1/packages/tech_talker"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    Command::cargo_bin("epm")
        .unwrap()
        .args(["--registry", &server.uri(), "info", "tech_talker"])
        .assert()
        .failure()
        .stderr(predicate::str::contains("not found").or(predicate::str::contains("tech_talker")));
}
