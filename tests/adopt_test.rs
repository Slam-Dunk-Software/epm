mod common;

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const PKG: &str = "ratatui_eps";
const VER: &str = "0.1.0";

const VALID_MANIFEST: &str = r#"[package]
name        = "ratatui_eps"
version     = "0.1.0"
description = "A personalized TUI framework"
authors     = ["nick"]
license     = "MIT"
repository  = "https://github.com/nick/ratatui_eps"
"#;

async fn mock_pkg(server: &MockServer, git_url: &str, sha: &str) {
    let ver = common::version_json(1, 1, VER, git_url, sha, false);
    let pkg = common::package_with_versions_json(1, PKG, vec![ver]);
    Mock::given(method("GET"))
        .and(path(format!("/api/v1/packages/{PKG}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(pkg))
        .mount(server)
        .await;
}

fn vendor_path(project: &TempDir) -> std::path::PathBuf {
    project.path().join("vendor").join(PKG)
}

fn epm_adopt(registry: &str, project: &TempDir, spec: &str) -> assert_cmd::assert::Assert {
    Command::cargo_bin("epm")
        .unwrap()
        .current_dir(project.path())
        .args(["adopt", spec])
        .env("EPM_REGISTRY", registry)
        .assert()
}

// ── directory / content ───────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn adopt_creates_vendor_directory() {
    let repo = common::TestRepo::create_with_manifest(VALID_MANIFEST);
    let server = MockServer::start().await;
    mock_pkg(&server, &repo.url, &repo.sha).await;
    let project = TempDir::new().unwrap();

    epm_adopt(&server.uri(), &project, PKG).success();

    assert!(vendor_path(&project).exists());
}

#[tokio::test(flavor = "multi_thread")]
async fn adopt_clones_repo_contents() {
    let repo = common::TestRepo::create_with_manifest(VALID_MANIFEST);
    let server = MockServer::start().await;
    mock_pkg(&server, &repo.url, &repo.sha).await;
    let project = TempDir::new().unwrap();

    epm_adopt(&server.uri(), &project, PKG).success();

    assert!(vendor_path(&project).join("eps.toml").exists());
}

#[tokio::test(flavor = "multi_thread")]
async fn adopt_checks_out_correct_commit_sha() {
    let repo = common::TestRepo::create_with_manifest(VALID_MANIFEST);
    let server = MockServer::start().await;
    mock_pkg(&server, &repo.url, &repo.sha).await;
    let project = TempDir::new().unwrap();

    epm_adopt(&server.uri(), &project, PKG).success();

    let output = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(vendor_path(&project))
        .output()
        .unwrap();
    let sha = String::from_utf8(output.stdout).unwrap().trim().to_string();
    assert_eq!(sha, repo.sha);
}

// ── .adopted.toml ─────────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn adopt_writes_adopted_toml() {
    let repo = common::TestRepo::create_with_manifest(VALID_MANIFEST);
    let server = MockServer::start().await;
    mock_pkg(&server, &repo.url, &repo.sha).await;
    let project = TempDir::new().unwrap();

    epm_adopt(&server.uri(), &project, PKG).success();

    assert!(vendor_path(&project).join(".adopted.toml").exists());
}

#[tokio::test(flavor = "multi_thread")]
async fn adopt_adopted_toml_contains_name_version_and_commit() {
    let repo = common::TestRepo::create_with_manifest(VALID_MANIFEST);
    let server = MockServer::start().await;
    mock_pkg(&server, &repo.url, &repo.sha).await;
    let project = TempDir::new().unwrap();

    epm_adopt(&server.uri(), &project, PKG).success();

    let contents = std::fs::read_to_string(vendor_path(&project).join(".adopted.toml")).unwrap();
    assert!(contents.contains(PKG), "should contain package name");
    assert!(contents.contains(VER), "should contain version");
    assert!(contents.contains(&repo.sha), "should contain commit sha");
}

// ── guard: already adopted ────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn adopt_rejects_if_vendor_dir_already_exists() {
    let server = MockServer::start().await;
    let project = TempDir::new().unwrap();

    std::fs::create_dir_all(vendor_path(&project)).unwrap();

    epm_adopt(&server.uri(), &project, PKG)
        .failure()
        .stderr(predicate::str::contains("already exists"));
}

// ── guard: eps.toml gate ──────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn adopt_rejects_package_without_eps_toml() {
    // TestRepo::create() does NOT include eps.toml
    let repo = common::TestRepo::create();
    let server = MockServer::start().await;
    mock_pkg(&server, &repo.url, &repo.sha).await;
    let project = TempDir::new().unwrap();

    epm_adopt(&server.uri(), &project, PKG)
        .failure()
        .stderr(predicate::str::contains("eps.toml"));
}

#[tokio::test(flavor = "multi_thread")]
async fn adopt_cleans_up_vendor_dir_on_eps_toml_failure() {
    let repo = common::TestRepo::create();
    let server = MockServer::start().await;
    mock_pkg(&server, &repo.url, &repo.sha).await;
    let project = TempDir::new().unwrap();

    epm_adopt(&server.uri(), &project, PKG).failure();

    assert!(!vendor_path(&project).exists(), "vendor dir should be cleaned up on failure");
}

// ── registry error cases ──────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn adopt_package_not_found_exits_nonzero() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/api/v1/packages/{PKG}")))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;
    let project = TempDir::new().unwrap();

    epm_adopt(&server.uri(), &project, PKG).failure();
}

// ── output ────────────────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn adopt_prints_adopting_message() {
    let repo = common::TestRepo::create_with_manifest(VALID_MANIFEST);
    let server = MockServer::start().await;
    mock_pkg(&server, &repo.url, &repo.sha).await;
    let project = TempDir::new().unwrap();

    epm_adopt(&server.uri(), &project, PKG)
        .success()
        .stdout(predicate::str::contains("Adopting"));
}

#[tokio::test(flavor = "multi_thread")]
async fn adopt_prints_success_message() {
    let repo = common::TestRepo::create_with_manifest(VALID_MANIFEST);
    let server = MockServer::start().await;
    mock_pkg(&server, &repo.url, &repo.sha).await;
    let project = TempDir::new().unwrap();

    epm_adopt(&server.uri(), &project, PKG)
        .success()
        .stdout(predicate::str::contains("Adopted"));
}

#[tokio::test(flavor = "multi_thread")]
async fn adopt_prints_sync_guidance() {
    let repo = common::TestRepo::create_with_manifest(VALID_MANIFEST);
    let server = MockServer::start().await;
    mock_pkg(&server, &repo.url, &repo.sha).await;
    let project = TempDir::new().unwrap();

    epm_adopt(&server.uri(), &project, PKG)
        .success()
        .stdout(predicate::str::contains("sync"));
}
