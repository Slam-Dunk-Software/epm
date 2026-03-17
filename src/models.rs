use std::collections::HashMap;

use serde::{Deserialize, Serialize};

pub type SystemDeps = HashMap<String, Vec<String>>;

// ── Publish models ────────────────────────────────────────────────────────────

/// Parsed from eps.toml
#[derive(Debug, Deserialize)]
pub struct EpsManifest {
    pub package: EpsPackage,
    #[serde(default)]
    pub eps: EpsSection,
    #[serde(default, rename = "system-dependencies")]
    pub system_deps: SystemDeps,
    #[serde(default)]
    pub hooks: EpsHooks,
    #[serde(default)]
    pub service: EpsService,
    #[serde(default)]
    pub skills: EpsSkills,
    #[serde(default)]
    pub mcp: EpsMcp,
}

/// Optional `[skills]` section — declares Claude Code slash command files.
/// `epm skills install` copies these to `~/.claude/commands/`.
#[derive(Debug, Deserialize, Default)]
pub struct EpsSkills {
    /// Paths relative to the package root, e.g. `["commands/semver-bump.md"]`
    #[serde(default)]
    pub files: Vec<String>,
}

/// Optional `[eps]` section — marks the repo as an EPS and carries metadata.
#[derive(Debug, Deserialize, Default)]
pub struct EpsSection {
    pub customization_guide: Option<String>,
    /// `"tool"` = maintained software, not a customizable harness.
    /// `epm new` blocks on tool packages; use `epm install` or `epm mcp install` instead.
    #[serde(default, rename = "type")]
    pub package_type: Option<String>,
}

#[derive(Debug, Deserialize)]
pub struct EpsPackage {
    pub name: String,
    pub version: String,
    pub description: String,
    pub authors: Vec<String>,
    pub license: String,
    pub repository: String,
    #[serde(default, rename = "platform")]
    pub platforms: Vec<String>,
    pub homepage: Option<String>,
}

#[derive(Debug, Deserialize, Default)]
pub struct EpsHooks {
    pub install:   Option<String>,
    pub configure: Option<String>,
    pub update:    Option<String>,
    pub uninstall: Option<String>,
}

/// Optional `[service]` section — present when the package runs as a daemon.
#[derive(Debug, Deserialize, Default)]
pub struct EpsService {
    #[serde(default)]
    pub enabled: bool,
    pub start: Option<String>,
    pub port: Option<u16>,
}

/// Optional `[mcp]` section — present when the package is an MCP server.
#[derive(Debug, Deserialize, Default)]
pub struct EpsMcp {
    /// Name of the compiled binary inside `target/release/`
    pub binary: Option<String>,
    /// Optional extra CLI args to pass when registering with the MCP client
    #[serde(default)]
    pub args: Vec<String>,
    /// Optional env vars to set in the MCP client config
    #[serde(default)]
    pub env: std::collections::HashMap<String, String>,
}

/// Sent to POST /api/v1/packages
#[derive(Debug, Serialize)]
pub struct PublishRequest {
    pub name: String,
    pub version: String,
    pub description: String,
    pub authors: Vec<String>,
    pub license: String,
    pub repository: String,
    pub platforms: Vec<String>,
    pub homepage: Option<String>,
    pub git_url: String,
    pub commit_sha: String,
    pub manifest_hash: String,
    pub system_deps: SystemDeps,
}

/// Returned by 201 response (mirrors server's ApiVersion)
#[derive(Debug, Deserialize)]
pub struct PublishedVersion {
    pub id: i64,
    pub package_id: i64,
    pub version: String,
    pub git_url: String,
    pub commit_sha: String,
    pub manifest_hash: String,
    pub yanked: bool,
    pub published_at: String,
    pub system_deps: SystemDeps,
}

// ── Adoption models ───────────────────────────────────────────────────────────

/// Written to `vendor/<name>/.adopted.toml` when a package is adopted.
#[derive(Debug, Serialize, Deserialize)]
pub struct AdoptionRecord {
    pub adoption: AdoptionMeta,
}

#[derive(Debug, Serialize, Deserialize)]
pub struct AdoptionMeta {
    pub name: String,
    pub upstream_git_url: String,
    pub adopted_version: String,
    pub adopted_commit: String,
}

