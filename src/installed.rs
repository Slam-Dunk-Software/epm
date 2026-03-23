//! Tracks what epm has installed so `epm self-uninstall` can clean up precisely.
//!
//! State is stored in `~/.epm/installed.toml`.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

// ── data model ────────────────────────────────────────────────────────────────

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct InstalledManifest {
    #[serde(default)]
    pub skills: Vec<InstalledSkillPkg>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InstalledSkillPkg {
    pub name: String,
    /// Absolute paths of files installed to `~/.claude/commands/`
    pub files: Vec<String>,
}

// ── load / save ───────────────────────────────────────────────────────────────

impl InstalledManifest {
    /// Load from `~/.epm/installed.toml`. Returns an empty manifest if the file
    /// does not exist or cannot be parsed (best-effort).
    pub fn load(home: &Path) -> Self {
        let path = manifest_path(home);
        let Ok(raw) = std::fs::read_to_string(&path) else {
            return Self::default();
        };
        toml::from_str(&raw).unwrap_or_default()
    }

    /// Persist to `~/.epm/installed.toml`, creating the directory if needed.
    pub fn save(&self, home: &Path) -> Result<()> {
        let path = manifest_path(home);
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)
                .with_context(|| format!("could not create {}", parent.display()))?;
        }
        let out = toml::to_string_pretty(self).context("could not serialize installed.toml")?;
        std::fs::write(&path, out)
            .with_context(|| format!("could not write {}", path.display()))?;
        Ok(())
    }

    // ── mutations ─────────────────────────────────────────────────────────────

    pub fn add_skills(&mut self, name: &str, files: Vec<String>) {
        self.skills.retain(|s| s.name != name);
        self.skills.push(InstalledSkillPkg {
            name: name.to_string(),
            files,
        });
    }

    pub fn remove_skills(&mut self, name: &str) {
        self.skills.retain(|s| s.name != name);
    }
}

fn manifest_path(home: &Path) -> PathBuf {
    home.join(".epm").join("installed.toml")
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn home() -> TempDir {
        TempDir::new().unwrap()
    }

    #[test]
    fn load_returns_empty_when_file_missing() {
        let h = home();
        let m = InstalledManifest::load(h.path());
        assert!(m.skills.is_empty());
    }

    #[test]
    fn save_and_load_roundtrip_skills() {
        let h = home();
        let mut m = InstalledManifest::default();
        m.add_skills(
            "eps_skills",
            vec![
                "/home/user/.claude/commands/semver-bump.md".to_string(),
                "/home/user/.claude/commands/epc-release.md".to_string(),
            ],
        );
        m.save(h.path()).unwrap();

        let loaded = InstalledManifest::load(h.path());
        assert_eq!(loaded.skills.len(), 1);
        assert_eq!(loaded.skills[0].name, "eps_skills");
        assert_eq!(loaded.skills[0].files.len(), 2);
    }

    #[test]
    fn add_skills_replaces_existing_entry() {
        let h = home();
        let mut m = InstalledManifest::default();
        m.add_skills("eps_skills", vec!["/old/file.md".to_string()]);
        m.add_skills("eps_skills", vec!["/new/file.md".to_string()]);
        m.save(h.path()).unwrap();

        let loaded = InstalledManifest::load(h.path());
        assert_eq!(loaded.skills.len(), 1);
        assert_eq!(loaded.skills[0].files[0], "/new/file.md");
    }

    #[test]
    fn remove_skills_removes_by_name() {
        let h = home();
        let mut m = InstalledManifest::default();
        m.add_skills("eps_skills", vec!["/a.md".to_string()]);
        m.add_skills("other_skills", vec!["/b.md".to_string()]);
        m.remove_skills("eps_skills");
        m.save(h.path()).unwrap();

        let loaded = InstalledManifest::load(h.path());
        assert_eq!(loaded.skills.len(), 1);
        assert_eq!(loaded.skills[0].name, "other_skills");
    }

    #[test]
    fn save_creates_epm_dir_if_missing() {
        let h = home();
        let mut m = InstalledManifest::default();
        m.add_skills("x", vec!["/bin/x.md".to_string()]);
        m.save(h.path()).unwrap();
        assert!(h.path().join(".epm").join("installed.toml").exists());
    }

    #[test]
    fn load_returns_empty_on_corrupt_toml() {
        let h = home();
        std::fs::create_dir_all(h.path().join(".epm")).unwrap();
        std::fs::write(h.path().join(".epm").join("installed.toml"), "NOT TOML ][[[").unwrap();
        let m = InstalledManifest::load(h.path());
        assert!(m.skills.is_empty());
    }
}
