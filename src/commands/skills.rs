//! `epm skills` — install, list, and remove Claude Code skill packages.
//!
//! Skills are EPS packages with a `[skills]` section in their eps.toml.
//! Installing one copies the declared `.md` files to `~/.claude/commands/`
//! so they appear as slash commands in Claude Code.

use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use clap::Subcommand;

use crate::client::RegistryClient;
use crate::commands::{install, list::list_installed_versions, uninstall};
use crate::installed::InstalledManifest;

#[derive(Subcommand)]
pub enum SkillsCommands {
    /// Install a skills package and copy skill files to ~/.claude/commands/
    Install {
        /// Package name (must have a [skills] section in its eps.toml)
        name: String,
    },
    /// List all installed skill files
    List,
    /// Remove a skills package and delete its skill files from ~/.claude/commands/
    Remove {
        /// Package name
        name: String,
    },
}

pub async fn run(cmd: &SkillsCommands, client: &RegistryClient) -> Result<()> {
    match cmd {
        SkillsCommands::Install { name } => run_install(client, name).await,
        SkillsCommands::List => run_list(),
        SkillsCommands::Remove { name } => run_remove(name),
    }
}

// ── install ───────────────────────────────────────────────────────────────────

async fn run_install(client: &RegistryClient, name: &str) -> Result<()> {
    install::run(client, name).await?;

    let pkg_root = packages_dir()?.join(name);
    let versions = list_installed_versions(&pkg_root)?;
    let version = versions
        .last()
        .ok_or_else(|| anyhow::anyhow!("install succeeded but no version found for '{name}'"))?
        .clone();

    let install_dir = pkg_root.join(&version);
    let manifest = read_manifest(&install_dir)
        .with_context(|| format!("could not read eps.toml for '{name}@{version}'"))?;

    let files = &manifest.skills.files;
    if files.is_empty() {
        bail!("'{name}' has no [skills] section — nothing to install");
    }

    let dest = claude_commands_dir()?;
    let installed_count = install_skill_files(files, &install_dir, &dest)?;

    // Record installed file paths in ~/.epm/installed.toml
    let home = dirs::home_dir().context("could not determine home directory")?;
    let installed_paths: Vec<String> = files
        .iter()
        .map(|f| {
            let filename = Path::new(f).file_name().unwrap_or_default();
            dest.join(filename).to_string_lossy().to_string()
        })
        .collect();
    let mut manifest = InstalledManifest::load(&home);
    manifest.add_skills(name, installed_paths);
    manifest.save(&home)?;

    println!("\n✓ {name} skills installed ({} file{})", installed_count, if installed_count == 1 { "" } else { "s" });
    for f in files {
        let fname = Path::new(f).file_name().unwrap_or_default().to_string_lossy();
        println!("  /{}", fname.trim_end_matches(".md"));
    }
    println!("\nIf you have any Claude Code instances running, you'll need to restart them to access the new skills.");
    Ok(())
}

// ── list ──────────────────────────────────────────────────────────────────────

fn run_list() -> Result<()> {
    let dir = claude_commands_dir()?;
    let skills = list_skills(&dir)?;

    if skills.is_empty() {
        println!("No skills installed (~/.claude/commands/ is empty).");
    } else {
        println!("Installed skills\n");
        for s in &skills {
            println!("  /{s}");
        }
    }
    Ok(())
}

// ── remove ────────────────────────────────────────────────────────────────────

fn run_remove(name: &str) -> Result<()> {
    let pkg_root = packages_dir()?.join(name);
    let versions = list_installed_versions(&pkg_root)?;

    if versions.is_empty() {
        bail!("'{name}' is not installed");
    }

    // Collect skill files from all installed versions before removing
    let dest = claude_commands_dir()?;
    for version in &versions {
        let install_dir = pkg_root.join(version);
        if let Ok(manifest) = read_manifest(&install_dir) {
            remove_skill_files(&manifest.skills.files, &dest);
        }
    }

    // Uninstall the package
    match uninstall::run(name) {
        Ok(()) => {}
        Err(e) if e.to_string().contains("not installed") => {}
        Err(e) => eprintln!("warning: could not uninstall package: {e}"),
    }

    // Remove from ~/.epm/installed.toml
    let home = dirs::home_dir().context("could not determine home directory")?;
    let mut manifest = InstalledManifest::load(&home);
    manifest.remove_skills(name);
    manifest.save(&home)?;

    println!("Removed '{name}' skills.");
    println!("If you have any Claude Code instances running, you'll need to restart them to apply the change.");
    Ok(())
}

