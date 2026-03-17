use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

use anyhow::{bail, Context, Result};

use crate::client::RegistryClient;
use crate::commands::sysdeps;
use crate::models::{EpsManifest, Version};

/// Split `"name"` or `"name@version"` into `(name, Option<version>)`.
pub fn parse_spec(spec: &str) -> (&str, Option<&str>) {
    if let Some((n, v)) = spec.split_once('@') {
        (n, Some(v))
    } else {
        (spec, None)
    }
}

/// Return the latest non-yanked version by semver, or `None` if all are yanked / list is empty.
/// Versions that cannot be parsed as semver are sorted to the end.
pub fn select_latest_version(mut versions: Vec<Version>) -> Option<Version> {
    versions.sort_by(|a, b| {
        let av = semver::Version::parse(&a.version).ok();
        let bv = semver::Version::parse(&b.version).ok();
        bv.cmp(&av) // descending: higher versions first
    });
    versions.into_iter().find(|v| !v.yanked)
}

/// Core install logic: clone + checkout + sysdep check for an already-resolved version.
pub async fn install_version(client: &RegistryClient, name: &str, version: &Version) -> Result<()> {
    let install_root = dirs::home_dir()
        .context("could not determine home directory")?
        .join(".epm")
        .join("packages")
        .join(name)
        .join(&version.version);

    if install_root.exists() {
        println!(
            "\x1b[32m✓\x1b[0m \x1b[1m{name}@{}\x1b[0m \x1b[2mis already installed\x1b[0m",
            version.version,
        );
        return Ok(());
    }

    println!(
        "\x1b[2mInstalling \x1b[0m\x1b[1m{name}@{}\x1b[0m\x1b[2m...\x1b[0m",
        version.version,
    );

    let install_str = install_root
        .to_str()
        .context("install path contains non-UTF-8 characters")?;

    let clone_status = Command::new("git")
        .args(["clone", "--quiet", &version.git_url, install_str])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .context("failed to run git clone")?;

    if !clone_status.success() {
        bail!(
            "git clone failed — check your internet connection and try again.\n\
             If the problem persists, try: git clone {} {}",
            version.git_url, install_str
        );
    }

    let checkout_status = Command::new("git")
        .args(["-C", install_str, "-c", "advice.detachedHead=false", "checkout", &version.commit_sha])
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .status()
        .context("failed to run git checkout")?;

    if !checkout_status.success() {
        bail!("git checkout {} failed", version.commit_sha);
    }

    sysdeps::check_system_deps(&version.system_deps)
        .with_context(|| format!("system dependency check failed for {name}@{}", version.version))?;

    if let Ok(manifest) = read_local_manifest(&install_root) {
        if let Some(hook) = manifest.hooks.install {
            run_hook(&hook, &install_root, name, &version.version)
                .with_context(|| format!("install hook failed for {name}@{}", version.version))?;
        }
    }

    println!(
        "\n\x1b[32m✓\x1b[0m \x1b[1m{name}@{}\x1b[0m \x1b[2minstalled\x1b[0m",
        version.version,
    );

    client.track_install(name, &version.version).await;

    Ok(())
}

fn read_local_manifest(install_root: &Path) -> Result<EpsManifest> {
    let path = install_root.join("eps.toml");
    let raw = std::fs::read_to_string(&path)
        .with_context(|| format!("could not read '{}'", path.display()))?;
    toml::from_str(&raw).with_context(|| format!("failed to parse '{}'", path.display()))
}

pub fn run_hook(script: &str, cwd: &Path, pkg_name: &str, pkg_version: &str) -> Result<()> {
    println!("\x1b[2mRunning install hook: {script}\x1b[0m");
    let status = Command::new("sh")
        .arg(script)
        .current_dir(cwd)
        .env("EPM_PACKAGE_NAME", pkg_name)
        .env("EPM_PACKAGE_VERSION", pkg_version)
        .env("EPM_INSTALL_DIR", cwd)
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .with_context(|| format!("failed to execute hook '{script}'"))?;

    if !status.success() {
        bail!("install hook '{script}' exited with status {status}");
    }
    Ok(())
}

