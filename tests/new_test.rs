mod common;

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const PKG: &str = "tech_talker";
const VER: &str = "0.1.0";

// ── helpers ───────────────────────────────────────────────────────────────────

async fn mock_pkg_with_version(server: &MockServer, git_url: &str, sha: &str) {
    let ver = common::version_json(1, 1, VER, git_url, sha, false);
    let pkg = common::package_with_versions_json(1, PKG, vec![ver]);
    Mock::given(method("GET"))
        .and(path(format!("/api/v1/packages/{PKG}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(pkg))
        .mount(server)
        .await;
}

async fn mock_track_install(server: &MockServer) {
    Mock::given(method("POST"))
        .and(path(format!("/api/v1/packages/{PKG}/installs")))
        .respond_with(ResponseTemplate::new(201))
        .expect(0..)
        .mount(server)
        .await;
}

/// Needed because suggest_typo fires GET /api/v1/packages after a 404.
async fn mock_list_packages_empty(server: &MockServer) {
    Mock::given(method("GET"))
        .and(path("/api/v1/packages"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!([])))
        .expect(0..)
        .mount(server)
        .await;
}

fn epm_new(registry: &str, home: &TempDir, args: &[&str]) -> assert_cmd::assert::Assert {
    Command::cargo_bin("epm")
        .unwrap()
        .args(["new"].iter().chain(args.iter()).copied().collect::<Vec<_>>())
        .env("EPM_REGISTRY", registry)
        .env("HOME", home.path())
        .env("EPM_HOME", home.path())
        .env("GIT_AUTHOR_NAME", "EPM Test")
        .env("GIT_AUTHOR_EMAIL", "test@epm.test")
        .env("GIT_COMMITTER_NAME", "EPM Test")
        .env("GIT_COMMITTER_EMAIL", "test@epm.test")
        .current_dir(home.path())
        .assert()
}

// ── directory ─────────────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn new_creates_directory_at_package_name() {
    let repo = common::TestRepo::create();
    let server = MockServer::start().await;
    mock_pkg_with_version(&server, &repo.url, &repo.sha).await;
    mock_track_install(&server).await;
    let home = TempDir::new().unwrap();

    epm_new(&server.uri(), &home, &[PKG]).success();

    assert!(home.path().join(PKG).exists());
}

#[tokio::test(flavor = "multi_thread")]
async fn new_creates_directory_at_custom_name() {
    let repo = common::TestRepo::create();
    let server = MockServer::start().await;
    mock_pkg_with_version(&server, &repo.url, &repo.sha).await;
    mock_track_install(&server).await;
    let home = TempDir::new().unwrap();

    epm_new(&server.uri(), &home, &[PKG, "my_project"]).success();

    assert!(home.path().join("my_project").exists());
    assert!(!home.path().join(PKG).exists());
}

// ── content ───────────────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn new_clones_repo_contents() {
    let repo = common::TestRepo::create();
    let server = MockServer::start().await;
    mock_pkg_with_version(&server, &repo.url, &repo.sha).await;
    mock_track_install(&server).await;
    let home = TempDir::new().unwrap();

    epm_new(&server.uri(), &home, &[PKG]).success();

    assert!(home.path().join(PKG).join("CUSTOMIZE.md").exists());
}

#[tokio::test(flavor = "multi_thread")]
async fn new_strips_upstream_git_history() {
    let repo = common::TestRepo::create();
    let server = MockServer::start().await;
    mock_pkg_with_version(&server, &repo.url, &repo.sha).await;
    mock_track_install(&server).await;
    let home = TempDir::new().unwrap();

    epm_new(&server.uri(), &home, &[PKG]).success();

    let dest = home.path().join(PKG);
    let log = std::process::Command::new("git")
        .args(["log", "--oneline"])
        .current_dir(&dest)
        .output()
        .expect("git log failed");

    let count = String::from_utf8(log.stdout).unwrap().lines().count();
    assert_eq!(count, 1, "expected exactly 1 commit (fresh init), got {count}");
}

#[tokio::test(flavor = "multi_thread")]
async fn new_initial_commit_message_contains_package_name() {
    let repo = common::TestRepo::create();
    let server = MockServer::start().await;
    mock_pkg_with_version(&server, &repo.url, &repo.sha).await;
    mock_track_install(&server).await;
    let home = TempDir::new().unwrap();

    epm_new(&server.uri(), &home, &[PKG]).success();

    let dest = home.path().join(PKG);
    let out = std::process::Command::new("git")
        .args(["log", "--format=%s", "-1"])
        .current_dir(&dest)
        .output()
        .expect("git log failed");

    let msg = String::from_utf8(out.stdout).unwrap();
    assert!(msg.contains(PKG), "commit message should contain '{PKG}', got: {msg}");
}

// ── stdout ────────────────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn new_prints_ready_message() {
    let repo = common::TestRepo::create();
    let server = MockServer::start().await;
    mock_pkg_with_version(&server, &repo.url, &repo.sha).await;
    mock_track_install(&server).await;
    let home = TempDir::new().unwrap();

    epm_new(&server.uri(), &home, &[PKG])
        .success()
        .stdout(predicate::str::contains("Ready"));
}

// ── errors ────────────────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn new_destination_already_exists_exits_nonzero() {
    let repo = common::TestRepo::create();
    let server = MockServer::start().await;
    mock_pkg_with_version(&server, &repo.url, &repo.sha).await;
    let home = TempDir::new().unwrap();

    std::fs::create_dir_all(home.path().join(PKG)).unwrap();

    epm_new(&server.uri(), &home, &[PKG]).failure();
}

#[tokio::test(flavor = "multi_thread")]
async fn new_package_not_found_exits_nonzero() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/api/v1/packages/{PKG}")))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;
    mock_list_packages_empty(&server).await;
    let home = TempDir::new().unwrap();

    epm_new(&server.uri(), &home, &[PKG]).failure();
}

#[tokio::test(flavor = "multi_thread")]
async fn new_package_not_found_shows_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/api/v1/packages/{PKG}")))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;
    mock_list_packages_empty(&server).await;
    let home = TempDir::new().unwrap();

    epm_new(&server.uri(), &home, &[PKG])
        .failure()
        .stderr(predicate::str::contains("not found").or(predicate::str::contains(PKG)));
}

// ── version spec ──────────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn new_with_version_spec() {
    let repo = common::TestRepo::create();
    let server = MockServer::start().await;
    mock_pkg_with_version(&server, &repo.url, &repo.sha).await;
    mock_track_install(&server).await;
    let home = TempDir::new().unwrap();

    epm_new(&server.uri(), &home, &[&format!("{PKG}@{VER}")]).success();

    assert!(home.path().join(PKG).exists());
}