// ── core functions (pub for testing) ─────────────────────────────────────────

/// Copy declared skill files from `install_dir` into `skills_dir`.
/// Returns the number of files successfully copied.
pub fn install_skill_files(files: &[String], install_dir: &Path, skills_dir: &Path) -> Result<usize> {
    std::fs::create_dir_all(skills_dir)
        .with_context(|| format!("could not create {}", skills_dir.display()))?;

    let mut count = 0;
    for rel_path in files {
        let src = install_dir.join(rel_path);
        if !src.exists() {
            bail!("skill file not found: {}", src.display());
        }
        let filename = src
            .file_name()
            .with_context(|| format!("invalid skill file path: {rel_path}"))?;
        let dest = skills_dir.join(filename);
        std::fs::copy(&src, &dest)
            .with_context(|| format!("failed to copy {} to {}", src.display(), dest.display()))?;
        count += 1;
    }
    Ok(count)
}

/// Remove declared skill files from `skills_dir`. Missing files are silently skipped.
pub fn remove_skill_files(files: &[String], skills_dir: &Path) {
    for rel_path in files {
        let src = Path::new(rel_path);
        if let Some(filename) = src.file_name() {
            let target = skills_dir.join(filename);
            let _ = std::fs::remove_file(target);
        }
    }
}

/// List skill names (filename without `.md`) from `skills_dir`.
pub fn list_skills(skills_dir: &Path) -> Result<Vec<String>> {
    if !skills_dir.exists() {
        return Ok(vec![]);
    }
    let mut skills = vec![];
    for entry in std::fs::read_dir(skills_dir).context("failed to read skills directory")? {
        let entry = entry?;
        let path = entry.path();
        if path.extension().map(|e| e == "md").unwrap_or(false) {
            if let Some(stem) = path.file_stem() {
                skills.push(stem.to_string_lossy().to_string());
            }
        }
    }
    skills.sort();
    Ok(skills)
}

// ── helpers ───────────────────────────────────────────────────────────────────

fn claude_commands_dir() -> Result<PathBuf> {
    Ok(dirs::home_dir()
        .context("could not determine home directory")?
        .join(".claude")
        .join("commands"))
}

fn packages_dir() -> Result<PathBuf> {
    Ok(dirs::home_dir()
        .context("could not determine home directory")?
        .join(".epm")
        .join("packages"))
}

fn read_manifest(install_dir: &Path) -> Result<crate::models::EpsManifest> {
    let path = install_dir.join("eps.toml");
    let raw = std::fs::read_to_string(&path)?;
    Ok(toml::from_str(&raw)?)
}

