mod common;

use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::json;
use tempfile::TempDir;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const PKG: &str = "tech_talker";
const VER: &str = "0.1.0";

/// Mount a mock that returns a package with one non-yanked version.
async fn mock_pkg_with_version(server: &MockServer, git_url: &str, sha: &str) {
    let ver = common::version_json(1, 1, VER, git_url, sha, false);
    let pkg = common::package_with_versions_json(1, PKG, vec![ver]);
    Mock::given(method("GET"))
        .and(path(format!("/api/v1/packages/{PKG}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(pkg))
        .mount(server)
        .await;
}


fn install_path(home: &TempDir) -> std::path::PathBuf {
    home.path().join(".epm").join("packages").join(PKG).join(VER)
}

fn epm_install(registry: &str, home: &TempDir, spec: &str) -> assert_cmd::assert::Assert {
    Command::cargo_bin("epm")
        .unwrap()
        .args(["--registry", registry, "install", spec])
        .env("HOME", home.path())
        .assert()
}

// ── directory / content tests ─────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn install_creates_directory_at_expected_path() {
    let repo = common::TestRepo::create();
    let server = MockServer::start().await;
    mock_pkg_with_version(&server, &repo.url, &repo.sha).await;
    let home = TempDir::new().unwrap();

    epm_install(&server.uri(), &home, PKG).success();

    assert!(install_path(&home).exists(), "install directory should exist");
}

#[tokio::test(flavor = "multi_thread")]
async fn install_clones_git_repo_contents() {
    let repo = common::TestRepo::create();
    let server = MockServer::start().await;
    mock_pkg_with_version(&server, &repo.url, &repo.sha).await;
    let home = TempDir::new().unwrap();

    epm_install(&server.uri(), &home, PKG).success();

    assert!(
        install_path(&home).join("CUSTOMIZE.md").exists(),
        "CUSTOMIZE.md should have been cloned"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn install_checks_out_correct_commit_sha() {
    let repo = common::TestRepo::create();
    let server = MockServer::start().await;
    mock_pkg_with_version(&server, &repo.url, &repo.sha).await;
    let home = TempDir::new().unwrap();

    epm_install(&server.uri(), &home, PKG).success();

    let output = std::process::Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(install_path(&home))
        .output()
        .expect("failed to run git rev-parse HEAD in install dir");
    let actual_sha = String::from_utf8(output.stdout).unwrap().trim().to_string();

    assert_eq!(actual_sha, repo.sha, "installed commit SHA should match");
}

// ── stdout messages ───────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn install_prints_installing_message() {
    let repo = common::TestRepo::create();
    let server = MockServer::start().await;
    mock_pkg_with_version(&server, &repo.url, &repo.sha).await;
    let home = TempDir::new().unwrap();

    epm_install(&server.uri(), &home, PKG)
        .success()
        .stdout(predicate::str::contains("Installing"));
}

#[tokio::test(flavor = "multi_thread")]
async fn install_prints_success_message_with_path() {
    let repo = common::TestRepo::create();
    let server = MockServer::start().await;
    mock_pkg_with_version(&server, &repo.url, &repo.sha).await;
    let home = TempDir::new().unwrap();

    epm_install(&server.uri(), &home, PKG)
        .success()
        .stdout(predicate::str::contains("Installed"))
        .stdout(predicate::str::contains(".epm"));
}

// ── pinned version (@syntax) ──────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn install_pinned_version_via_at_syntax() {
    let repo = common::TestRepo::create();
    let server = MockServer::start().await;
    // Pinned installs now resolve through get_package, not get_version.
    mock_pkg_with_version(&server, &repo.url, &repo.sha).await;
    let home = TempDir::new().unwrap();

    epm_install(&server.uri(), &home, &format!("{PKG}@{VER}"))
        .success();

    assert!(install_path(&home).exists());
}

// ── already installed ─────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn install_already_installed_skips_clone() {
    let repo = common::TestRepo::create();
    let server = MockServer::start().await;
    mock_pkg_with_version(&server, &repo.url, &repo.sha).await;
    let home = TempDir::new().unwrap();

    // Pre-create the install directory — simulates already-installed
    std::fs::create_dir_all(install_path(&home)).unwrap();

    epm_install(&server.uri(), &home, PKG).success();

    // CUSTOMIZE.md should NOT exist since we skipped the clone
    assert!(
        !install_path(&home).join("CUSTOMIZE.md").exists(),
        "clone should have been skipped"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn install_already_installed_prints_skip_message() {
    let repo = common::TestRepo::create();
    let server = MockServer::start().await;
    mock_pkg_with_version(&server, &repo.url, &repo.sha).await;
    let home = TempDir::new().unwrap();

    std::fs::create_dir_all(install_path(&home)).unwrap();

    epm_install(&server.uri(), &home, PKG)
        .success()
        .stdout(predicate::str::contains("already installed"));
}

// ── error cases ───────────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn install_package_not_found_exits_nonzero() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/api/v1/packages/{PKG}")))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;
    let home = TempDir::new().unwrap();

    epm_install(&server.uri(), &home, PKG).failure();
}

#[tokio::test(flavor = "multi_thread")]
async fn install_package_not_found_shows_error_in_stderr() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(format!("/api/v1/packages/{PKG}")))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;
    let home = TempDir::new().unwrap();

    epm_install(&server.uri(), &home, PKG)
        .failure()
        .stderr(predicate::str::contains("not found").or(predicate::str::contains(PKG)));
}

