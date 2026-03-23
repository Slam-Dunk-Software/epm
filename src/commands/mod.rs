pub mod adopt;
pub mod services;
pub mod login;
pub mod open;
pub mod self_uninstall;
pub mod self_update;
pub mod skills;
pub mod new;
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

/// Print a redirect message for packages that are harnesses, not installable binaries.
pub fn guard_epm_core(name: &str) -> ! {
    eprintln!(
        "\x1b[33m⚠\x1b[0m  \x1b[1m{name}\x1b[0m is a harness — scaffold it with:\n"
    );
    eprintln!("     \x1b[36mepm new {name}\x1b[0m");
    std::process::exit(1);
}
