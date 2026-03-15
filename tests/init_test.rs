use assert_cmd::Command;
use predicates::prelude::*;
use tempfile::TempDir;

fn epm_init(name: &str, extra_args: &[&str], cwd: &TempDir) -> assert_cmd::assert::Assert {
    Command::cargo_bin("epm")
        .unwrap()
        .current_dir(cwd.path())
        .args(["init", name])
        .args(extra_args)
        .assert()
}

// ── files created ─────────────────────────────────────────────────────────────

#[test]
fn init_creates_directory() {
    let cwd = TempDir::new().unwrap();
    epm_init("my_pkg", &["--no-git"], &cwd).success();
    assert!(cwd.path().join("my_pkg").is_dir());
}

#[test]
fn init_creates_eps_toml() {
    let cwd = TempDir::new().unwrap();
    epm_init("my_pkg", &["--no-git"], &cwd).success();
    assert!(cwd.path().join("my_pkg/eps.toml").is_file());
}

#[test]
fn init_creates_customize_md() {
    let cwd = TempDir::new().unwrap();
    epm_init("my_pkg", &["--no-git"], &cwd).success();
    assert!(cwd.path().join("my_pkg/CUSTOMIZE.md").is_file());
}

#[test]
fn init_creates_run_sh() {
    let cwd = TempDir::new().unwrap();
    epm_init("my_pkg", &["--no-git"], &cwd).success();
    assert!(cwd.path().join("my_pkg/run.sh").is_file());
}

#[cfg(unix)]
#[test]
fn init_run_sh_is_executable() {
    use std::os::unix::fs::PermissionsExt;
    let cwd = TempDir::new().unwrap();
    epm_init("my_pkg", &["--no-git"], &cwd).success();
    let mode = std::fs::metadata(cwd.path().join("my_pkg/run.sh"))
        .unwrap()
        .permissions()
        .mode();
    // owner execute bit must be set
    assert!(mode & 0o100 != 0, "run.sh should be executable, mode={mode:o}");
}

// ── eps.toml content ──────────────────────────────────────────────────────────

#[test]
fn init_eps_toml_contains_package_name() {
    let cwd = TempDir::new().unwrap();
    epm_init("cool_harness", &["--no-git"], &cwd).success();
    let content = std::fs::read_to_string(cwd.path().join("cool_harness/eps.toml")).unwrap();
    assert!(content.contains("name = \"cool_harness\""), "eps.toml: {content}");
}

#[test]
fn init_eps_toml_sets_initial_version() {
    let cwd = TempDir::new().unwrap();
    epm_init("versioned_pkg", &["--no-git"], &cwd).success();
    let content = std::fs::read_to_string(cwd.path().join("versioned_pkg/eps.toml")).unwrap();
    assert!(content.contains("version = \"0.1.0\""), "eps.toml: {content}");
}

#[test]
fn init_eps_toml_is_valid_toml_with_correct_fields() {
    let cwd = TempDir::new().unwrap();
    epm_init("toml_test_pkg", &["--no-git"], &cwd).success();
    let content = std::fs::read_to_string(cwd.path().join("toml_test_pkg/eps.toml")).unwrap();
    let parsed: toml::Value = toml::from_str(&content).expect("eps.toml must be valid TOML");
    let pkg = &parsed["package"];
    assert_eq!(pkg["name"].as_str().unwrap(), "toml_test_pkg");
    assert_eq!(pkg["version"].as_str().unwrap(), "0.1.0");
    assert_eq!(pkg["license"].as_str().unwrap(), "MIT");
    assert!(pkg["authors"].as_array().is_some());
    assert!(pkg["platforms"].as_array().is_some());
}

// ── --description flag ────────────────────────────────────────────────────────

#[test]
fn init_description_flag_written_to_eps_toml() {
    let cwd = TempDir::new().unwrap();
    epm_init("desc_pkg", &["--no-git", "--description", "My cool harness"], &cwd).success();
    let content = std::fs::read_to_string(cwd.path().join("desc_pkg/eps.toml")).unwrap();
    assert!(content.contains("My cool harness"), "eps.toml: {content}");
}

#[test]
fn init_description_short_flag_works() {
    let cwd = TempDir::new().unwrap();
    epm_init("short_flag_pkg", &["--no-git", "-d", "Short flag desc"], &cwd).success();
    let content = std::fs::read_to_string(cwd.path().join("short_flag_pkg/eps.toml")).unwrap();
    assert!(content.contains("Short flag desc"), "eps.toml: {content}");
}

#[test]
fn init_without_description_uses_placeholder() {
    let cwd = TempDir::new().unwrap();
    epm_init("no_desc_pkg", &["--no-git"], &cwd).success();
    let content = std::fs::read_to_string(cwd.path().join("no_desc_pkg/eps.toml")).unwrap();
    assert!(content.contains("description ="), "eps.toml: {content}");
    // placeholder should be non-empty
    let parsed: toml::Value = toml::from_str(&content).unwrap();
    let desc = parsed["package"]["description"].as_str().unwrap();
    assert!(!desc.is_empty());
}

