# epm installer for Windows (PowerShell)
# Usage: irm https://raw.githubusercontent.com/Slam-Dunk-Software/epm/main/install.ps1 | iex

$Repo = "https://github.com/Slam-Dunk-Software/epm"

if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
    Write-Host "epm requires Rust. Install it first:" -ForegroundColor Yellow
    Write-Host ""
    Write-Host "  https://rustup.rs"
    Write-Host ""
    exit 1
}

Write-Host "Installing epm from $Repo ..."
cargo install --git $Repo --quiet

Write-Host ""
Write-Host "epm installed. Try: epm new <harness>" -ForegroundColor Green