// ── tests ─────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn make_skill_file(dir: &Path, name: &str) -> PathBuf {
        let path = dir.join(name);
        fs::write(&path, format!("# {name}\nDo something useful.")).unwrap();
        path
    }

    // --- install_skill_files ---

    #[test]
    fn install_copies_files_to_skills_dir() {
        let src = TempDir::new().unwrap();
        let dest = TempDir::new().unwrap();
        make_skill_file(src.path(), "semver-bump.md");
        make_skill_file(src.path(), "epc-release.md");

        let files = vec!["semver-bump.md".to_string(), "epc-release.md".to_string()];
        let count = install_skill_files(&files, src.path(), dest.path()).unwrap();

        assert_eq!(count, 2);
        assert!(dest.path().join("semver-bump.md").exists());
        assert!(dest.path().join("epc-release.md").exists());
    }

    #[test]
    fn install_creates_skills_dir_if_missing() {
        let src = TempDir::new().unwrap();
        make_skill_file(src.path(), "my-skill.md");

        let dest = TempDir::new().unwrap();
        let nested = dest.path().join("new").join("commands");

        let files = vec!["my-skill.md".to_string()];
        install_skill_files(&files, src.path(), &nested).unwrap();

        assert!(nested.join("my-skill.md").exists());
    }

    #[test]
    fn install_handles_nested_source_path() {
        let src = TempDir::new().unwrap();
        let sub = src.path().join("commands");
        fs::create_dir_all(&sub).unwrap();
        make_skill_file(&sub, "semver-bump.md");

        let dest = TempDir::new().unwrap();
        let files = vec!["commands/semver-bump.md".to_string()];
        let count = install_skill_files(&files, src.path(), dest.path()).unwrap();

        assert_eq!(count, 1);
        // Installed by filename only, not preserving subdir structure
        assert!(dest.path().join("semver-bump.md").exists());
    }

    #[test]
    fn install_errors_on_missing_source_file() {
        let src = TempDir::new().unwrap();
        let dest = TempDir::new().unwrap();

        let files = vec!["nonexistent.md".to_string()];
        let err = install_skill_files(&files, src.path(), dest.path()).unwrap_err();
        assert!(err.to_string().contains("skill file not found"));
    }

    #[test]
    fn install_returns_zero_for_empty_files_list() {
        let src = TempDir::new().unwrap();
        let dest = TempDir::new().unwrap();
        let count = install_skill_files(&[], src.path(), dest.path()).unwrap();
        assert_eq!(count, 0);
    }

    // --- remove_skill_files ---

    #[test]
    fn remove_deletes_skill_files() {
        let dir = TempDir::new().unwrap();
        make_skill_file(dir.path(), "semver-bump.md");
        make_skill_file(dir.path(), "epc-release.md");

        let files = vec!["semver-bump.md".to_string(), "epc-release.md".to_string()];
        remove_skill_files(&files, dir.path());

        assert!(!dir.path().join("semver-bump.md").exists());
        assert!(!dir.path().join("epc-release.md").exists());
    }

    #[test]
    fn remove_ignores_missing_files() {
        let dir = TempDir::new().unwrap();
        // Should not panic or error
        let files = vec!["nonexistent.md".to_string()];
        remove_skill_files(&files, dir.path());
    }

    #[test]
    fn remove_only_deletes_by_filename_not_path() {
        let dir = TempDir::new().unwrap();
        make_skill_file(dir.path(), "semver-bump.md");

        // Source path has a subdir prefix — should still find by filename
        let files = vec!["commands/semver-bump.md".to_string()];
        remove_skill_files(&files, dir.path());

        assert!(!dir.path().join("semver-bump.md").exists());
    }

    // --- list_skills ---

    #[test]
    fn list_returns_md_stems_sorted() {
        let dir = TempDir::new().unwrap();
        make_skill_file(dir.path(), "semver-bump.md");
        make_skill_file(dir.path(), "epc-release.md");
        make_skill_file(dir.path(), "note.md");

        let skills = list_skills(dir.path()).unwrap();
        assert_eq!(skills, vec!["epc-release", "note", "semver-bump"]);
    }

    #[test]
    fn list_ignores_non_md_files() {
        let dir = TempDir::new().unwrap();
        make_skill_file(dir.path(), "semver-bump.md");
        fs::write(dir.path().join("README.txt"), "ignore me").unwrap();
        fs::write(dir.path().join("config.json"), "{}").unwrap();

        let skills = list_skills(dir.path()).unwrap();
        assert_eq!(skills, vec!["semver-bump"]);
    }

    #[test]
    fn list_returns_empty_when_dir_missing() {
        let skills = list_skills(Path::new("/tmp/definitely-does-not-exist-xyzzy")).unwrap();
        assert!(skills.is_empty());
    }

    #[test]
    fn list_returns_empty_for_empty_dir() {
        let dir = TempDir::new().unwrap();
        let skills = list_skills(dir.path()).unwrap();
        assert!(skills.is_empty());
    }
}
