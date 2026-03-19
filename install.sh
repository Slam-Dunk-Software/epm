#!/usr/bin/env sh
set -e

# epm installer — downloads a pre-built binary from GitHub Releases
# Usage: curl -fsSL https://raw.githubusercontent.com/Slam-Dunk-Software/epm/main/install.sh | sh
# Options: --quiet / -q   skip post-install prompts

REPO="Slam-Dunk-Software/epm"
INSTALL_DIR="${EPM_INSTALL_DIR:-/usr/local/bin}"
QUIET=0

# Colors (disabled if not a terminal)
if [ -t 1 ]; then
  BOLD="\033[1m"
  GREEN="\033[32m"
  CYAN="\033[36m"
  YELLOW="\033[33m"
  RESET="\033[0m"
else
  BOLD="" GREEN="" CYAN="" YELLOW="" RESET=""
fi

for arg in "$@"; do
  case "$arg" in
    --quiet|-q) QUIET=1 ;;
  esac
done

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

printf "${BOLD}Installing epm $LATEST for $TARGET...${RESET}\n"

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
printf "${GREEN}✓ epm $LATEST installed to $INSTALL_DIR/epm${RESET}\n"
echo ""

# ── post-install setup ────────────────────────────────────────────────────────

if [ "$QUIET" = "1" ]; then
  exit 0
fi

# Read from /dev/tty so prompts work even when piped through curl
prompt() {
  printf "${CYAN}[?]${RESET} %s ${BOLD}[Y/n]${RESET} " "$1"
  read -r answer </dev/tty
  case "$answer" in
    [nN]*) return 1 ;;
    *)     return 0 ;;
  esac
}

printf "${BOLD}Would you like to set up a few extras?${RESET} (skip any with 'n')\n"

# EPC runtime (most important — required to deploy EPS services)
if prompt "  Install epc (process supervisor — runs your EPS services)?"; then
  echo ""
  epm runtime install epc
  echo ""
fi

# MCP server
if prompt "  Install eps_mcp (MCP knowledge server for Claude Code)?"; then
  echo ""
  epm mcp install eps_mcp
  echo ""
fi

# Skills
if prompt "  Install eps_skills (slash commands for Claude Code)?"; then
  echo ""
  epm skills install eps_skills
  echo ""
fi

printf "\n${GREEN}All done!${RESET} Run ${CYAN}epm new <harness>${RESET} to deploy your first service.\n"
printf "New to EPS? Start here: ${CYAN}https://epm.dev/docs/guides/getting-started${RESET}\n"
