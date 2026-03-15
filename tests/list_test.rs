use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

fn epm_list(home: &TempDir) -> assert_cmd::assert::Assert {
    Command::cargo_bin("epm")
        .unwrap()
        .args(["list"])
        .env("HOME", home.path())
        .assert()
}

#[test]
fn list_no_packages_installed_shows_message() {
    let home = TempDir::new().unwrap();
    epm_list(&home)
        .success()
        .stdout(predicate::str::contains("No packages installed."));
}

#[test]
fn list_shows_installed_package() {
    let home = TempDir::new().unwrap();
    std::fs::create_dir_all(home.path().join(".epm/packages/tech_talker/0.1.0")).unwrap();

    epm_list(&home)
        .success()
        .stdout(predicate::str::contains("tech_talker@0.1.0"));
}

#[test]
fn list_shows_multiple_packages() {
    let home = TempDir::new().unwrap();
    std::fs::create_dir_all(home.path().join(".epm/packages/tech_talker/0.1.0")).unwrap();
    std::fs::create_dir_all(home.path().join(".epm/packages/pi/0.2.0")).unwrap();

    let output = epm_list(&home).success().get_output().stdout.clone();
    let stdout = String::from_utf8(output).unwrap();
    assert!(stdout.contains("tech_talker@0.1.0"), "expected tech_talker@0.1.0 in: {stdout}");
    assert!(stdout.contains("pi@0.2.0"), "expected pi@0.2.0 in: {stdout}");
}

#[test]
fn list_shows_multiple_versions_of_same_package() {
    let home = TempDir::new().unwrap();
    std::fs::create_dir_all(home.path().join(".epm/packages/tech_talker/0.1.0")).unwrap();
    std::fs::create_dir_all(home.path().join(".epm/packages/tech_talker/0.2.0")).unwrap();

    let output = epm_list(&home).success().get_output().stdout.clone();
    let stdout = String::from_utf8(output).unwrap();
    assert!(stdout.contains("tech_talker@0.1.0"), "expected 0.1.0 in: {stdout}");
    assert!(stdout.contains("tech_talker@0.2.0"), "expected 0.2.0 in: {stdout}");
}
