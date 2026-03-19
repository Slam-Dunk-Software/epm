mod common;

use assert_cmd::Command;
use predicates::prelude::*;

#[test]
fn open_prints_url() {
    let mut cmd = Command::cargo_bin("epm").unwrap();
    cmd.env("EPM_REGISTRY", "https://epm.dev")
        .args(["open", "todo"]);
    cmd.assert()
        .success()
        .stdout(predicate::str::contains("https://epm.dev/packages/todo"));
}

#[test]
fn open_uses_registry_env_var() {
    let mut cmd = Command::cargo_bin("epm").unwrap();
    cmd.env("EPM_REGISTRY", "http://localhost:9999")
        .args(["open", "crm"]);
    // May fail to open browser in CI, but should at least attempt the right URL.
    // We only check stdout contains the constructed URL on success, or stderr on failure.
    let output = cmd.output().unwrap();
    let all = String::from_utf8_lossy(&output.stdout).to_string()
        + &String::from_utf8_lossy(&output.stderr);
    assert!(
        all.contains("http://localhost:9999/packages/crm"),
        "expected URL in output, got: {all}"
    );
}

#[test]
fn open_requires_name_argument() {
    let mut cmd = Command::cargo_bin("epm").unwrap();
    cmd.arg("open");
    cmd.assert().failure();
}
