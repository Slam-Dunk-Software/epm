use std::path::Path;
use std::process::Command as StdCommand;

use anyhow::{bail, Context, Result};
use hex::encode as hex_encode;
use sha2::{Digest, Sha256};

use crate::{
    client::RegistryClient,
    models::{EpsManifest, PublishRequest},
};

/// Check that `git tag -l "v{version}"` returns the expected tag in the given directory.
/// Pass `cwd = None` to use the current directory (normal publish flow).
pub fn check_git_tag(version: &str, cwd: Option<&Path>) -> Result<()> {
    let tag = format!("v{version}");
    let mut cmd = StdCommand::new("git");
    cmd.args(["tag", "-l", &tag]);
    if let Some(dir) = cwd {
        cmd.current_dir(dir);
    }
    let out = cmd.output().context("failed to run git tag -l")?;
    let found = String::from_utf8(out.stdout)?.trim().to_string();
    if found.is_empty() {
        bail!(
            "Tag {tag} not found. Create it first:\n  git tag {tag} && git push origin {tag}"
        );
    }
    Ok(())
}

pub async fn run(client: &RegistryClient, manifest_path: &Path) -> Result<()> {
    let bytes = std::fs::read(manifest_path)
        .with_context(|| format!("could not read '{}'", manifest_path.display()))?;

    let manifest: EpsManifest = toml::from_str(
        std::str::from_utf8(&bytes).context("eps.toml is not valid UTF-8")?,
    )
    .with_context(|| format!("failed to parse '{}'", manifest_path.display()))?;
    let pkg = &manifest.package;

    let git_out = StdCommand::new("git")
        .args(["rev-parse", "HEAD"])
        .output()
        .context("failed to run git rev-parse HEAD")?;
    if !git_out.status.success() {
        bail!("not in a git repository (git rev-parse HEAD failed)");
    }
    let commit_sha = String::from_utf8(git_out.stdout)?.trim().to_string();

    // Require a git tag matching the version before publishing
    check_git_tag(&pkg.version, None)?;

    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let manifest_hash = format!("sha256:{}", hex_encode(hasher.finalize()));

    println!("Publishing {}@{} ...", pkg.name, pkg.version);

    let published = client
        .publish_package(&PublishRequest {
            name: pkg.name.clone(),
            version: pkg.version.clone(),
            description: pkg.description.clone(),
            authors: pkg.authors.clone(),
            license: pkg.license.clone(),
            repository: pkg.repository.clone(),
            platforms: pkg.platforms.clone(),
            homepage: pkg.homepage.clone(),
            git_url: pkg.repository.clone(),
            commit_sha,
            manifest_hash,
            system_deps: manifest.system_deps.clone(),
        })
        .await?;

    println!("Published {}@{}", pkg.name, published.version);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::TempDir;

    fn init_git_repo(dir: &Path) {
        StdCommand::new("git").args(["init"]).current_dir(dir).output().unwrap();
        StdCommand::new("git").args(["config", "user.email", "test@test.com"]).current_dir(dir).output().unwrap();
        StdCommand::new("git").args(["config", "user.name", "Test"]).current_dir(dir).output().unwrap();
        // Need at least one commit so tagging works
        fs::write(dir.join("README.md"), "test").unwrap();
        StdCommand::new("git").args(["add", "."]).current_dir(dir).output().unwrap();
        StdCommand::new("git").args(["commit", "-m", "init"]).current_dir(dir).output().unwrap();
    }

    #[test]
    fn check_git_tag_passes_when_tag_exists() {
        let dir = TempDir::new().unwrap();
        init_git_repo(dir.path());
        StdCommand::new("git").args(["tag", "v1.2.3"]).current_dir(dir.path()).output().unwrap();

        assert!(check_git_tag("1.2.3", Some(dir.path())).is_ok());
    }

    #[test]
    fn check_git_tag_fails_when_tag_absent() {
        let dir = TempDir::new().unwrap();
        init_git_repo(dir.path());

        let err = check_git_tag("9.9.9", Some(dir.path())).unwrap_err();
        assert!(err.to_string().contains("v9.9.9"), "unexpected error: {err}");
        assert!(err.to_string().contains("git tag"), "unexpected error: {err}");
    }

    #[test]
    fn check_git_tag_error_message_includes_push_hint() {
        let dir = TempDir::new().unwrap();
        init_git_repo(dir.path());

        let err = check_git_tag("0.1.0", Some(dir.path())).unwrap_err();
        assert!(err.to_string().contains("git push origin"), "no push hint: {err}");
    }

    #[test]
    fn check_git_tag_different_tag_does_not_satisfy() {
        let dir = TempDir::new().unwrap();
        init_git_repo(dir.path());
        // Tag v1.0.0 exists but we check for v2.0.0
        StdCommand::new("git").args(["tag", "v1.0.0"]).current_dir(dir.path()).output().unwrap();

        assert!(check_git_tag("2.0.0", Some(dir.path())).is_err());
    }
}