/// Return the current machine's Rust target triple, e.g. `"x86_64-apple-darwin"`.
fn current_target_triple() -> String {
    let arch = std::env::consts::ARCH;
    match std::env::consts::OS {
        "macos"   => format!("{arch}-apple-darwin"),
        "linux"   => format!("{arch}-unknown-linux-gnu"),
        "windows" => format!("{arch}-pc-windows-msvc"),
        os        => format!("{arch}-unknown-{os}"),
    }
}

/// Fail if the package's platform list doesn't include the current machine.
/// An empty list means "no restriction".
pub fn check_platform(platforms: &[String], name: &str) -> Result<()> {
    if platforms.is_empty() {
        return Ok(());
    }
    let current = current_target_triple();
    if platforms.iter().any(|p| p == &current) {
        Ok(())
    } else {
        anyhow::bail!(
            "'{name}' does not support your platform ({current}); supported: {}",
            platforms.join(", ")
        )
    }
}

/// Resolve a spec string to a version and install it.
pub async fn run(client: &RegistryClient, spec: &str) -> Result<()> {
    let (name, pinned_version) = parse_spec(spec);

    // Always fetch the full package so we can check platforms before cloning.
    let pkg = client.get_package(name).await?;
    check_platform(&pkg.platforms, name)?;

    let version: Version = if let Some(ver) = pinned_version {
        pkg.versions
            .into_iter()
            .find(|v| v.version == ver)
            .ok_or_else(|| anyhow::anyhow!("version '{ver}' of package '{name}' not found"))?
    } else {
        select_latest_version(pkg.versions)
            .ok_or_else(|| anyhow::anyhow!("no non-yanked versions available for '{name}'"))?
    };

    install_version(client, name, &version).await
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;
    use std::fs;
    use tempfile::TempDir;

    fn make_version(version: &str, yanked: bool) -> Version {
        Version {
            id: 1,
            package_id: 1,
            version: version.to_string(),
            git_url: "https://github.com/test/test".to_string(),
            commit_sha: "abc123".to_string(),
            manifest_hash: "def456".to_string(),
            yanked,
            published_at: "2025-01-01T00:00:00".to_string(),
            system_deps: HashMap::new(),
        }
    }

    // --- check_platform ---

    #[test]
    fn check_platform_empty_list_always_passes() {
        assert!(check_platform(&[], "pkg").is_ok());
    }

    #[test]
    fn check_platform_passes_when_current_platform_listed() {
        let current = current_target_triple();
        assert!(check_platform(&[current], "pkg").is_ok());
    }

    #[test]
    fn check_platform_fails_when_platform_not_listed() {
        // Use a platform that is definitely not the current one
        let other = if current_target_triple().contains("aarch64") {
            "x86_64-apple-darwin".to_string()
        } else {
            "aarch64-apple-darwin".to_string()
        };
        let err = check_platform(&[other], "mypkg").unwrap_err();
        assert!(err.to_string().contains("mypkg"));
        assert!(err.to_string().contains("does not support"));
    }

    #[test]
    fn check_platform_passes_when_one_of_multiple_matches() {
        let current = current_target_triple();
        let platforms = vec!["aarch64-apple-darwin".to_string(), "x86_64-apple-darwin".to_string()];
        // This should pass on either architecture
        let _ = current; // used implicitly via current_target_triple inside check_platform
        assert!(check_platform(&platforms, "pkg").is_ok());
    }

    // --- parse_spec ---

    #[test]
    fn parse_spec_name_only() {
        let (name, ver) = parse_spec("tech_talker");
        assert_eq!(name, "tech_talker");
        assert_eq!(ver, None);
    }

    #[test]
    fn parse_spec_with_version() {
        let (name, ver) = parse_spec("tech_talker@1.0.0");
        assert_eq!(name, "tech_talker");
        assert_eq!(ver, Some("1.0.0"));
    }

    #[test]
    fn parse_spec_multiple_at_signs_splits_on_first() {
        let (name, ver) = parse_spec("tech_talker@1.0.0@extra");
        assert_eq!(name, "tech_talker");
        assert_eq!(ver, Some("1.0.0@extra"));
    }

    #[test]
    fn parse_spec_empty_name_with_version() {
        let (name, ver) = parse_spec("@1.0.0");
        assert_eq!(name, "");
        assert_eq!(ver, Some("1.0.0"));
    }

    // --- select_latest_version ---

    #[test]
    fn select_latest_version_empty_returns_none() {
        assert!(select_latest_version(vec![]).is_none());
    }

    #[test]
    fn select_latest_version_single_non_yanked() {
        let v = make_version("1.0.0", false);
        let result = select_latest_version(vec![v]);
        assert!(result.is_some());
        assert_eq!(result.unwrap().version, "1.0.0");
    }

    #[test]
    fn select_latest_version_all_yanked_returns_none() {
        let versions = vec![make_version("1.0.0", true), make_version("1.1.0", true)];
        assert!(select_latest_version(versions).is_none());
    }

    #[test]
    fn select_latest_version_skips_yanked_picks_highest_available() {
        let versions = vec![
            make_version("1.2.0", true),
            make_version("1.1.0", false),
            make_version("1.0.0", false),
        ];
        let result = select_latest_version(versions);
        assert_eq!(result.unwrap().version, "1.1.0");
    }

    #[test]
    fn select_latest_version_picks_highest_when_both_available() {
        let versions = vec![make_version("1.1.0", false), make_version("1.0.0", false)];
        let result = select_latest_version(versions);
        assert_eq!(result.unwrap().version, "1.1.0");
    }

    #[test]
    fn select_latest_version_sorts_regardless_of_input_order() {
        // Deliberately provide versions oldest-first — must still return highest
        let versions = vec![
            make_version("0.1.0", false),
            make_version("0.3.0", false),
            make_version("0.2.0", false),
        ];
        let result = select_latest_version(versions);
        assert_eq!(result.unwrap().version, "0.3.0");
    }

    #[test]
    fn select_latest_version_handles_double_digit_minor() {
        // Alphabetic sort would give "0.9.0" > "0.10.0"; semver must give "0.10.0" > "0.9.0"
        let versions = vec![make_version("0.9.0", false), make_version("0.10.0", false)];
        let result = select_latest_version(versions);
        assert_eq!(result.unwrap().version, "0.10.0");
    }

    // --- run_hook ---

    #[test]
    fn run_hook_succeeds_and_sets_env_vars() {
        let dir = TempDir::new().unwrap();
        // Write a script that records the env vars to a file
        let out_file = dir.path().join("env_out.txt");
        let script_path = dir.path().join("check_env.sh");
        fs::write(
            &script_path,
            format!(
                "#!/bin/sh\necho \"$EPM_PACKAGE_NAME $EPM_PACKAGE_VERSION $EPM_INSTALL_DIR\" > '{}'\n",
                out_file.display()
            ),
        ).unwrap();

        run_hook("check_env.sh", dir.path(), "mypkg", "1.2.3").unwrap();

        let output = fs::read_to_string(&out_file).unwrap();
        let output = output.trim();
        assert!(output.contains("mypkg"), "EPM_PACKAGE_NAME missing: {output}");
        assert!(output.contains("1.2.3"), "EPM_PACKAGE_VERSION missing: {output}");
        assert!(output.contains(dir.path().to_str().unwrap()), "EPM_INSTALL_DIR missing: {output}");
    }

    #[test]
    fn run_hook_fails_on_nonzero_exit() {
        let dir = TempDir::new().unwrap();
        let script_path = dir.path().join("fail.sh");
        fs::write(&script_path, "#!/bin/sh\nexit 1\n").unwrap();

        let err = run_hook("fail.sh", dir.path(), "pkg", "0.1.0").unwrap_err();
        assert!(err.to_string().contains("install hook"), "unexpected error: {err}");
    }

    #[test]
    fn run_hook_skipped_when_manifest_has_no_install_hook() {
        let dir = TempDir::new().unwrap();
        fs::write(
            dir.path().join("eps.toml"),
            r#"[package]
name        = "nohook"
version     = "0.1.0"
description = "No hook"
authors     = ["nick"]
license     = "MIT"
repository  = "https://github.com/nick/nohook"
"#,
        ).unwrap();

        // read_local_manifest should succeed but hooks.install is None — no error
        let manifest = read_local_manifest(dir.path()).unwrap();
        assert!(manifest.hooks.install.is_none());
    }
}
