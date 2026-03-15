#![allow(dead_code)]

use serde_json::{json, Value};
use std::process::Command;
use tempfile::TempDir;

// ── JSON fixture builders ────────────────────────────────────────────────────

pub fn package_json(id: i64, name: &str, description: &str) -> Value {
    json!({
        "id": id,
        "name": name,
        "description": description,
        "authors": ["test_author"],
        "license": "MIT",
        "homepage": null,
        "repository": "https://github.com/test/test",
        "platforms": ["aarch64-apple-darwin"],
        "created_at": "2025-01-01T00:00:00",
        "updated_at": "2025-01-01T00:00:00"
    })
}

pub fn package_json_with_homepage(id: i64, name: &str, homepage: &str) -> Value {
    json!({
        "id": id,
        "name": name,
        "description": "test description",
        "authors": ["test_author"],
        "license": "MIT",
        "homepage": homepage,
        "repository": "https://github.com/test/test",
        "platforms": ["aarch64-apple-darwin"],
        "created_at": "2025-01-01T00:00:00",
        "updated_at": "2025-01-01T00:00:00"
    })
}

pub fn version_json(
    id: i64,
    pkg_id: i64,
    version: &str,
    git_url: &str,
    sha: &str,
    yanked: bool,
) -> Value {
    json!({
        "id": id,
        "package_id": pkg_id,
        "version": version,
        "git_url": git_url,
        "commit_sha": sha,
        "manifest_hash": "abc123",
        "yanked": yanked,
        "published_at": "2025-01-01T00:00:00",
        "system_deps": {}
    })
}

pub fn package_with_versions_json(pkg_id: i64, name: &str, versions: Vec<Value>) -> Value {
    json!({
        "id": pkg_id,
        "name": name,
        "description": "test description",
        "authors": ["test_author"],
        "license": "MIT",
        "homepage": null,
        "repository": "https://github.com/test/test",
        "platforms": ["aarch64-apple-darwin", "x86_64-apple-darwin"],
        "created_at": "2025-01-01T00:00:00",
        "updated_at": "2025-01-01T00:00:00",
        "versions": versions
    })
}

// ── TestRepo ─────────────────────────────────────────────────────────────────

/// A temporary git repository for install tests.
///
/// The directory is kept alive as long as this struct lives (via `_dir`).
/// `url` is a `file:///...` path suitable for `git clone`.
/// `sha` is the full commit hash of the initial commit.
pub struct TestRepo {
    pub _dir: TempDir,
    pub url:  String,
    pub sha:  String,
}

impl TestRepo {
    pub fn create() -> Self {
        let dir = TempDir::new().expect("failed to create temp dir for TestRepo");
        let path = dir.path();

        run_git(&["init"], path);
        run_git(&["config", "user.email", "test@epm.test"], path);
        run_git(&["config", "user.name", "EPM Test"], path);

        // Create a minimal file so the commit isn't empty
        std::fs::write(path.join("CUSTOMIZE.md"), "# Customize\nThis is a test harness.\n")
            .expect("failed to write CUSTOMIZE.md");

        run_git(&["add", "."], path);
        run_git(&["commit", "-m", "initial commit"], path);

        // Capture HEAD SHA
        let output = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(path)
            .output()
            .expect("failed to run git rev-parse HEAD");

        let sha = String::from_utf8(output.stdout)
            .expect("git rev-parse output is not UTF-8")
            .trim()
            .to_string();

        // Use the canonical path so macOS symlinks don't confuse git
        let canonical = path.canonicalize().expect("failed to canonicalize repo path");
        let url = format!("file://{}", canonical.display());

        TestRepo { _dir: dir, url, sha }
    }

    pub fn create_with_manifest(manifest_content: &str) -> Self {
        let dir = TempDir::new().expect("failed to create temp dir for TestRepo");
        let path = dir.path();

        run_git(&["init"], path);
        run_git(&["config", "user.email", "test@epm.test"], path);
        run_git(&["config", "user.name", "EPM Test"], path);

        std::fs::write(path.join("CUSTOMIZE.md"), "# Customize\n")
            .expect("failed to write CUSTOMIZE.md");
        std::fs::write(path.join("eps.toml"), manifest_content)
            .expect("failed to write eps.toml");

        run_git(&["add", "."], path);
        run_git(&["commit", "-m", "initial commit"], path);

        // Create a git tag matching the version in the manifest so `epm publish` passes
        if let Some(ver) = extract_version(manifest_content) {
            run_git(&["tag", &format!("v{ver}")], path);
        }

        let output = Command::new("git")
            .args(["rev-parse", "HEAD"])
            .current_dir(path)
            .output()
            .expect("failed to run git rev-parse HEAD");

        let sha = String::from_utf8(output.stdout)
            .expect("git rev-parse output is not UTF-8")
            .trim()
            .to_string();

        let canonical = path.canonicalize().expect("failed to canonicalize repo path");
        let url = format!("file://{}", canonical.display());

        TestRepo { _dir: dir, url, sha }
    }
}

/// Extract the `version = "x.y.z"` value from a TOML manifest string.
fn extract_version(manifest: &str) -> Option<&str> {
    manifest.lines().find_map(|line| {
        let line = line.trim();
        if line.starts_with("version") {
            let mut parts = line.splitn(2, '"');
            parts.next();      // skip `version = `
            parts.next()       // the value between first pair of quotes
                .and_then(|s| s.splitn(2, '"').next())
        } else {
            None
        }
    })
}

fn run_git(args: &[&str], dir: &std::path::Path) {
    let status = Command::new("git")
        .args(args)
        .current_dir(dir)
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .status()
        .unwrap_or_else(|e| panic!("failed to run git {args:?}: {e}"));
    if !status.success() {
        panic!("git {args:?} failed with status {status}");
    }
}