#[tokio::test(flavor = "multi_thread")]
async fn install_no_eligible_versions_exits_nonzero() {
    let server = MockServer::start().await;
    // All versions are yanked
    let yanked_ver = json!({
        "id": 1,
        "package_id": 1,
        "version": VER,
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

    epm_install(&server.uri(), &home, PKG).failure();
}

// ── system dependency checks ──────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn install_succeeds_with_empty_system_deps() {
    let repo = common::TestRepo::create();
    let server = MockServer::start().await;
    // version_json already includes system_deps: {} — this confirms empty deps don't block
    mock_pkg_with_version(&server, &repo.url, &repo.sha).await;
    let home = TempDir::new().unwrap();

    epm_install(&server.uri(), &home, PKG).success();
}

#[tokio::test(flavor = "multi_thread")]
async fn install_fails_with_missing_system_dep() {
    let repo = common::TestRepo::create();
    let server = MockServer::start().await;
    // Serve a version that requires a nonexistent brew package
    let ver = json!({
        "id": 1,
        "package_id": 1,
        "version": VER,
        "git_url": repo.url,
        "commit_sha": repo.sha,
        "manifest_hash": "abc123",
        "yanked": false,
        "published_at": "2025-01-01T00:00:00",
        "system_deps": {"brew": ["epm_nonexistent_xyz_456"]}
    });
    let pkg = common::package_with_versions_json(1, PKG, vec![ver]);
    Mock::given(method("GET"))
        .and(path(format!("/api/v1/packages/{PKG}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(pkg))
        .mount(&server)
        .await;
    let home = TempDir::new().unwrap();

    epm_install(&server.uri(), &home, PKG)
        .failure()
        .stderr(predicate::str::contains("missing system dependencies"));
}

// ── platform checks ───────────────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread")]
async fn install_rejects_unsupported_platform() {
    let server = MockServer::start().await;
    // Serve a package whose platform list explicitly excludes the current machine.
    let other_platform = if cfg!(target_arch = "aarch64") {
        "x86_64-apple-darwin"
    } else {
        "aarch64-apple-darwin"
    };
    let ver = common::version_json(1, 1, VER, "https://github.com/test/test", "abc123", false);
    let pkg = json!({
        "id": 1,
        "name": PKG,
        "description": "test",
        "authors": ["test"],
        "license": "MIT",
        "homepage": null,
        "repository": "https://github.com/test/test",
        "platforms": [other_platform],
        "created_at": "2025-01-01T00:00:00",
        "updated_at": "2025-01-01T00:00:00",
        "versions": [ver]
    });
    Mock::given(method("GET"))
        .and(path(format!("/api/v1/packages/{PKG}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(pkg))
        .mount(&server)
        .await;
    let home = TempDir::new().unwrap();

    epm_install(&server.uri(), &home, PKG)
        .failure()
        .stderr(predicate::str::contains("does not support your platform"));
}
