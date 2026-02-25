#!/bin/bash

# Stellar Teye - Testnet Deployment Orchestrator
# Usage: ./scripts/deploy_testnet.sh [contract]
# Contract defaults to "vision_records"

set -euo pipefail

NETWORK="testnet"
CONTRACT="${1:-vision_records}"

echo "🚀 Orchestrating deployment of '$CONTRACT' to '$NETWORK'..."

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEPLOY_DIR="$ROOT_DIR/deployments"
CURRENT_FILE="$DEPLOY_DIR/${NETWORK}_${CONTRACT}.json"
PREVIOUS_FILE="$DEPLOY_DIR/${NETWORK}_${CONTRACT}_previous.json"

mkdir -p "$DEPLOY_DIR"

if [ -f "$CURRENT_FILE" ]; then
  echo "📂 Backing up current deployment file to: $PREVIOUS_FILE"
  cp "$CURRENT_FILE" "$PREVIOUS_FILE"
fi

echo "📤 Running base deployment script..."
DEPLOY_OUTPUT="$("$ROOT_DIR/scripts/deploy.sh" "$NETWORK" "$CONTRACT")"
echo "$DEPLOY_OUTPUT"

CONTRACT_ID=$(echo "$DEPLOY_OUTPUT" | sed -n 's/^DEPLOYMENT_CONTRACT_ID=//p')

if [ -z "$CONTRACT_ID" ]; then
  echo "❌ Failed to parse contract ID from deployment output."
  exit 1
fi

echo "🔍 Verifying deployment on network '$NETWORK' (contract ID: $CONTRACT_ID)..."

# Basic verification: for known contracts, invoke a simple, side-effect-free method
VERIFY_OK=0
case "$CONTRACT" in
  vision_records)
    if soroban contract invoke \
      --id "$CONTRACT_ID" \
      --source default \
      --network "$NETWORK" \
      --fn version >/dev/null 2>&1; then
      VERIFY_OK=1
    fi
    ;;
  *)
    echo "ℹ️ No contract-specific verification implemented for '$CONTRACT'. Skipping invoke check."
    VERIFY_OK=1
    ;;
esac

if [ "$VERIFY_OK" -ne 1 ]; then
  echo "❌ Deployment verification failed for contract '$CONTRACT' (ID: $CONTRACT_ID)."
  if [ -f "$PREVIOUS_FILE" ]; then
    echo "↩️ Restoring previous deployment descriptor from backup."
    cp "$PREVIOUS_FILE" "$CURRENT_FILE"
  fi
  exit 1
fi

echo "✅ Deployment verified successfully."
echo "VERIFIED_CONTRACT_ID=$CONTRACT_ID"
