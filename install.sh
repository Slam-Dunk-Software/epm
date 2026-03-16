#!/usr/bin/env sh
set -e

# epm installer — downloads a pre-built binary from GitHub Releases
# Usage: curl -fsSL https://raw.githubusercontent.com/Slam-Dunk-Software/epm/main/install.sh | sh

REPO="Slam-Dunk-Software/epm"
INSTALL_DIR="${EPM_INSTALL_DIR:-/usr/local/bin}"

# Detect OS and architecture
OS="$(uname -s)"
ARCH="$(uname -m)"

case "$OS" in
  Darwin)
    case "$ARCH" in
      arm64)  TARGET="aarch64-apple-darwin" ;;
      x86_64) TARGET="x86_64-apple-darwin" ;;
      *)      echo "Unsupported macOS architecture: $ARCH"; exit 1 ;;
    esac
    ;;
  Linux)
    case "$ARCH" in
      x86_64) TARGET="x86_64-unknown-linux-gnu" ;;
      *)      echo "Unsupported Linux architecture: $ARCH"; exit 1 ;;
    esac
    ;;
  *)
    echo "Unsupported OS: $OS"
    echo "Try installing from source: cargo install --git https://github.com/$REPO"
    exit 1
    ;;
esac

# Fetch the latest release tag
LATEST=$(curl -fsSL "https://api.github.com/repos/$REPO/releases/latest" \
  | grep '"tag_name"' \
  | sed 's/.*"tag_name": *"\([^"]*\)".*/\1/')

if [ -z "$LATEST" ]; then
  echo "Could not determine latest release. Check https://github.com/$REPO/releases"
  exit 1
fi

URL="https://github.com/$REPO/releases/download/$LATEST/epm-${TARGET}.tar.gz"

echo "Installing epm $LATEST for $TARGET..."

TMP="$(mktemp -d)"
trap 'rm -rf "$TMP"' EXIT

curl -fsSL "$URL" | tar -xz -C "$TMP"

# Install — try INSTALL_DIR directly, fall back to sudo
if [ -w "$INSTALL_DIR" ]; then
  mv "$TMP/epm" "$INSTALL_DIR/epm"
elif [ "$INSTALL_DIR" = "/usr/local/bin" ]; then
  echo "Installing to /usr/local/bin (may prompt for password)..."
  sudo mv "$TMP/epm" /usr/local/bin/epm
else
  mkdir -p "$INSTALL_DIR"
  mv "$TMP/epm" "$INSTALL_DIR/epm"
fi

chmod +x "$INSTALL_DIR/epm"

echo ""
echo "✓ epm $LATEST installed to $INSTALL_DIR/epm"
echo ""
echo "Try: epm new <harness>"
