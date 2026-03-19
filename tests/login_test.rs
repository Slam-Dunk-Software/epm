mod common;

use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

fn epm_login_with_token(home: &TempDir, token: &str, registry: &str) -> assert_cmd::assert::Assert {
    Command::cargo_bin("epm")
        .unwrap()
        .args(["login", "--token", token])
        .env("HOME", home.path())
        .env("EPM_REGISTRY", registry)
        .assert()
}

#[test]
fn login_with_token_succeeds() {
    let home = TempDir::new().unwrap();
    epm_login_with_token(&home, "mytoken123", "https://epm.dev")
        .success()
        .stdout(predicate::str::contains("Token saved"));
}

#[test]
fn login_with_token_writes_credentials_file() {
    let home = TempDir::new().unwrap();
    epm_login_with_token(&home, "stored_token_xyz", "https://epm.dev").success();

    let creds_path = home.path().join(".epm").join("credentials");
    assert!(creds_path.exists(), "credentials file should be created");
    let contents = std::fs::read_to_string(&creds_path).unwrap();
    assert!(contents.contains("stored_token_xyz"), "credentials should contain the token");
}

#[test]
fn login_with_token_credentials_file_has_correct_permissions() {
    use std::os::unix::fs::PermissionsExt;
    let home = TempDir::new().unwrap();
    epm_login_with_token(&home, "secure_token", "https://epm.dev").success();

    let creds_path = home.path().join(".epm").join("credentials");
    let meta = std::fs::metadata(&creds_path).unwrap();
    let mode = meta.permissions().mode();
    assert_eq!(mode & 0o777, 0o600, "credentials should be mode 0600");
}
