#!/bin/bash

# Stellar Teye - Automated Setup Script
# This script sets up the development environment for the Teye-Contracts project

set -e

echo "🌟 Stellar Teye - Development Environment Setup"
echo "================================================"

# Colors for output
RED='\033[0;31m'
GREEN='\033[0;32m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Function to print colored output
print_status() {
    echo -e "${GREEN}✓${NC} $1"
}

print_warning() {
    echo -e "${YELLOW}⚠${NC} $1"
}

print_error() {
    echo -e "${RED}✗${NC} $1"
}

# Check if Rust is installed
echo ""
echo "📦 Checking Rust installation..."
if command -v rustc &> /dev/null; then
    RUST_VERSION=$(rustc --version)
    print_status "Rust is installed: $RUST_VERSION"
else
    print_warning "Rust not found. Installing..."
    curl --proto '=https' --tlsv1.2 -sSf https://sh.rustup.rs | sh -s -- -y
    source "$HOME/.cargo/env"
    print_status "Rust installed successfully"
fi

# Add WASM target
echo ""
echo "🎯 Adding WebAssembly target..."
rustup target add wasm32-unknown-unknown
print_status "WASM target added"

# Add components
echo ""
echo "🔧 Adding Rust components..."
rustup component add rustfmt clippy rust-src
print_status "Components added (rustfmt, clippy, rust-src)"

# Install Soroban CLI
echo ""
echo "🚀 Installing Soroban CLI..."
if command -v soroban &> /dev/null; then
    SOROBAN_VERSION=$(soroban version)
    print_status "Soroban CLI already installed: $SOROBAN_VERSION"
else
    cargo install --locked soroban-cli
    print_status "Soroban CLI installed"
fi

# Generate default identity
echo ""
echo "🔑 Setting up Soroban identity..."
if soroban config identity show default &> /dev/null; then
    print_status "Default identity already exists"
else
    soroban config identity generate default
    print_status "Generated new default identity"
fi

# Configure networks
echo ""
echo "🌐 Configuring networks..."

# Local network
soroban config network add local \
    --rpc-url http://localhost:8000/soroban/rpc \
    --network-passphrase "Standalone Network ; February 2017" \
    2>/dev/null || true
print_status "Local network configured"

# Testnet
soroban config network add testnet \
    --rpc-url https://soroban-testnet.stellar.org:443 \
    --network-passphrase "Test SDF Network ; September 2015" \
    2>/dev/null || true
print_status "Testnet configured"

# Futurenet
soroban config network add futurenet \
    --rpc-url https://rpc-futurenet.stellar.org:443 \
    --network-passphrase "Test SDF Future Network ; October 2022" \
    2>/dev/null || true
print_status "Futurenet configured"

# Build project
echo ""
echo "🔨 Building project..."
cargo build --all-targets
print_status "Project built successfully"

# Run tests
echo ""
echo "🧪 Running tests..."
cargo test --all
print_status "All tests passed"

# Summary
echo ""
echo "================================================"
echo "🎉 Setup Complete!"
echo "================================================"
echo ""
echo "Next steps:"
echo "  1. Start local network:  make start-local"
echo "  2. Deploy contracts:     make deploy-local"
echo "  3. Run tests:           make test"
echo ""
echo "Happy coding! 👁️"