// ── Registry models ───────────────────────────────────────────────────────────

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct Package {
    pub id:          i64,
    pub name:        String,
    pub description: String,
    pub authors:     Vec<String>,
    pub license:     String,
    pub homepage:    Option<String>,
    pub repository:  String,
    pub platforms:   Vec<String>,
    pub created_at:  String,
    pub updated_at:  String,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct Version {
    pub id:            i64,
    pub package_id:    i64,
    pub version:       String,
    pub git_url:       String,
    pub commit_sha:    String,
    pub manifest_hash: String,
    pub yanked:        bool,
    pub published_at:  String,
    pub system_deps:   SystemDeps,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
pub struct PackageWithVersions {
    // flattened Package fields
    pub id:          i64,
    pub name:        String,
    pub description: String,
    pub authors:     Vec<String>,
    pub license:     String,
    pub homepage:    Option<String>,
    pub repository:  String,
    pub platforms:   Vec<String>,
    pub created_at:  String,
    pub updated_at:  String,
    pub versions:    Vec<Version>,
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- EpsSkills ---

    #[test]
    fn eps_skills_defaults_to_empty_when_absent() {
        let toml = r#"
[package]
name        = "mypkg"
version     = "0.1.0"
description = "Test"
authors     = ["nick"]
license     = "MIT"
repository  = "https://github.com/nick/mypkg"
"#;
        let m: EpsManifest = toml::from_str(toml).unwrap();
        assert!(m.skills.files.is_empty());
    }

    #[test]
    fn eps_skills_parses_files_list() {
        let toml = r#"
[package]
name        = "eps_skills"
version     = "0.1.0"
description = "Skills"
authors     = ["nick"]
license     = "MIT"
repository  = "https://github.com/nick/eps_skills"

[skills]
files = ["commands/semver-bump.md", "commands/epc-release.md"]
"#;
        let m: EpsManifest = toml::from_str(toml).unwrap();
        assert_eq!(m.skills.files, vec!["commands/semver-bump.md", "commands/epc-release.md"]);
    }

    #[test]
    fn eps_skills_empty_files_list() {
        let toml = r#"
[package]
name        = "eps_skills"
version     = "0.1.0"
description = "Skills"
authors     = ["nick"]
license     = "MIT"
repository  = "https://github.com/nick/eps_skills"

[skills]
files = []
"#;
        let m: EpsManifest = toml::from_str(toml).unwrap();
        assert!(m.skills.files.is_empty());
    }

    // --- EpsManifest / EpsPackage (TOML parsing) ---

    #[test]
    fn eps_manifest_parses_platform_singular() {
        let toml = r#"
[package]
name        = "hookplayer"
version     = "0.2.0"
description = "Sound player"
authors     = ["nick"]
license     = "MIT"
repository  = "https://github.com/nick/hookplayer"
platform    = ["aarch64-apple-darwin", "x86_64-apple-darwin"]
"#;
        let m: EpsManifest = toml::from_str(toml).unwrap();
        assert_eq!(m.package.name, "hookplayer");
        assert_eq!(m.package.platforms, vec!["aarch64-apple-darwin", "x86_64-apple-darwin"]);
    }

    #[test]
    fn eps_manifest_platforms_defaults_to_empty_when_absent() {
        let toml = r#"
[package]
name        = "simple_todo"
version     = "0.1.0"
description = "Todo app"
authors     = ["nick"]
license     = "MIT"
repository  = "https://github.com/nick/simple_todo"
"#;
        let m: EpsManifest = toml::from_str(toml).unwrap();
        assert!(m.package.platforms.is_empty());
    }

    #[test]
    fn eps_manifest_parses_hooks_section() {
        let toml = r#"
[package]
name        = "hookplayer"
version     = "0.2.0"
description = "Sound player"
authors     = ["nick"]
license     = "MIT"
repository  = "https://github.com/nick/hookplayer"
platform    = ["aarch64-apple-darwin"]

[hooks]
install = "install.sh"
"#;
        let m: EpsManifest = toml::from_str(toml).unwrap();
        assert_eq!(m.hooks.install, Some("install.sh".to_string()));
        assert!(m.hooks.configure.is_none());
        assert!(m.hooks.update.is_none());
        assert!(m.hooks.uninstall.is_none());
    }

    #[test]
    fn eps_manifest_hooks_defaults_to_none_when_absent() {
        let toml = r#"
[package]
name        = "simple_todo"
version     = "0.1.0"
description = "Todo app"
authors     = ["nick"]
license     = "MIT"
repository  = "https://github.com/nick/simple_todo"
"#;
        let m: EpsManifest = toml::from_str(toml).unwrap();
        assert!(m.hooks.install.is_none());
        assert!(m.hooks.configure.is_none());
        assert!(m.hooks.update.is_none());
        assert!(m.hooks.uninstall.is_none());
    }

    #[test]
    fn eps_manifest_parses_all_hooks() {
        let toml = r#"
[package]
name        = "mypkg"
version     = "1.0.0"
description = "Test"
authors     = ["nick"]
license     = "MIT"
repository  = "https://github.com/nick/mypkg"

[hooks]
install   = "scripts/install.sh"
configure = "scripts/configure.sh"
update    = "scripts/update.sh"
uninstall = "scripts/uninstall.sh"
"#;
        let m: EpsManifest = toml::from_str(toml).unwrap();
        assert_eq!(m.hooks.install,   Some("scripts/install.sh".to_string()));
        assert_eq!(m.hooks.configure, Some("scripts/configure.sh".to_string()));
        assert_eq!(m.hooks.update,    Some("scripts/update.sh".to_string()));
        assert_eq!(m.hooks.uninstall, Some("scripts/uninstall.sh".to_string()));
    }

    // --- Package ---

    #[test]
    fn package_deserializes_from_json() {
        let json = r#"{
            "id": 1,
            "name": "tech_talker",
            "description": "Audio transcription harness",
            "authors": ["nickagliano"],
            "license": "MIT",
            "homepage": "https://example.com",
            "repository": "https://github.com/test/tech_talker",
            "platforms": ["aarch64-apple-darwin"],
            "created_at": "2025-01-01T00:00:00",
            "updated_at": "2025-01-01T00:00:00"
        }"#;
        let pkg: Package = serde_json::from_str(json).unwrap();
        assert_eq!(pkg.id, 1);
        assert_eq!(pkg.name, "tech_talker");
        assert_eq!(pkg.description, "Audio transcription harness");
        assert_eq!(pkg.license, "MIT");
        assert_eq!(pkg.homepage, Some("https://example.com".to_string()));
        assert_eq!(pkg.authors, vec!["nickagliano"]);
        assert_eq!(pkg.platforms, vec!["aarch64-apple-darwin"]);
    }

    #[test]
    fn package_deserializes_with_null_homepage() {
        let json = r#"{
            "id": 2,
            "name": "pi",
            "description": "Minimal agent harness",
            "authors": ["armin"],
            "license": "Apache-2.0",
            "homepage": null,
            "repository": "https://github.com/test/pi",
            "platforms": ["x86_64-unknown-linux-gnu"],
            "created_at": "2025-06-01T00:00:00",
            "updated_at": "2025-06-01T00:00:00"
        }"#;
        let pkg: Package = serde_json::from_str(json).unwrap();
        assert_eq!(pkg.homepage, None);
    }

    // --- Version ---

    #[test]
    fn version_deserializes_from_json() {
        let json = r#"{
            "id": 10,
            "package_id": 1,
            "version": "0.1.0",
            "git_url": "https://github.com/test/tech_talker",
            "commit_sha": "deadbeef",
            "manifest_hash": "cafebabe",
            "yanked": false,
            "published_at": "2025-01-15T00:00:00",
            "system_deps": {}
        }"#;
        let v: Version = serde_json::from_str(json).unwrap();
        assert_eq!(v.id, 10);
        assert_eq!(v.package_id, 1);
        assert_eq!(v.version, "0.1.0");
        assert_eq!(v.commit_sha, "deadbeef");
        assert!(!v.yanked);
        assert!(v.system_deps.is_empty());
    }

    #[test]
    fn version_yanked_true_deserializes() {
        let json = r#"{
            "id": 11,
            "package_id": 1,
            "version": "0.0.1",
            "git_url": "https://github.com/test/tech_talker",
            "commit_sha": "abad1dea",
            "manifest_hash": "facade",
            "yanked": true,
            "published_at": "2024-12-01T00:00:00",
            "system_deps": {}
        }"#;
        let v: Version = serde_json::from_str(json).unwrap();
        assert!(v.yanked);
    }

    #[test]
    fn version_system_deps_deserializes() {
        let json = r#"{
            "id": 12,
            "package_id": 1,
            "version": "0.2.0",
            "git_url": "https://github.com/test/tech_talker",
            "commit_sha": "c0ffee",
            "manifest_hash": "badc0de",
            "yanked": false,
            "published_at": "2025-02-01T00:00:00",
            "system_deps": {"brew": ["cmake", "libomp"], "gem": ["xcpretty"]}
        }"#;
        let v: Version = serde_json::from_str(json).unwrap();
        assert_eq!(v.system_deps["brew"], vec!["cmake", "libomp"]);
        assert_eq!(v.system_deps["gem"], vec!["xcpretty"]);
    }

    // --- PackageWithVersions ---

    #[test]
    fn package_with_versions_deserializes() {
        let json = r#"{
            "id": 1,
            "name": "tech_talker",
            "description": "Audio transcription harness",
            "authors": ["nickagliano"],
            "license": "MIT",
            "homepage": null,
            "repository": "https://github.com/test/tech_talker",
            "platforms": ["aarch64-apple-darwin"],
            "created_at": "2025-01-01T00:00:00",
            "updated_at": "2025-01-01T00:00:00",
            "versions": [
                {
                    "id": 1,
                    "package_id": 1,
                    "version": "0.1.0",
                    "git_url": "https://github.com/test/tech_talker",
                    "commit_sha": "deadbeef",
                    "manifest_hash": "cafebabe",
                    "yanked": false,
                    "published_at": "2025-01-15T00:00:00",
                    "system_deps": {}
                }
            ]
        }"#;
        let pkg: PackageWithVersions = serde_json::from_str(json).unwrap();
        assert_eq!(pkg.name, "tech_talker");
        assert_eq!(pkg.authors, vec!["nickagliano"]);
        assert_eq!(pkg.platforms, vec!["aarch64-apple-darwin"]);
        assert_eq!(pkg.versions.len(), 1);
        assert_eq!(pkg.versions[0].version, "0.1.0");
    }

    #[test]
    fn package_with_versions_empty_versions_array() {
        let json = r#"{
            "id": 1,
            "name": "empty_pkg",
            "description": "No versions yet",
            "authors": [],
            "license": "MIT",
            "homepage": null,
            "repository": "https://github.com/test/empty_pkg",
            "platforms": [],
            "created_at": "2025-01-01T00:00:00",
            "updated_at": "2025-01-01T00:00:00",
            "versions": []
        }"#;
        let pkg: PackageWithVersions = serde_json::from_str(json).unwrap();
        assert!(pkg.versions.is_empty());
    }

    #[test]
    fn package_with_versions_multiple_versions_preserves_order() {
        let json = r#"{
            "id": 1,
            "name": "multi_ver",
            "description": "Multiple versions",
            "authors": [],
            "license": "MIT",
            "homepage": null,
            "repository": "https://github.com/test/multi_ver",
            "platforms": [],
            "created_at": "2025-01-01T00:00:00",
            "updated_at": "2025-01-01T00:00:00",
            "versions": [
                {
                    "id": 3,
                    "package_id": 1,
                    "version": "0.3.0",
                    "git_url": "https://github.com/test/multi_ver",
                    "commit_sha": "sha3",
                    "manifest_hash": "hash3",
                    "yanked": false,
                    "published_at": "2025-03-01T00:00:00",
                    "system_deps": {}
                },
                {
                    "id": 2,
                    "package_id": 1,
                    "version": "0.2.0",
                    "git_url": "https://github.com/test/multi_ver",
                    "commit_sha": "sha2",
                    "manifest_hash": "hash2",
                    "yanked": false,
                    "published_at": "2025-02-01T00:00:00",
                    "system_deps": {}
                },
                {
                    "id": 1,
                    "package_id": 1,
                    "version": "0.1.0",
                    "git_url": "https://github.com/test/multi_ver",
                    "commit_sha": "sha1",
                    "manifest_hash": "hash1",
                    "yanked": false,
                    "published_at": "2025-01-01T00:00:00",
                    "system_deps": {}
                }
            ]
        }"#;
        let pkg: PackageWithVersions = serde_json::from_str(json).unwrap();
        assert_eq!(pkg.versions.len(), 3);
        assert_eq!(pkg.versions[0].version, "0.3.0");
        assert_eq!(pkg.versions[1].version, "0.2.0");
        assert_eq!(pkg.versions[2].version, "0.1.0");
    }
}
