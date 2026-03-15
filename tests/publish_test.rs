mod common;

use assert_cmd::prelude::*;
use predicates::prelude::*;
use serde_json::json;
use std::process::Command;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const MANIFEST: &str = r#"
[package]
name        = "tech_talker"
version     = "0.1.0"
description = "Audio transcription harness"
authors     = ["nickagliano"]
license     = "MIT"
repository  = "https://github.com/nickagliano/tech_talker"
platform    = ["aarch64-apple-darwin"]
"#;

const MANIFEST_WITH_SYSTEM_DEPS: &str = r#"
[package]
name        = "tech_talker"
version     = "0.1.0"
description = "Audio transcription harness"
authors     = ["nickagliano"]
license     = "MIT"
repository  = "https://github.com/nickagliano/tech_talker"
platform    = ["aarch64-apple-darwin"]

[system-dependencies]
brew = ["cmake"]
"#;

fn version_body() -> serde_json::Value {
    json!({
        "id": 1,
        "package_id": 1,
        "version": "0.1.0",
        "git_url": "https://github.com/nickagliano/tech_talker",
        "commit_sha": "abc",
        "manifest_hash": "sha256:xx",
        "yanked": false,
        "published_at": "2025-01-01T00:00:00",
        "system_deps": {}
    })
}

#[tokio::test(flavor = "multi_thread")]
async fn publish_exits_zero_on_success() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/packages"))
        .respond_with(ResponseTemplate::new(201).set_body_json(version_body()))
        .mount(&server)
        .await;

    let repo = common::TestRepo::create_with_manifest(MANIFEST);
    Command::cargo_bin("epm")
        .unwrap()
        .args(["publish"])
        .env("EPM_REGISTRY", &server.uri())
        .current_dir(repo._dir.path())
        .assert()
        .success();
}

#[tokio::test(flavor = "multi_thread")]
async fn publish_prints_name_at_version() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/packages"))
        .respond_with(ResponseTemplate::new(201).set_body_json(version_body()))
        .mount(&server)
        .await;

    let repo = common::TestRepo::create_with_manifest(MANIFEST);
    Command::cargo_bin("epm")
        .unwrap()
        .args(["publish"])
        .env("EPM_REGISTRY", &server.uri())
        .current_dir(repo._dir.path())
        .assert()
        .success()
        .stdout(predicate::str::contains("tech_talker@0.1.0"));
}

#[tokio::test(flavor = "multi_thread")]
async fn publish_sends_correct_commit_sha() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/packages"))
        .respond_with(ResponseTemplate::new(201).set_body_json(version_body()))
        .mount(&server)
        .await;

    let repo = common::TestRepo::create_with_manifest(MANIFEST);
    let expected_sha = repo.sha.clone();

    Command::cargo_bin("epm")
        .unwrap()
        .args(["publish"])
        .env("EPM_REGISTRY", &server.uri())
        .current_dir(repo._dir.path())
        .assert()
        .success();

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    let body: serde_json::Value = serde_json::from_slice(&requests[0].body).unwrap();
    assert_eq!(body["commit_sha"], expected_sha.as_str());
}

#[tokio::test(flavor = "multi_thread")]
async fn publish_sends_manifest_name_and_version() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/packages"))
        .respond_with(ResponseTemplate::new(201).set_body_json(version_body()))
        .mount(&server)
        .await;

    let repo = common::TestRepo::create_with_manifest(MANIFEST);
    Command::cargo_bin("epm")
        .unwrap()
        .args(["publish"])
        .env("EPM_REGISTRY", &server.uri())
        .current_dir(repo._dir.path())
        .assert()
        .success();

    let requests = server.received_requests().await.unwrap();
    let body: serde_json::Value = serde_json::from_slice(&requests[0].body).unwrap();
    assert_eq!(body["name"], "tech_talker");
    assert_eq!(body["version"], "0.1.0");
}

#[tokio::test(flavor = "multi_thread")]
async fn publish_fails_without_eps_toml() {
    let server = MockServer::start().await;
    // No mock needed — should fail before any HTTP request

    let repo = common::TestRepo::create(); // no eps.toml
    Command::cargo_bin("epm")
        .unwrap()
        .args(["publish"])
        .env("EPM_REGISTRY", &server.uri())
        .current_dir(repo._dir.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("eps.toml"));
}

