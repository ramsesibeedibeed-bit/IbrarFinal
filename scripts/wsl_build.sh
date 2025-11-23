#!/usr/bin/env bash
set -euo pipefail

echo "Starting WSL Anchor build script..."
REPO="/mnt/c/Users/Tiwaloluwa/Desktop/realyozoonsmartcontract/sol-token-mill-interface"
cd "$REPO"

echo "Working directory: $(pwd)"

export PATH="$HOME/.cargo/bin:$HOME/.local/share/solana/install/active_release/bin:$PATH"

# Install system build deps if clang is missing
if ! command -v clang >/dev/null 2>&1; then
  echo "Installing system packages (sudo required)..."
  sudo apt-get update -y
  sudo apt-get install -y build-essential curl git clang llvm libssl-dev pkg-config libclang-dev make python3 ca-certificates
fi

# Install rustup if needed
if ! command -v rustc >/dev/null 2>&1; then
  echo "Installing rustup..."
  curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
  source "$HOME/.cargo/env"
fi

source "$HOME/.cargo/env" || true

# Install Solana CLI if needed
if ! command -v solana >/dev/null 2>&1; then
  echo "Installing Solana CLI v1.18.26..."
  curl -sSfL https://release.solana.com/v1.18.26/install | sh -s
  export PATH="$HOME/.local/share/solana/install/active_release/bin:$PATH"
fi

# Install anchor-cli v0.30.1 if needed
if ! command -v anchor >/dev/null 2>&1; then
  echo "Installing anchor-cli v0.30.1 via cargo..."
  cargo install --locked --git https://github.com/coral-xyz/anchor --tag v0.30.1 anchor-cli || true
fi

echo "Tool versions:"
rustc --version || true
cargo --version || true
solana --version || true
anchor --version || true

# Fetch crates and build
echo "Fetching crates..."
cargo fetch || true

echo "Running anchor build --skip-lint (this may take several minutes)..."
anchor build --skip-lint 2>&1 | tee build_wsl_output.txt

echo "Build finished. Output saved to build_wsl_output.txt"
