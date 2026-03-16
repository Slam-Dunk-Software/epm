#!/usr/bin/env sh
set -e

# epm installer — requires Rust/Cargo
# Usage: curl -fsSL https://epm.dev/install.sh | sh

REPO="https://github.com/Slam-Dunk-Software/epm"

if ! command -v cargo >/dev/null 2>&1; then
  echo "epm requires Rust. Install it first:"
  echo ""
  echo "  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh"
  echo ""
  exit 1
fi

echo "Installing epm from $REPO ..."
cargo install --git "$REPO" --quiet

echo ""
echo "✓ epm installed. Try: epm new <harness>"
