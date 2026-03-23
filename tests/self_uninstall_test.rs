use assert_cmd::Command;
use predicates::prelude::*;
use std::fs;
use tempfile::TempDir;

fn epm(args: &[&str], home: &TempDir) -> assert_cmd::assert::Assert {
    Command::cargo_bin("epm")
        .unwrap()
        .args(args)
        .env("HOME", home.path())
        .assert()
}

fn epm_with_path(args: &[&str], home: &TempDir, path: &str) -> assert_cmd::assert::Assert {
    Command::cargo_bin("epm")
        .unwrap()
        .args(args)
        .env("HOME", home.path())
        .env("PATH", path)
        .assert()
}

fn setup_fake_epc_binary(home: &TempDir) -> std::path::PathBuf {
    let bin_dir = home.path().join("bin");
    fs::create_dir_all(&bin_dir).unwrap();
    let epc_path = bin_dir.join("epc");
    fs::write(&epc_path, "#!/bin/sh\necho 'epc 0.1.0'\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&epc_path, fs::Permissions::from_mode(0o755)).unwrap();
    }
    bin_dir
}

fn write_installed_toml(home: &TempDir, content: &str) {
    let dir = home.path().join(".epm");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("installed.toml"), content).unwrap();
}

// ── self-uninstall ────────────────────────────────────────────────────────────

