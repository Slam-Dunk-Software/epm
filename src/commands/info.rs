use anyhow::Result;

use crate::client::RegistryClient;

pub async fn run(client: &RegistryClient, name: &str) -> Result<()> {
    let pkg = client.get_package(name).await?;

    println!("Name:        {}", pkg.name);
    println!("Description: {}", pkg.description);
    println!("License:     {}", pkg.license);
    println!("Repository:  {}", pkg.repository);
    if let Some(hp) = &pkg.homepage {
        println!("Homepage:    {hp}");
    }
    println!("Authors:     {}", pkg.authors.join(", "));
    println!("Platforms:   {}", pkg.platforms.join(", "));
    println!("Created:     {}", pkg.created_at);
    println!("Updated:     {}", pkg.updated_at);

    if pkg.versions.is_empty() {
        println!("\nNo published versions.");
    } else {
        println!("\nVersions:");
        for v in &pkg.versions {
            let yanked = if v.yanked { " [yanked]" } else { "" };
            println!("  {}{}  ({})", v.version, yanked, v.published_at);
            if !v.system_deps.is_empty() {
                let parts: Vec<String> = v
                    .system_deps
                    .iter()
                    .map(|(mgr, pkgs)| format!("{mgr}: {}", pkgs.join(", ")))
                    .collect();
                println!("    System deps:  {}", parts.join(" | "));
            }
        }
    }

    Ok(())
}
