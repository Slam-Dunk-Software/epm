use std::process::Command;

use anyhow::{bail, Result};

use crate::models::SystemDeps;

pub fn check_system_deps(deps: &SystemDeps) -> Result<()> {
    if deps.is_empty() {
        return Ok(());
    }

    let mut missing: Vec<String> = vec![];
    for pkg in deps.get("brew").map(|v| v.as_slice()).unwrap_or(&[]) {
        if !is_brew_installed(pkg) {
            missing.push(format!("brew install {pkg}"));
        }
    }
    for pkg in deps.get("cargo").map(|v| v.as_slice()).unwrap_or(&[]) {
        if !is_binary_in_path(pkg) {
            missing.push(format!("cargo install {pkg}"));
        }
    }
    for pkg in deps.get("gem").map(|v| v.as_slice()).unwrap_or(&[]) {
        if !is_gem_installed(pkg) {
            missing.push(format!("gem install {pkg}"));
        }
    }
    if missing.is_empty() {
        return Ok(());
    }

    let cmds = missing.join("\n  ");
    bail!("missing system dependencies — run:\n  {cmds}");
}

fn is_brew_installed(pkg: &str) -> bool {
    Command::new("brew")
        .args(["list", "--formula", pkg])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
        || Command::new("brew")
            .args(["list", "--cask", pkg])
            .status()
            .map(|s| s.success())
            .unwrap_or(false)
}

fn is_binary_in_path(bin: &str) -> bool {
    Command::new("which")
        .arg(bin)
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

fn is_gem_installed(gem: &str) -> bool {
    Command::new("gem")
        .args(["list", "-i", gem])
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn deps(pairs: &[(&str, &[&str])]) -> SystemDeps {
        pairs
            .iter()
            .map(|(mgr, pkgs)| (mgr.to_string(), pkgs.iter().map(|p| p.to_string()).collect()))
            .collect()
    }

    #[test]
    fn empty_deps_ok() {
        assert!(check_system_deps(&HashMap::new()).is_ok());
    }

    #[test]
    fn known_binary_in_path() {
        assert!(is_binary_in_path("git"));
    }

    #[test]
    fn unknown_binary_not_in_path() {
        assert!(!is_binary_in_path("epm_nonexistent_xyz_456"));
    }

    #[test]
    fn cargo_dep_satisfied_when_binary_in_path() {
        assert!(check_system_deps(&deps(&[("cargo", &["git"])])).is_ok());
    }

    #[test]
    fn cargo_dep_missing_returns_err() {
        let r = check_system_deps(&deps(&[("cargo", &["epm_nonexistent_xyz_456"])]));
        assert!(r.is_err());
        let msg = r.unwrap_err().to_string();
        assert!(msg.contains("missing system dependencies"));
        assert!(msg.contains("cargo install"));
    }
}