#[test]
fn self_uninstall_removes_tracked_skill_files() {
    let home = TempDir::new().unwrap();

    let commands_dir = home.path().join(".claude").join("commands");
    fs::create_dir_all(&commands_dir).unwrap();
    fs::write(commands_dir.join("semver-bump.md"), "# semver-bump").unwrap();
    fs::write(commands_dir.join("epc-release.md"), "# epc-release").unwrap();

    let semver_path = commands_dir.join("semver-bump.md").to_string_lossy().to_string();
    let epcrel_path = commands_dir.join("epc-release.md").to_string_lossy().to_string();

    write_installed_toml(&home, &format!(r#"
[[skills]]
name  = "eps_skills"
files = ["{semver_path}", "{epcrel_path}"]
"#));

    epm(&["self-uninstall", "--yes", "--keep-binary"], &home).success();

    assert!(!commands_dir.join("semver-bump.md").exists(), "semver-bump.md should be removed");
    assert!(!commands_dir.join("epc-release.md").exists(), "epc-release.md should be removed");
}

#[test]
fn self_uninstall_leaves_untracked_skill_files_intact() {
    let home = TempDir::new().unwrap();

    let commands_dir = home.path().join(".claude").join("commands");
    fs::create_dir_all(&commands_dir).unwrap();
    fs::write(commands_dir.join("semver-bump.md"), "# semver-bump").unwrap();
    fs::write(commands_dir.join("my-custom-skill.md"), "# custom").unwrap();

    let semver_path = commands_dir.join("semver-bump.md").to_string_lossy().to_string();

    write_installed_toml(&home, &format!(r#"
[[skills]]
name  = "eps_skills"
files = ["{semver_path}"]
"#));

    epm(&["self-uninstall", "--yes", "--keep-binary"], &home).success();

    assert!(commands_dir.join("my-custom-skill.md").exists(), "custom skill should remain");
}

#[test]
fn self_uninstall_removes_epm_dir() {
    let home = TempDir::new().unwrap();
    write_installed_toml(&home, "");

    epm(&["self-uninstall", "--yes", "--keep-binary"], &home).success();

    assert!(!home.path().join(".epm").exists(), "~/.epm should be removed");
}

#[test]
fn self_uninstall_with_no_manifest_exits_cleanly() {
    let home = TempDir::new().unwrap();
    // No ~/.epm/ at all
    epm(&["self-uninstall", "--yes", "--keep-binary"], &home).success();
}

#[test]
fn self_uninstall_prints_what_was_removed() {
    let home = TempDir::new().unwrap();

    let commands_dir = home.path().join(".claude").join("commands");
    fs::create_dir_all(&commands_dir).unwrap();
    fs::write(commands_dir.join("semver-bump.md"), "# semver-bump").unwrap();
    let semver_path = commands_dir.join("semver-bump.md").to_string_lossy().to_string();

    write_installed_toml(&home, &format!(r#"
[[skills]]
name  = "eps_skills"
files = ["{semver_path}"]
"#));

    let out = epm(&["self-uninstall", "--yes", "--keep-binary"], &home)
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(out).unwrap();

    assert!(stdout.contains("eps_skills"), "should mention removed skills");
}

// ── epc / runtime cleanup ─────────────────────────────────────────────────────

#[test]
fn self_uninstall_succeeds_with_epc_binary_present() {
    // epc is no longer managed by epm — a stray epc binary on PATH should not
    // cause self-uninstall to fail (it is simply ignored).
    let home = TempDir::new().unwrap();
    let bin_dir = setup_fake_epc_binary(&home);
    write_installed_toml(&home, "");

    let path = format!("{}:/usr/local/bin:/usr/bin:/bin", bin_dir.display());
    epm_with_path(&["self-uninstall", "--yes", "--keep-binary"], &home, &path).success();

    // epc binary is left untouched — epm no longer removes it
    assert!(bin_dir.join("epc").exists(), "epc binary should be left alone");
}

#[test]
fn self_uninstall_removes_epc_state_dir() {
    let home = TempDir::new().unwrap();
    write_installed_toml(&home, "");

    let epc_dir = home.path().join(".epc");
    fs::create_dir_all(&epc_dir).unwrap();
    fs::write(epc_dir.join("services.toml"), "[services]\n").unwrap();

    epm(&["self-uninstall", "--yes", "--keep-binary"], &home).success();

    assert!(!epc_dir.exists(), "~/.epc/ should be removed");
}

#[cfg(target_os = "macos")]
#[test]
fn self_uninstall_removes_epc_launchagent() {
    let home = TempDir::new().unwrap();
    write_installed_toml(&home, "");

    let agents_dir = home.path().join("Library").join("LaunchAgents");
    fs::create_dir_all(&agents_dir).unwrap();
    let plist = agents_dir.join("com.eps.epc-startup.plist");
    fs::write(&plist, "<?xml version=\"1.0\"?>\n<plist></plist>").unwrap();

    epm(&["self-uninstall", "--yes", "--keep-binary"], &home).success();

    assert!(!plist.exists(), "LaunchAgent plist should be removed");
}

#[cfg(target_os = "linux")]
#[test]
fn self_uninstall_removes_epc_systemd_unit() {
    let home = TempDir::new().unwrap();
    write_installed_toml(&home, "");

    let systemd_dir = home.path().join(".config").join("systemd").join("user");
    fs::create_dir_all(&systemd_dir).unwrap();
    let unit = systemd_dir.join("epc-startup.service");
    fs::write(&unit, "[Unit]\nDescription=EPC\n").unwrap();

    epm(&["self-uninstall", "--yes", "--keep-binary"], &home).success();

    assert!(!unit.exists(), "systemd unit file should be removed");
}

#[test]
fn self_uninstall_succeeds_when_epc_not_installed() {
    let home = TempDir::new().unwrap();
    write_installed_toml(&home, "");
    // No epc binary on PATH, no ~/.epc/ — should not panic
    epm_with_path(&["self-uninstall", "--yes", "--keep-binary"], &home, "/nonexistent").success();
}

#[test]
fn self_uninstall_prints_epc_state_removed() {
    let home = TempDir::new().unwrap();
    write_installed_toml(&home, "");

    let epc_dir = home.path().join(".epc");
    fs::create_dir_all(&epc_dir).unwrap();

    let out = epm(&["self-uninstall", "--yes", "--keep-binary"], &home)
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(out).unwrap();

    assert!(stdout.contains(".epc"), "should mention ~/.epc/ removal");
}
