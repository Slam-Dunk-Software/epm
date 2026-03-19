/// Stores and retrieves per-registry auth tokens.
///
/// Format: `~/.epm/credentials`
/// ```toml
/// [default]
/// registry = "https://epm.dev"
/// token    = "abc123..."
/// ```
use std::os::unix::fs::PermissionsExt;
use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Serialize, Deserialize)]
struct CredentialsFile {
    #[serde(rename = "default", default)]
    default: Option<Profile>,
    #[serde(flatten)]
    profiles: std::collections::HashMap<String, Profile>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Profile {
    registry: String,
    token: String,
}

fn credentials_path_for(home: &Path) -> PathBuf {
    home.join(".epm").join("credentials")
}

pub fn credentials_path() -> Result<PathBuf> {
    let home = dirs::home_dir().context("could not determine home directory")?;
    Ok(credentials_path_for(&home))
}

/// Save a token for the given registry URL.
/// Writes to `~/.epm/credentials` with mode 0600.
pub fn save(registry: &str, token: &str) -> Result<()> {
    let path = credentials_path()?;
    save_to(&path, registry, token)
}

pub fn save_to(path: &Path, registry: &str, token: &str) -> Result<()> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)
            .with_context(|| format!("failed to create {}", parent.display()))?;
    }

    let profile = Profile {
        registry: registry.to_string(),
        token: token.to_string(),
    };

    // Build or update the credentials file
    let mut creds: CredentialsFile = if path.exists() {
        let raw = std::fs::read_to_string(path)
            .with_context(|| format!("failed to read {}", path.display()))?;
        toml::from_str(&raw).unwrap_or_default()
    } else {
        CredentialsFile::default()
    };

    // Store under the "default" profile and also under a hostname-derived key
    creds.default = Some(profile.clone());
    let key = registry_key(registry);
    creds.profiles.insert(key, profile);

    let content = toml::to_string_pretty(&creds).context("failed to serialize credentials")?;
    std::fs::write(path, &content)
        .with_context(|| format!("failed to write {}", path.display()))?;

    // chmod 600
    let perms = std::fs::Permissions::from_mode(0o600);
    std::fs::set_permissions(path, perms)
        .with_context(|| format!("failed to set permissions on {}", path.display()))?;

    Ok(())
}

/// Load the token for the given registry URL.
/// Returns `None` if credentials file is missing or no matching entry exists.
pub fn load(registry: &str) -> Result<Option<String>> {
    let path = credentials_path()?;
    load_from(&path, registry)
}

pub fn load_from(path: &Path, registry: &str) -> Result<Option<String>> {
    if !path.exists() {
        return Ok(None);
    }

    let raw = std::fs::read_to_string(path)
        .with_context(|| format!("failed to read {}", path.display()))?;
    let creds: CredentialsFile = toml::from_str(&raw).unwrap_or_default();

    // Try exact registry key first
    let key = registry_key(registry);
    if let Some(profile) = creds.profiles.get(&key) {
        return Ok(Some(profile.token.clone()));
    }

    // Fall back to "default" if it matches the registry
    if let Some(default) = &creds.default {
        if default.registry == registry {
            return Ok(Some(default.token.clone()));
        }
    }

    Ok(None)
}

/// Derives a TOML key from a registry URL by stripping scheme and slashes.
fn registry_key(registry: &str) -> String {
    registry
        .trim_start_matches("https://")
        .trim_start_matches("http://")
        .trim_end_matches('/')
        .replace('.', "_")
        .replace('-', "_")
        .replace(':', "_")
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    fn tmp_creds(dir: &TempDir) -> PathBuf {
        dir.path().join(".epm").join("credentials")
    }

    #[test]
    fn save_and_load_roundtrip() {
        let dir = TempDir::new().unwrap();
        let path = tmp_creds(&dir);

        save_to(&path, "https://epm.dev", "mytoken123").unwrap();
        let loaded = load_from(&path, "https://epm.dev").unwrap();
        assert_eq!(loaded, Some("mytoken123".to_string()));
    }

    #[test]
    fn load_returns_none_when_file_missing() {
        let dir = TempDir::new().unwrap();
        let path = tmp_creds(&dir);

        let loaded = load_from(&path, "https://epm.dev").unwrap();
        assert_eq!(loaded, None);
    }

    #[test]
    fn load_returns_none_for_different_registry() {
        let dir = TempDir::new().unwrap();
        let path = tmp_creds(&dir);

        save_to(&path, "https://epm.dev", "token_a").unwrap();
        let loaded = load_from(&path, "https://other.registry.dev").unwrap();
        assert_eq!(loaded, None);
    }

    #[test]
    fn save_creates_parent_directory() {
        let dir = TempDir::new().unwrap();
        let path = tmp_creds(&dir);

        assert!(!path.parent().unwrap().exists());
        save_to(&path, "https://epm.dev", "tok").unwrap();
        assert!(path.exists());
    }

    #[test]
    fn saved_file_has_mode_0600() {
        let dir = TempDir::new().unwrap();
        let path = tmp_creds(&dir);

        save_to(&path, "https://epm.dev", "tok").unwrap();
        let meta = std::fs::metadata(&path).unwrap();
        let mode = meta.permissions().mode();
        assert_eq!(mode & 0o777, 0o600, "expected mode 0600, got {:o}", mode & 0o777);
    }

    #[test]
    fn second_save_overwrites_token() {
        let dir = TempDir::new().unwrap();
        let path = tmp_creds(&dir);

        save_to(&path, "https://epm.dev", "old_token").unwrap();
        save_to(&path, "https://epm.dev", "new_token").unwrap();
        let loaded = load_from(&path, "https://epm.dev").unwrap();
        assert_eq!(loaded, Some("new_token".to_string()));
    }
}
