use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::json;
use std::fs;
use tempfile::TempDir;

fn epm(args: &[&str], home: &TempDir) -> assert_cmd::assert::Assert {
    Command::cargo_bin("epm")
        .unwrap()
        .args(args)
        .env("HOME", home.path())
        .assert()
}

fn write_claude_json(home: &TempDir, content: &serde_json::Value) {
    let path = home.path().join(".claude.json");
    fs::write(path, serde_json::to_string_pretty(content).unwrap()).unwrap();
}

fn read_claude_json(home: &TempDir) -> serde_json::Value {
    let raw = fs::read_to_string(home.path().join(".claude.json")).unwrap();
    serde_json::from_str(&raw).unwrap()
}

fn write_installed_toml(home: &TempDir, content: &str) {
    let dir = home.path().join(".epm");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("installed.toml"), content).unwrap();
}

// ── self-uninstall ────────────────────────────────────────────────────────────

#[test]
fn self_uninstall_removes_tracked_mcp_from_claude_json() {
    let home = TempDir::new().unwrap();

    write_installed_toml(&home, r#"
[[mcp]]
name   = "eps_mcp"
binary = "/fake/bin/eps_mcp"
"#);
    write_claude_json(&home, &json!({
        "mcpServers": {
            "eps_mcp": { "command": "/fake/bin/eps_mcp", "args": [], "env": {} }
        }
    }));

    epm(&["self-uninstall", "--yes", "--keep-binary"], &home).success();

    let val = read_claude_json(&home);
    assert!(
        val["mcpServers"].get("eps_mcp").is_none(),
        "eps_mcp should have been removed from ~/.claude.json"
    );
}

#[test]
fn self_uninstall_leaves_untracked_mcp_intact() {
    let home = TempDir::new().unwrap();

    write_installed_toml(&home, r#"
[[mcp]]
name   = "eps_mcp"
binary = "/fake/bin/eps_mcp"
"#);
    write_claude_json(&home, &json!({
        "mcpServers": {
            "eps_mcp":    { "command": "/fake/bin/eps_mcp",  "args": [], "env": {} },
            "manual_mcp": { "command": "/fake/bin/manual",   "args": [], "env": {} }
        }
    }));

    epm(&["self-uninstall", "--yes", "--keep-binary"], &home).success();

    let val = read_claude_json(&home);
    assert!(
        val["mcpServers"].get("manual_mcp").is_some(),
        "manually registered MCP should remain"
    );
}

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
[[mcp]]
name   = "eps_mcp"
binary = "/fake/bin/eps_mcp"

[[skills]]
name  = "eps_skills"
files = ["{semver_path}"]
"#));
    write_claude_json(&home, &json!({
        "mcpServers": {
            "eps_mcp": { "command": "/fake/bin/eps_mcp", "args": [], "env": {} }
        }
    }));

    let out = epm(&["self-uninstall", "--yes", "--keep-binary"], &home)
        .success()
        .get_output()
        .stdout
        .clone();
    let stdout = String::from_utf8(out).unwrap();

    assert!(stdout.contains("eps_mcp"), "should mention removed MCP");
    assert!(stdout.contains("eps_skills"), "should mention removed skills");
}
