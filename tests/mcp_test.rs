use assert_cmd::Command;
use predicates::prelude::*;
use serde_json::json;
use tempfile::TempDir;

fn epm_mcp(args: &[&str], home: &TempDir) -> assert_cmd::assert::Assert {
    Command::cargo_bin("epm")
        .unwrap()
        .args(["mcp"].iter().chain(args).copied().collect::<Vec<_>>())
        .env("HOME", home.path())
        .assert()
}

fn write_claude_json(home: &TempDir, content: &serde_json::Value) {
    let path = home.path().join(".claude.json");
    std::fs::write(path, serde_json::to_string_pretty(content).unwrap()).unwrap();
}

fn read_claude_json(home: &TempDir) -> serde_json::Value {
    let path = home.path().join(".claude.json");
    let raw = std::fs::read_to_string(path).unwrap();
    serde_json::from_str(&raw).unwrap()
}

// ── mcp list ─────────────────────────────────────────────────────────────────

#[test]
fn mcp_list_no_claude_json_shows_message() {
    let home = TempDir::new().unwrap();
    epm_mcp(&["list"], &home)
        .success()
        .stdout(predicate::str::contains("No MCP servers registered"));
}

#[test]
fn mcp_list_empty_mcp_servers_shows_message() {
    let home = TempDir::new().unwrap();
    write_claude_json(&home, &json!({ "mcpServers": {} }));
    epm_mcp(&["list"], &home)
        .success()
        .stdout(predicate::str::contains("No MCP servers registered"));
}

#[test]
fn mcp_list_shows_registered_server_name() {
    let home = TempDir::new().unwrap();
    write_claude_json(&home, &json!({
        "mcpServers": {
            "eps_mcp": { "command": "/usr/local/bin/eps_mcp", "args": [], "env": {} }
        }
    }));
    epm_mcp(&["list"], &home)
        .success()
        .stdout(predicate::str::contains("eps_mcp"));
}

#[test]
fn mcp_list_shows_registered_server_command() {
    let home = TempDir::new().unwrap();
    write_claude_json(&home, &json!({
        "mcpServers": {
            "eps_mcp": { "command": "/usr/local/bin/eps_mcp", "args": [], "env": {} }
        }
    }));
    epm_mcp(&["list"], &home)
        .success()
        .stdout(predicate::str::contains("/usr/local/bin/eps_mcp"));
}

#[test]
fn mcp_list_shows_multiple_servers() {
    let home = TempDir::new().unwrap();
    write_claude_json(&home, &json!({
        "mcpServers": {
            "eps_mcp":  { "command": "/bin/eps_mcp",  "args": [], "env": {} },
            "other_mcp": { "command": "/bin/other_mcp", "args": [], "env": {} }
        }
    }));
    let out = epm_mcp(&["list"], &home).success().get_output().stdout.clone();
    let stdout = String::from_utf8(out).unwrap();
    assert!(stdout.contains("eps_mcp"),   "expected eps_mcp in: {stdout}");
    assert!(stdout.contains("other_mcp"), "expected other_mcp in: {stdout}");
}

// ── mcp remove ───────────────────────────────────────────────────────────────

#[test]
fn mcp_remove_deletes_entry_from_claude_json() {
    let home = TempDir::new().unwrap();
    write_claude_json(&home, &json!({
        "mcpServers": {
            "eps_mcp": { "command": "/bin/eps_mcp", "args": [], "env": {} }
        }
    }));

    epm_mcp(&["remove", "eps_mcp"], &home).success();

    let val = read_claude_json(&home);
    assert!(
        val["mcpServers"].get("eps_mcp").is_none(),
        "eps_mcp should have been removed"
    );
}

#[test]
fn mcp_remove_leaves_other_servers_intact() {
    let home = TempDir::new().unwrap();
    write_claude_json(&home, &json!({
        "mcpServers": {
            "eps_mcp":  { "command": "/bin/eps_mcp",  "args": [], "env": {} },
            "other_mcp": { "command": "/bin/other", "args": [], "env": {} }
        }
    }));

    epm_mcp(&["remove", "eps_mcp"], &home).success();

    let val = read_claude_json(&home);
    assert!(val["mcpServers"].get("other_mcp").is_some(), "other_mcp should remain");
}

#[test]
fn mcp_remove_prints_success_message() {
    let home = TempDir::new().unwrap();
    write_claude_json(&home, &json!({
        "mcpServers": {
            "eps_mcp": { "command": "/bin/eps_mcp", "args": [], "env": {} }
        }
    }));

    epm_mcp(&["remove", "eps_mcp"], &home)
        .success()
        .stdout(predicate::str::contains("eps_mcp"));
}

#[test]
fn mcp_remove_server_not_registered_exits_nonzero() {
    let home = TempDir::new().unwrap();
    write_claude_json(&home, &json!({ "mcpServers": {} }));

    epm_mcp(&["remove", "eps_mcp"], &home).failure();
}

#[test]
fn mcp_remove_no_claude_json_exits_nonzero() {
    let home = TempDir::new().unwrap();
    epm_mcp(&["remove", "eps_mcp"], &home).failure();
}
