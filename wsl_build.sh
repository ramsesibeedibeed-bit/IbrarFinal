#!/usr/bin/env bash
set -euo pipefail

echo "Running inside WSL: $(uname -a)"
REPO="/mnt/c/Users/Tiwaloluwa/Desktop/realyozoonsmartcontract/sol-token-mill-interface"
if [ ! -d "$REPO" ]; then
  echo "Repository path not found inside WSL: $REPO"
  exit 2
fi
cd "$REPO"
echo "Working dir: $(pwd)"

if ! command -v rustc >/dev/null 2>&1; then
  echo "Installing rustup..."
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
  source "$HOME/.cargo/env"
fi

source "$HOME/.cargo/env" || true
rustc --version || true

if ! command -v solana >/dev/null 2>&1; then
  echo "Installing solana v1.18.26..."
  sh -c "$(curl -sSfL https://release.solana.com/v1.18.26/install)"
fi

if ! command -v anchor >/dev/null 2>&1; then
  echo "Installing anchor-cli v0.30.1..."
  cargo install --locked --git https://github.com/coral-xyz/anchor --tag v0.30.1 anchor-cli || true
fi

echo "Tool versions:"
rustc --version || true
cargo --version || true
solana --version || true
anchor --version || true

echo "Fetching crates..."
cargo fetch || true

echo "Running anchor build --skip-lint"
anchor build --skip-lint
