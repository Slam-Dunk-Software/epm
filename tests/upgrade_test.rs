mod common;

use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::json;
use tempfile::TempDir;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const PKG: &str = "tech_talker";

fn epm_upgrade(registry: &str, home: &TempDir, name: &str) -> assert_cmd::assert::Assert {
    Command::cargo_bin("epm")
        .unwrap()
        .args(["--registry", registry, "upgrade", name])
        .env("HOME", home.path())
        .assert()
}

fn install_path(home: &TempDir, version: &str) -> std::path::PathBuf {
    home.path()
        .join(".epm")
        .join("packages")
        .join(PKG)
        .join(version)
}

#[tokio::test(flavor = "multi_thread")]
async fn upgrade_installs_newer_version() {
    let repo = common::TestRepo::create();
    let server = MockServer::start().await;

    // Registry knows about 0.2.0
    let ver_020 = common::version_json(2, 1, "0.2.0", &repo.url, &repo.sha, false);
    let pkg = common::package_with_versions_json(1, PKG, vec![ver_020.clone()]);
    Mock::given(method("GET"))
        .and(path(format!("/api/v1/packages/{PKG}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(pkg))
        .mount(&server)
        .await;

    // install::run will hit the pinned-version endpoint
    Mock::given(method("GET"))
        .and(path(format!("/api/v1/packages/{PKG}/0.2.0")))
        .respond_with(ResponseTemplate::new(200).set_body_json(ver_020))
        .mount(&server)
        .await;

    let home = TempDir::new().unwrap();
    // Simulate 0.1.0 already installed
    std::fs::create_dir_all(install_path(&home, "0.1.0")).unwrap();

    epm_upgrade(&server.uri(), &home, PKG).success();

    assert!(install_path(&home, "0.2.0").exists(), "0.2.0 should be installed");
}

#[tokio::test(flavor = "multi_thread")]
async fn upgrade_already_up_to_date_exits_zero() {
    let server = MockServer::start().await;

    let ver = common::version_json(1, 1, "0.1.0", "https://github.com/test/test", "abc123", false);
    let pkg = common::package_with_versions_json(1, PKG, vec![ver]);
    Mock::given(method("GET"))
        .and(path(format!("/api/v1/packages/{PKG}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(pkg))
        .mount(&server)
        .await;

    let home = TempDir::new().unwrap();
    // 0.1.0 already installed
    std::fs::create_dir_all(install_path(&home, "0.1.0")).unwrap();

    epm_upgrade(&server.uri(), &home, PKG)
        .success()
        .stdout(predicate::str::contains("already up to date"));
}

#[tokio::test(flavor = "multi_thread")]
async fn upgrade_package_not_found_exits_nonzero() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/api/v1/packages/{PKG}")))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;

    let home = TempDir::new().unwrap();
    epm_upgrade(&server.uri(), &home, PKG).failure();
}

#[tokio::test(flavor = "multi_thread")]
async fn upgrade_no_eligible_versions_exits_nonzero() {
    let server = MockServer::start().await;
    let yanked_ver = json!({
        "id": 1,
        "package_id": 1,
        "version": "0.1.0",
        "git_url": "https://github.com/test/test",
        "commit_sha": "abc123",
        "manifest_hash": "def456",
        "yanked": true,
        "published_at": "2025-01-01T00:00:00",
        "system_deps": {}
    });
    let pkg = common::package_with_versions_json(1, PKG, vec![yanked_ver]);
    Mock::given(method("GET"))
        .and(path(format!("/api/v1/packages/{PKG}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(pkg))
        .mount(&server)
        .await;

    let home = TempDir::new().unwrap();
    epm_upgrade(&server.uri(), &home, PKG).failure();
}
