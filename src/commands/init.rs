use std::path::Path;
use std::process::Command;

use anyhow::{bail, Context, Result};

/// Validate an EPS package name.
///
/// Rules: 2–64 characters, lowercase letters/digits/underscores, must start
/// with a letter. Matches what the registry enforces at publish time.
pub fn validate_name(name: &str) -> Result<()> {
    if name.len() < 2 || name.len() > 64 {
        bail!(
            "invalid package name '{}': must be 2–64 characters",
            name
        );
    }
    let mut chars = name.chars();
    let first = chars.next().unwrap();
    if !first.is_ascii_lowercase() {
        bail!(
            "invalid package name '{}': must start with a lowercase letter (e.g. 'my_pkg')",
            name
        );
    }
    if let Some(bad) = chars.find(|c| !matches!(c, 'a'..='z' | '0'..='9' | '_')) {
        bail!(
            "invalid package name '{}': character '{}' not allowed — use lowercase letters, digits, and underscores only",
            name, bad
        );
    }
    Ok(())
}

fn git_config(key: &str) -> Option<String> {
    Command::new("git")
        .args(["config", "--global", key])
        .output()
        .ok()
        .and_then(|o| {
            if o.status.success() {
                String::from_utf8(o.stdout)
                    .ok()
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
            } else {
                None
            }
        })
}

pub fn run(name: &str, description: Option<&str>, no_git: bool) -> Result<()> {
    validate_name(name)?;

    let dir = Path::new(name);
    if dir.exists() {
        bail!("'{}' already exists", name);
    }

    std::fs::create_dir(dir)
        .with_context(|| format!("failed to create directory '{name}'"))?;

    // Best-effort author from git config
    let author = match (git_config("user.name"), git_config("user.email")) {
        (Some(n), Some(e)) => format!("{n} <{e}>"),
        (Some(n), None) => n,
        _ => "your-name".to_string(),
    };

    let desc = description.unwrap_or("A deliberately incomplete harness — customize me.");

    let eps_toml = format!(
        r#"[package]
name = "{name}"
version = "0.1.0"
description = "{desc}"
authors = ["{author}"]
license = "MIT"
repository = "https://github.com/your-org/{name}"
platforms = []  # empty = no platform restriction; e.g. ["aarch64-apple-darwin"]

# Declare system dependencies required before install
# [system-dependencies]
# brew = ["cmake"]
# apt  = ["build-essential"]
"#
    );

    let customize_md = format!(
        r#"# {name} — Customization Guide

This is an [Extremely Personal Software (EPS)](https://epm.sh) harness. It is
**functional by default but deliberately incomplete** — the ports below are where
you make it yours.

## Ports

> Document your named extension points here. For each port: what it does, and
> how to customize it.

### `PORT_NAME`

**What it does:** Describe the extension point.
**How to customize:** What should the user edit, replace, or configure?

---

## Getting Started

```sh
# 1. Clone
git clone <your-repo-url>
cd {name}

# 2. Configure your ports (see above)

# 3. Run
./run.sh
```

## Philosophy

This harness ships with just enough to be useful. Everything else is a port.
"#
    );

    let run_sh = format!(
        r#"#!/usr/bin/env bash
set -euo pipefail

# {name}/run.sh — entry point for this EPS harness
#
# Edit this file to wire up your ports.
# See CUSTOMIZE.md for a description of each extension point.

# TODO: replace with your actual run command
echo "Running {name}..."
"#
    );

    std::fs::write(dir.join("eps.toml"), &eps_toml)
        .context("failed to write eps.toml")?;
    std::fs::write(dir.join("CUSTOMIZE.md"), &customize_md)
        .context("failed to write CUSTOMIZE.md")?;

    let run_sh_path = dir.join("run.sh");
    std::fs::write(&run_sh_path, &run_sh)
        .context("failed to write run.sh")?;

    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(&run_sh_path)
            .context("failed to read run.sh metadata")?
            .permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(&run_sh_path, perms)
            .context("failed to make run.sh executable")?;
    }

    if !no_git {
        let status = Command::new("git")
            .args(["init", name])
            .status()
            .context("failed to run git init")?;
        if !status.success() {
            bail!("git init failed");
        }
    }

    println!("\n\x1b[32m✓\x1b[0m Created EPS package \x1b[1m{name}\x1b[0m\n");
    println!("\x1b[2m  {name}/eps.toml");
    println!("  {name}/CUSTOMIZE.md");
    println!("  {name}/run.sh");
    if !no_git {
        println!("  {name}/.git/");
    }
    println!("\x1b[0m");
    println!("\x1b[2mNext steps:\x1b[0m");
    println!("  \x1b[36mcd {name}\x1b[0m");
    println!("  \x1b[2m# fill in eps.toml and CUSTOMIZE.md, then:\x1b[0m");
    println!("  \x1b[36mepm publish\x1b[0m");

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn valid_names_pass() {
        for name in &["ab", "my_pkg", "tech_talker", "pkg123", "a2b"] {
            assert!(validate_name(name).is_ok(), "expected '{name}' to be valid");
        }
    }

    #[test]
    fn name_too_short_fails() {
        assert!(validate_name("a").is_err());
        assert!(validate_name("").is_err());
    }

    #[test]
    fn name_too_long_fails() {
        let long = "a".repeat(65);
        assert!(validate_name(&long).is_err());
    }

    #[test]
    fn name_at_length_limits_passes() {
        assert!(validate_name("ab").is_ok());
        let at_max = "a".repeat(64);
        assert!(validate_name(&at_max).is_ok());
    }

    #[test]
    fn name_starting_with_digit_fails() {
        let err = validate_name("1pkg").unwrap_err();
        assert!(err.to_string().contains("lowercase letter"));
    }

    #[test]
    fn name_starting_with_underscore_fails() {
        assert!(validate_name("_pkg").is_err());
    }

    #[test]
    fn name_with_uppercase_fails() {
        let err = validate_name("MyPkg").unwrap_err();
        assert!(err.to_string().contains('M') || err.to_string().contains("not allowed"));
    }

    #[test]
    fn name_with_hyphen_fails() {
        let err = validate_name("my-pkg").unwrap_err();
        assert!(err.to_string().contains('-'));
    }

    #[test]
    fn name_with_space_fails() {
        assert!(validate_name("my pkg").is_err());
    }

    #[test]
    fn name_with_special_chars_fails() {
        for name in &["pkg!", "pkg@v", "pkg.js"] {
            assert!(validate_name(name).is_err(), "expected '{name}' to fail");
        }
    }

    #[test]
    fn error_message_includes_bad_character() {
        let err = validate_name("my-pkg").unwrap_err();
        assert!(err.to_string().contains('-'));
    }
}
