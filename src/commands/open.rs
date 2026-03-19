use anyhow::{bail, Result};

pub fn run(registry: &str, name: &str) -> Result<()> {
    let url = format!("{registry}/packages/{name}");

    #[cfg(target_os = "macos")]
    let status = std::process::Command::new("open").arg(&url).status();

    #[cfg(target_os = "linux")]
    let status = std::process::Command::new("xdg-open").arg(&url).status();

    #[cfg(target_os = "windows")]
    let status = std::process::Command::new("cmd")
        .args(["/c", "start", &url])
        .status();

    #[cfg(not(any(target_os = "macos", target_os = "linux", target_os = "windows")))]
    bail!("epm open is not supported on this platform");

    #[cfg(any(target_os = "macos", target_os = "linux", target_os = "windows"))]
    match status {
        Ok(s) if s.success() => {
            println!("Opening \x1b[36m{url}\x1b[0m");
            Ok(())
        }
        _ => bail!("failed to open browser — visit {url} manually"),
    }
}
