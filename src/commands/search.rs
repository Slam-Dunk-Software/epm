use anyhow::Result;

use crate::client::RegistryClient;

pub async fn run(client: &RegistryClient, query: Option<&str>) -> Result<()> {
    let packages = client.list_packages().await?;

    let filtered: Vec<_> = match query {
        Some(q) => {
            let q = q.to_lowercase();
            packages
                .iter()
                .filter(|p| {
                    p.name.to_lowercase().contains(&q)
                        || p.description.to_lowercase().contains(&q)
                })
                .collect()
        }
        None => packages.iter().collect(),
    };

    if filtered.is_empty() {
        if let Some(q) = query {
            println!("No packages matching \x1b[1m'{q}'\x1b[0m.");
        } else {
            println!("No packages in registry.");
        }
        return Ok(());
    }

    let name_width = filtered.iter().map(|p| p.name.len()).max().unwrap_or(4);
    println!("\x1b[2m{:<name_width$}  {}\x1b[0m", "NAME", "DESCRIPTION", name_width = name_width);
    println!("\x1b[2m{}\x1b[0m", "─".repeat(name_width + 2 + 50));

    for pkg in filtered {
        println!("\x1b[1m{:<name_width$}\x1b[0m  \x1b[2m{}\x1b[0m", pkg.name, pkg.description, name_width = name_width);
    }

    Ok(())
}
