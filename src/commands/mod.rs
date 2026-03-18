pub mod adopt;
pub mod open;
pub mod self_uninstall;
pub mod self_update;
pub mod mcp;
pub mod skills;
pub mod new;
pub mod runtime;
pub mod info;
pub mod init;
pub mod install;
pub mod list;
pub mod publish;
pub mod search;
pub mod sync;
pub mod sysdeps;
pub mod uninstall;
pub mod upgrade;

/// Print an EPM Core redirect message and exit.
///
/// Called whenever a command that doesn't apply to EPM Core packages (install,
/// new, adopt, upgrade, etc.) is invoked against one. The registry is the
/// source of truth for `package_type = "epm_core"`.
pub fn guard_epm_core(name: &str) -> ! {
    eprintln!(
        "\x1b[33m⚠\x1b[0m  \x1b[1m{name}\x1b[0m is an \x1b[1mEPM Core\x1b[0m package — not a regular harness.\n"
    );
    eprintln!("   Manage it with:");
    eprintln!("     \x1b[36mepm runtime status\x1b[0m");
    eprintln!("     \x1b[36mepm runtime install {name}\x1b[0m");
    eprintln!("     \x1b[36mepm runtime upgrade {name}\x1b[0m");
    std::process::exit(1);
}
