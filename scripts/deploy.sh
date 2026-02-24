#!/bin/bash

# Stellar Teye - Contract Deployment Script
# Usage: ./scripts/deploy.sh <network> [contract] [--admin <address>]
#
# Networks: local, testnet, futurenet, mainnet
#
# Options:
#   --admin <address>   Permanent admin address. When provided, the script
#                       initializes the contract with the deployer as a
#                       temporary admin, then transfers admin rights to this
#                       address (least-privilege deployment).
#
# Without --admin the contract is deployed but NOT initialized. The caller
# must initialize and transfer admin separately.

set -e

NETWORK=${1:-local}
CONTRACT=${2:-vision_records}
ADMIN_ADDRESS=""

# Parse optional --admin flag
shift 2 2>/dev/null || true
while [ "$#" -gt 0 ]; do
    case "$1" in
        --admin)
            ADMIN_ADDRESS="$2"
            shift 2
            ;;
        *)
            echo "Unknown option: $1"
            exit 1
            ;;
    esac
done

echo "Deploying $CONTRACT to $NETWORK..."

# Build the contract
echo "Building contract..."
cargo build --target wasm32-unknown-unknown --release

WASM_PATH="target/wasm32-unknown-unknown/release/${CONTRACT}.wasm"

if [ ! -f "$WASM_PATH" ]; then
    echo "WASM file not found: $WASM_PATH"
    exit 1
fi

# Deploy
echo "Deploying to $NETWORK..."
CONTRACT_ID=$(soroban contract deploy \
    --wasm "$WASM_PATH" \
    --source default \
    --network "$NETWORK")

echo "Contract deployed: $CONTRACT_ID"

# ---- Secure initialization with admin transfer ----
if [ -n "$ADMIN_ADDRESS" ]; then
    # Resolve the deployer's public key for initialization
    DEPLOYER_ADDRESS=$(soroban keys address default 2>/dev/null || echo "")
    if [ -z "$DEPLOYER_ADDRESS" ]; then
        echo "ERROR: Could not resolve deployer address."
        echo "The contract was deployed but NOT initialized."
        exit 1
    fi

    echo "Initializing contract with deployer as temporary admin..."
    soroban contract invoke \
        --id "$CONTRACT_ID" \
        --source default \
        --network "$NETWORK" \
        -- \
        initialize \
        --admin "$DEPLOYER_ADDRESS"

    echo "Transferring admin to permanent address: $ADMIN_ADDRESS"
    soroban contract invoke \
        --id "$CONTRACT_ID" \
        --source default \
        --network "$NETWORK" \
        -- \
        transfer_admin \
        --current_admin "$DEPLOYER_ADDRESS" \
        --new_admin "$ADMIN_ADDRESS"

    # Verify the transfer succeeded
    VERIFIED_ADMIN=$(soroban contract invoke \
        --id "$CONTRACT_ID" \
        --source default \
        --network "$NETWORK" \
        -- \
        get_admin 2>/dev/null || echo "")

    if echo "$VERIFIED_ADMIN" | grep -q "$ADMIN_ADDRESS"; then
        echo "Admin transfer verified."
    else
        echo "WARNING: Could not verify admin transfer."
        echo "Verify manually: soroban contract invoke --id $CONTRACT_ID -- get_admin"
    fi
else
    echo "NOTE: No --admin flag provided. Contract deployed but NOT initialized."
    echo "Run initialization and admin transfer separately."
    echo "See docs/deployment-security.md for the secure deployment procedure."
fi

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
    "admin": "$ADMIN_ADDRESS",
    "deployed_at": "$(date -u +"%Y-%m-%dT%H:%M:%SZ")",
    "wasm_hash": "$(sha256sum "$WASM_PATH" | cut -d' ' -f1)",
    "admin_transferred": $([ -n "$ADMIN_ADDRESS" ] && echo "true" || echo "false")
}
EOF

echo "Deployment info saved to: $DEPLOY_FILE"