#[tokio::test(flavor = "multi_thread")]
async fn publish_fails_with_malformed_toml() {
    let server = MockServer::start().await;

    let bad_manifest = "this is not [ valid toml !!!";
    let repo = common::TestRepo::create_with_manifest(bad_manifest);
    Command::cargo_bin("epm")
        .unwrap()
        .args(["publish"])
        .env("EPM_REGISTRY", &server.uri())
        .current_dir(repo._dir.path())
        .assert()
        .failure();
}

#[tokio::test(flavor = "multi_thread")]
async fn publish_409_shows_already_exists_in_stderr() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/packages"))
        .respond_with(ResponseTemplate::new(409).set_body_json(json!({"error": "version already exists"})))
        .mount(&server)
        .await;

    let repo = common::TestRepo::create_with_manifest(MANIFEST);
    Command::cargo_bin("epm")
        .unwrap()
        .args(["publish"])
        .env("EPM_REGISTRY", &server.uri())
        .current_dir(repo._dir.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("already exists"));
}

#[tokio::test(flavor = "multi_thread")]
async fn publish_registry_unreachable_exits_nonzero() {
    let repo = common::TestRepo::create_with_manifest(MANIFEST);
    Command::cargo_bin("epm")
        .unwrap()
        .args(["publish"])
        .env("EPM_REGISTRY", "http://127.0.0.1:59876")
        .current_dir(repo._dir.path())
        .assert()
        .failure();
}

#[tokio::test(flavor = "multi_thread")]
async fn publish_sends_system_deps() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/packages"))
        .respond_with(ResponseTemplate::new(201).set_body_json(version_body()))
        .mount(&server)
        .await;

    let repo = common::TestRepo::create_with_manifest(MANIFEST_WITH_SYSTEM_DEPS);
    Command::cargo_bin("epm")
        .unwrap()
        .args(["publish"])
        .env("EPM_REGISTRY", &server.uri())
        .current_dir(repo._dir.path())
        .assert()
        .success();

    let requests = server.received_requests().await.unwrap();
    assert_eq!(requests.len(), 1);
    let body: serde_json::Value = serde_json::from_slice(&requests[0].body).unwrap();
    let brew_deps = body["system_deps"]["brew"].as_array().unwrap();
    assert!(brew_deps.iter().any(|v| v == "cmake"));
}

#[tokio::test(flavor = "multi_thread")]
async fn publish_401_shows_unauthorized_error() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/packages"))
        .respond_with(ResponseTemplate::new(401))
        .mount(&server)
        .await;

    let repo = common::TestRepo::create_with_manifest(MANIFEST);
    Command::cargo_bin("epm")
        .unwrap()
        .args(["publish"])
        .env("EPM_REGISTRY", &server.uri())
        .current_dir(repo._dir.path())
        .assert()
        .failure()
        .stderr(predicate::str::contains("EPM_PUBLISH_TOKEN"))
        .stderr(predicate::str::contains("--token"));
}

#[tokio::test(flavor = "multi_thread")]
async fn publish_token_flag_sends_authorization_header() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/packages"))
        .respond_with(ResponseTemplate::new(201).set_body_json(version_body()))
        .mount(&server)
        .await;

    let repo = common::TestRepo::create_with_manifest(MANIFEST);
    Command::cargo_bin("epm")
        .unwrap()
        .args(["--token", "mytoken", "publish"])
        .env("EPM_REGISTRY", &server.uri())
        .current_dir(repo._dir.path())
        .assert()
        .success();

    let requests = server.received_requests().await.unwrap();
    let auth = requests[0].headers.get("authorization").unwrap();
    assert_eq!(auth, "Bearer mytoken");
}

#[tokio::test(flavor = "multi_thread")]
async fn publish_token_env_var_sends_authorization_header() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/api/v1/packages"))
        .respond_with(ResponseTemplate::new(201).set_body_json(version_body()))
        .mount(&server)
        .await;

    let repo = common::TestRepo::create_with_manifest(MANIFEST);
    Command::cargo_bin("epm")
        .unwrap()
        .args(["publish"])
        .env("EPM_REGISTRY", &server.uri())
        .env("EPM_PUBLISH_TOKEN", "envtoken")
        .current_dir(repo._dir.path())
        .assert()
        .success();

    let requests = server.received_requests().await.unwrap();
    let auth = requests[0].headers.get("authorization").unwrap();
    assert_eq!(auth, "Bearer envtoken");
}
