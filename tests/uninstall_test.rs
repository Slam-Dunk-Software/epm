use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

fn epm_uninstall(home: &TempDir, spec: &str) -> assert_cmd::assert::Assert {
    Command::cargo_bin("epm")
        .unwrap()
        .args(["uninstall", spec])
        .env("HOME", home.path())
        .assert()
}

fn install_path(home: &TempDir, name: &str, version: &str) -> std::path::PathBuf {
    home.path().join(".epm").join("packages").join(name).join(version)
}

#[test]
fn uninstall_pinned_version_removes_directory() {
    let home = TempDir::new().unwrap();
    let path = install_path(&home, "tech_talker", "0.1.0");
    std::fs::create_dir_all(&path).unwrap();

    epm_uninstall(&home, "tech_talker@0.1.0").success();

    assert!(!path.exists(), "install directory should be removed");
}

#[test]
fn uninstall_pinned_version_prints_success_message() {
    let home = TempDir::new().unwrap();
    std::fs::create_dir_all(install_path(&home, "tech_talker", "0.1.0")).unwrap();

    epm_uninstall(&home, "tech_talker@0.1.0")
        .success()
        .stdout(predicate::str::contains("Uninstalled tech_talker@0.1.0"));
}

#[test]
fn uninstall_not_installed_exits_nonzero() {
    let home = TempDir::new().unwrap();
    epm_uninstall(&home, "tech_talker@0.1.0").failure();
}

#[test]
fn uninstall_name_only_removes_when_single_version_installed() {
    let home = TempDir::new().unwrap();
    let path = install_path(&home, "tech_talker", "0.1.0");
    std::fs::create_dir_all(&path).unwrap();

    epm_uninstall(&home, "tech_talker").success();

    assert!(!path.exists(), "install directory should be removed");
}

#[test]
fn uninstall_last_version_removes_package_directory() {
    let home = TempDir::new().unwrap();
    std::fs::create_dir_all(install_path(&home, "tech_talker", "0.1.0")).unwrap();

    epm_uninstall(&home, "tech_talker@0.1.0").success();

    let pkg_dir = home.path().join(".epm").join("packages").join("tech_talker");
    assert!(!pkg_dir.exists(), "empty package directory should be removed");
}

#[test]
fn uninstall_one_of_two_versions_leaves_package_directory() {
    let home = TempDir::new().unwrap();
    std::fs::create_dir_all(install_path(&home, "tech_talker", "0.1.0")).unwrap();
    std::fs::create_dir_all(install_path(&home, "tech_talker", "0.2.0")).unwrap();

    epm_uninstall(&home, "tech_talker@0.1.0").success();

    let pkg_dir = home.path().join(".epm").join("packages").join("tech_talker");
    assert!(pkg_dir.exists(), "package directory should remain when versions still installed");
}

#[test]
fn uninstall_name_only_errors_when_multiple_versions_installed() {
    let home = TempDir::new().unwrap();
    std::fs::create_dir_all(install_path(&home, "tech_talker", "0.1.0")).unwrap();
    std::fs::create_dir_all(install_path(&home, "tech_talker", "0.2.0")).unwrap();

    epm_uninstall(&home, "tech_talker")
        .failure()
        .stderr(predicate::str::contains("multiple versions installed"));
}
