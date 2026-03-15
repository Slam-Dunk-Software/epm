use std::fs::read_dir;
use std::path::Path;

use anyhow::{Context, Result};

/// Returns sorted list of installed version directory names under `pkg_root`.
pub fn list_installed_versions(pkg_root: &Path) -> Result<Vec<String>> {
    if !pkg_root.exists() {
        return Ok(vec![]);
    }
    let mut versions = vec![];
    for entry in read_dir(pkg_root).context("failed to read package directory")? {
        let entry = entry?;
        if entry.file_type()?.is_dir() {
            versions.push(entry.file_name().to_string_lossy().to_string());
        }
    }
    versions.sort();
    Ok(versions)
}

pub fn run() -> Result<()> {
    let packages_dir = dirs::home_dir()
        .context("could not determine home directory")?
        .join(".epm")
        .join("packages");

    if !packages_dir.exists() {
        println!("No packages installed.");
        return Ok(());
    }

    let mut entries: Vec<String> = vec![];
    for pkg in read_dir(&packages_dir).context("failed to read packages directory")? {
        let pkg = pkg?;
        if !pkg.file_type()?.is_dir() {
            continue;
        }
        for ver in read_dir(pkg.path()).context("failed to read package directory")? {
            let ver = ver?;
            if !ver.file_type()?.is_dir() {
                continue;
            }
            entries.push(format!(
                "{}@{}",
                pkg.file_name().to_string_lossy(),
                ver.file_name().to_string_lossy()
            ));
        }
    }

    entries.sort();
    if entries.is_empty() {
        println!("No packages installed.");
    } else {
        for e in &entries {
            println!("{e}");
        }
    }

    Ok(())
}
