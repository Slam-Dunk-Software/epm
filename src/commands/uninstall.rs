use std::fs;

use anyhow::{bail, Context, Result};

use crate::commands::install::parse_spec;
use crate::commands::list::list_installed_versions;

pub fn run(spec: &str) -> Result<()> {
    let (name, pinned_version) = parse_spec(spec);

    let pkg_root = dirs::home_dir()
        .context("could not determine home directory")?
        .join(".epm")
        .join("packages")
        .join(name);

    let version = if let Some(v) = pinned_version {
        v.to_string()
    } else {
        let mut installed = list_installed_versions(&pkg_root)?;
        match installed.len() {
            0 => bail!("{name} is not installed"),
            1 => installed.remove(0),
            _ => {
            let list = installed.join(", ");
            bail!("multiple versions installed: {list}\nUse `epm uninstall {name}@<version>` to target one.");
        }
        }
    };

    let target = pkg_root.join(&version);
    if !target.exists() {
        bail!("{name}@{version} is not installed");
    }

    fs::remove_dir_all(&target)?;
    println!("Uninstalled {name}@{version}");

    // Remove the package directory if it's now empty (last version removed)
    if pkg_root.read_dir().map(|mut d| d.next().is_none()).unwrap_or(false) {
        fs::remove_dir(&pkg_root)?;
    }

    Ok(())
}
