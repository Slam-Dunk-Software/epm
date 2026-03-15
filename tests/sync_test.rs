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

async fn mock_pkg_on(server: &MockServer, git_url: &str, sha: &str, version: &str) {
    let ver = common::version_json(1, 1, version, git_url, sha, false);
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

fn epm_adopt(registry: &str, project: &TempDir) -> assert_cmd::assert::Assert {
    Command::cargo_bin("epm")
        .unwrap()
        .current_dir(project.path())
        .args(["adopt", PKG])
        .env("EPM_REGISTRY", registry)
        .assert()
}

fn epm_sync(registry: &str, project: &TempDir) -> assert_cmd::assert::Assert {
    Command::cargo_bin("epm")
        .unwrap()
        .current_dir(project.path())
        .args(["sync", PKG])
        .env("EPM_REGISTRY", registry)
        .assert()
}

fn epm_sync_wipe(registry: &str, project: &TempDir) -> assert_cmd::assert::Assert {
    Command::cargo_bin("epm")
        .unwrap()
        .current_dir(project.path())
        .args(["sync", PKG, "--wipe"])
        .env("EPM_REGISTRY", registry)
        .assert()
}

// ── up to date ────────────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn sync_reports_up_to_date_when_commit_matches() {
    let repo = common::TestRepo::create_with_manifest(VALID_MANIFEST);
    let project = TempDir::new().unwrap();

    // Adopt
    let adopt_server = MockServer::start().await;
    mock_pkg_on(&adopt_server, &repo.url, &repo.sha, VER).await;
    epm_adopt(&adopt_server.uri(), &project).success();

    // Sync against same sha
    let sync_server = MockServer::start().await;
    mock_pkg_on(&sync_server, &repo.url, &repo.sha, VER).await;

    epm_sync(&sync_server.uri(), &project)
        .success()
        .stdout(predicate::str::contains("up to date").or(predicate::str::contains("up-to-date")));
}

// ── upstream has moved ────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn sync_reports_upstream_changes_when_commit_differs() {
    let repo = common::TestRepo::create_with_manifest(VALID_MANIFEST);
    let project = TempDir::new().unwrap();

    let adopt_server = MockServer::start().await;
    mock_pkg_on(&adopt_server, &repo.url, &repo.sha, VER).await;
    epm_adopt(&adopt_server.uri(), &project).success();

    // Sync against a newer commit
    let sync_server = MockServer::start().await;
    let new_sha = "deadbeefdeadbeefdeadbeefdeadbeefdeadbeef";
    mock_pkg_on(&sync_server, &repo.url, new_sha, "0.2.0").await;

    epm_sync(&sync_server.uri(), &project)
        .success()
        .stdout(predicate::str::contains("upstream"));
}

#[tokio::test(flavor = "multi_thread")]
async fn sync_shows_wipe_guidance_when_upstream_differs() {
    let repo = common::TestRepo::create_with_manifest(VALID_MANIFEST);
    let project = TempDir::new().unwrap();

    let adopt_server = MockServer::start().await;
    mock_pkg_on(&adopt_server, &repo.url, &repo.sha, VER).await;
    epm_adopt(&adopt_server.uri(), &project).success();

    let sync_server = MockServer::start().await;
    mock_pkg_on(&sync_server, &repo.url, "deadbeefdeadbeefdeadbeefdeadbeefdeadbeef", "0.2.0").await;

    epm_sync(&sync_server.uri(), &project)
        .success()
        .stdout(predicate::str::contains("--wipe"));
}

// ── --wipe ────────────────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn sync_wipe_removes_local_vibe_work() {
    let repo = common::TestRepo::create_with_manifest(VALID_MANIFEST);
    let project = TempDir::new().unwrap();

    let adopt_server = MockServer::start().await;
    mock_pkg_on(&adopt_server, &repo.url, &repo.sha, VER).await;
    epm_adopt(&adopt_server.uri(), &project).success();

    // Plant a sentinel file to confirm wipe removes it
    std::fs::write(vendor_path(&project).join("MY_VIBE.md"), "my custom stuff").unwrap();

    let wipe_server = MockServer::start().await;
    mock_pkg_on(&wipe_server, &repo.url, &repo.sha, VER).await;
    epm_sync_wipe(&wipe_server.uri(), &project).success();

    assert!(
        !vendor_path(&project).join("MY_VIBE.md").exists(),
        "sentinel file should be wiped"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn sync_wipe_re_clones_fresh_copy() {
    let repo = common::TestRepo::create_with_manifest(VALID_MANIFEST);
    let project = TempDir::new().unwrap();

    let adopt_server = MockServer::start().await;
    mock_pkg_on(&adopt_server, &repo.url, &repo.sha, VER).await;
    epm_adopt(&adopt_server.uri(), &project).success();

    let wipe_server = MockServer::start().await;
    mock_pkg_on(&wipe_server, &repo.url, &repo.sha, VER).await;
    epm_sync_wipe(&wipe_server.uri(), &project).success();

    assert!(vendor_path(&project).join("eps.toml").exists());
}

#[tokio::test(flavor = "multi_thread")]
async fn sync_wipe_updates_adopted_toml() {
    let repo = common::TestRepo::create_with_manifest(VALID_MANIFEST);
    let project = TempDir::new().unwrap();

    let adopt_server = MockServer::start().await;
    mock_pkg_on(&adopt_server, &repo.url, &repo.sha, VER).await;
    epm_adopt(&adopt_server.uri(), &project).success();

    let new_sha = &repo.sha; // same sha, different version string
    let wipe_server = MockServer::start().await;
    mock_pkg_on(&wipe_server, &repo.url, new_sha, "0.2.0").await;
    epm_sync_wipe(&wipe_server.uri(), &project).success();

    let contents =
        std::fs::read_to_string(vendor_path(&project).join(".adopted.toml")).unwrap();
    assert!(contents.contains("0.2.0"), ".adopted.toml should reflect new version");
}

// ── error cases ───────────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn sync_fails_if_not_adopted() {
    let server = MockServer::start().await;
    let project = TempDir::new().unwrap();

    Command::cargo_bin("epm")
        .unwrap()
        .current_dir(project.path())
        .args(["sync", PKG])
        .env("EPM_REGISTRY", &server.uri())
        .assert()
        .failure()
        .stderr(
            predicate::str::contains("not adopted")
                .or(predicate::str::contains("vendor")),
        );
}

#[tokio::test(flavor = "multi_thread")]
async fn sync_fails_if_adopted_toml_missing() {
    let server = MockServer::start().await;
    let project = TempDir::new().unwrap();

    // vendor dir exists but no .adopted.toml
    std::fs::create_dir_all(vendor_path(&project)).unwrap();

    Command::cargo_bin("epm")
        .unwrap()
        .current_dir(project.path())
        .args(["sync", PKG])
        .env("EPM_REGISTRY", &server.uri())
        .assert()
        .failure()
        .stderr(
            predicate::str::contains(".adopted.toml")
                .or(predicate::str::contains("not adopted")),
        );
}
