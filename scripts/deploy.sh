#!/bin/bash

# Stellar Teye - Contract Deployment Script
# Usage: ./scripts/deploy.sh <network> [contract]
# Networks: local, testnet, futurenet, mainnet

set -e

NETWORK=${1:-local}
CONTRACT=${2:-vision_records}

echo "🚀 Deploying $CONTRACT to $NETWORK..."

# Build the contract
echo "📦 Building contract..."
cargo build --target wasm32-unknown-unknown --release

WASM_PATH="target/wasm32-unknown-unknown/release/${CONTRACT}.wasm"

if [ ! -f "$WASM_PATH" ]; then
    echo "❌ WASM file not found: $WASM_PATH"
    exit 1
fi

# Deploy
echo "📤 Deploying to $NETWORK..."
CONTRACT_ID=$(soroban contract deploy \
    --wasm "$WASM_PATH" \
    --source default \
    --network "$NETWORK")

echo ""
echo "✅ Deployment successful!"
echo "📋 Contract ID: $CONTRACT_ID"
echo ""

# Emit a machine-readable line for CI parsing
echo "DEPLOYMENT_CONTRACT_ID=$CONTRACT_ID"

# Save deployment info
DEPLOY_DIR="deployments"
mkdir -p "$DEPLOY_DIR"

DEPLOY_FILE="$DEPLOY_DIR/${NETWORK}_${CONTRACT}.json"
cat > "$DEPLOY_FILE" << EOF
{
    "network": "$NETWORK",
    "contract": "$CONTRACT",
    "contract_id": "$CONTRACT_ID",
    "deployed_at": "$(date -u +"%Y-%m-%dT%H:%M:%SZ")",
    "wasm_hash": "$(sha256sum "$WASM_PATH" | cut -d' ' -f1)"
}
EOF

echo "💾 Deployment info saved to: $DEPLOY_FILE"