// ── run.sh content ────────────────────────────────────────────────────────────

#[test]
fn init_run_sh_contains_package_name() {
    let cwd = TempDir::new().unwrap();
    epm_init("named_harness", &["--no-git"], &cwd).success();
    let content = std::fs::read_to_string(cwd.path().join("named_harness/run.sh")).unwrap();
    assert!(content.contains("named_harness"), "run.sh: {content}");
}

#[test]
fn init_run_sh_has_shebang() {
    let cwd = TempDir::new().unwrap();
    epm_init("shebang_pkg", &["--no-git"], &cwd).success();
    let content = std::fs::read_to_string(cwd.path().join("shebang_pkg/run.sh")).unwrap();
    assert!(content.starts_with("#!/usr/bin/env bash"), "run.sh: {content}");
}

// ── CUSTOMIZE.md content ──────────────────────────────────────────────────────

#[test]
fn init_customize_md_contains_package_name() {
    let cwd = TempDir::new().unwrap();
    epm_init("named_harness2", &["--no-git"], &cwd).success();
    let content = std::fs::read_to_string(cwd.path().join("named_harness2/CUSTOMIZE.md")).unwrap();
    assert!(content.contains("named_harness2"), "CUSTOMIZE.md: {content}");
}

#[test]
fn init_customize_md_contains_ports_section() {
    let cwd = TempDir::new().unwrap();
    epm_init("ports_pkg", &["--no-git"], &cwd).success();
    let content = std::fs::read_to_string(cwd.path().join("ports_pkg/CUSTOMIZE.md")).unwrap();
    assert!(content.contains("Ports"), "expected 'Ports' section in CUSTOMIZE.md: {content}");
}

// ── git init ──────────────────────────────────────────────────────────────────

#[test]
fn init_runs_git_init_by_default() {
    let cwd = TempDir::new().unwrap();
    epm_init("git_pkg", &[], &cwd).success();
    assert!(cwd.path().join("git_pkg/.git").is_dir());
}

#[test]
fn init_no_git_skips_git_init() {
    let cwd = TempDir::new().unwrap();
    epm_init("no_git_pkg", &["--no-git"], &cwd).success();
    assert!(!cwd.path().join("no_git_pkg/.git").exists());
}

// ── stdout ────────────────────────────────────────────────────────────────────

#[test]
fn init_prints_package_name() {
    let cwd = TempDir::new().unwrap();
    epm_init("my_harness", &["--no-git"], &cwd)
        .success()
        .stdout(predicate::str::contains("my_harness"));
}

#[test]
fn init_prints_epm_publish_hint() {
    let cwd = TempDir::new().unwrap();
    epm_init("hint_pkg", &["--no-git"], &cwd)
        .success()
        .stdout(predicate::str::contains("epm publish"));
}

#[test]
fn init_prints_run_sh_in_file_list() {
    let cwd = TempDir::new().unwrap();
    epm_init("list_pkg", &["--no-git"], &cwd)
        .success()
        .stdout(predicate::str::contains("run.sh"));
}

// ── name validation ───────────────────────────────────────────────────────────

#[test]
fn init_rejects_uppercase_name() {
    let cwd = TempDir::new().unwrap();
    epm_init("MyPkg", &["--no-git"], &cwd)
        .failure()
        .stderr(predicate::str::contains("MyPkg"));
}

#[test]
fn init_rejects_name_with_hyphen() {
    let cwd = TempDir::new().unwrap();
    epm_init("my-pkg", &["--no-git"], &cwd)
        .failure()
        .stderr(predicate::str::contains('-'));
}

#[test]
fn init_rejects_name_starting_with_digit() {
    let cwd = TempDir::new().unwrap();
    epm_init("1pkg", &["--no-git"], &cwd)
        .failure()
        .stderr(predicate::str::contains("lowercase letter"));
}

#[test]
fn init_rejects_single_character_name() {
    let cwd = TempDir::new().unwrap();
    epm_init("a", &["--no-git"], &cwd)
        .failure()
        .stderr(predicate::str::contains("2–64"));
}

#[test]
fn init_rejects_name_with_spaces() {
    let cwd = TempDir::new().unwrap();
    epm_init("my pkg", &["--no-git"], &cwd).failure();
}

#[test]
fn init_validation_fails_before_creating_directory() {
    let cwd = TempDir::new().unwrap();
    epm_init("Bad-Name", &["--no-git"], &cwd).failure();
    // directory must not have been created
    assert!(!cwd.path().join("Bad-Name").exists());
}

// ── already-exists error ──────────────────────────────────────────────────────

#[test]
fn init_fails_if_directory_already_exists() {
    let cwd = TempDir::new().unwrap();
    std::fs::create_dir(cwd.path().join("existing_pkg")).unwrap();
    epm_init("existing_pkg", &["--no-git"], &cwd)
        .failure()
        .stderr(predicate::str::contains("existing_pkg"));
}
