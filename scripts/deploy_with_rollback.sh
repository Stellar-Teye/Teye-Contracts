#!/usr/bin/env bash

# Safe deploy wrapper with descriptor rollback.
# Usage: ./scripts/deploy_with_rollback.sh <network> [contract] [--admin <address>] [--verify-fn <fn>]

set -euo pipefail

NETWORK="${1:-testnet}"
CONTRACT="${2:-vision_records}"
ADMIN_ADDRESS=""
VERIFY_FN=""

# Consume positional args.
shift $(( $# >= 1 ? 1 : 0 )) || true
shift $(( $# >= 1 ? 1 : 0 )) || true

while [[ $# -gt 0 ]]; do
  case "$1" in
    --admin)
      ADMIN_ADDRESS="$2"
      shift 2
      ;;
    --verify-fn)
      VERIFY_FN="$2"
      shift 2
      ;;
    *)
      echo "Unknown option: $1"
      exit 1
      ;;
  esac
done

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DEPLOY_DIR="$ROOT_DIR/deployments"
DEPLOY_FILE="$DEPLOY_DIR/${NETWORK}_${CONTRACT}.json"
BACKUP_FILE="$DEPLOY_DIR/${NETWORK}_${CONTRACT}_previous.json"

mkdir -p "$DEPLOY_DIR"

restore_descriptor() {
  if [[ -f "$BACKUP_FILE" ]]; then
    cp "$BACKUP_FILE" "$DEPLOY_FILE"
    echo "Rolled back deployment descriptor to previous known-good file."
  else
    rm -f "$DEPLOY_FILE"
    echo "No previous descriptor found; removed current descriptor."
  fi
}

if [[ -f "$DEPLOY_FILE" ]]; then
  cp "$DEPLOY_FILE" "$BACKUP_FILE"
  echo "Created descriptor backup: $BACKUP_FILE"
fi

DEPLOY_CMD=("$ROOT_DIR/scripts/deploy.sh" "$NETWORK" "$CONTRACT")
if [[ -n "$ADMIN_ADDRESS" ]]; then
  DEPLOY_CMD+=("--admin" "$ADMIN_ADDRESS")
fi

echo "Running deployment: ${DEPLOY_CMD[*]}"
set +e
DEPLOY_OUTPUT="$(${DEPLOY_CMD[@]} 2>&1)"
DEPLOY_EXIT=$?
set -e
echo "$DEPLOY_OUTPUT"

if [[ $DEPLOY_EXIT -ne 0 ]]; then
  echo "Deployment failed; restoring descriptor backup."
  restore_descriptor
  exit $DEPLOY_EXIT
fi

CONTRACT_ID="$(echo "$DEPLOY_OUTPUT" | sed -n 's/^DEPLOYMENT_CONTRACT_ID=//p' | tail -n 1)"
if [[ -z "$CONTRACT_ID" ]]; then
  echo "Could not parse DEPLOYMENT_CONTRACT_ID; restoring descriptor backup."
  restore_descriptor
  exit 1
fi

echo "Deployment succeeded: $CONTRACT_ID"

verify_succeeded=0

if [[ -n "$VERIFY_FN" ]]; then
  if soroban contract invoke --id "$CONTRACT_ID" --source default --network "$NETWORK" -- --fn "$VERIFY_FN" >/dev/null 2>&1; then
    verify_succeeded=1
  fi
else
  # Try a few common read-only probes.
  if soroban contract invoke --id "$CONTRACT_ID" --source default --network "$NETWORK" -- --fn is_initialized >/dev/null 2>&1; then
    verify_succeeded=1
  elif soroban contract invoke --id "$CONTRACT_ID" --source default --network "$NETWORK" -- --fn version >/dev/null 2>&1; then
    verify_succeeded=1
  elif soroban contract invoke --id "$CONTRACT_ID" --source default --network "$NETWORK" -- --fn get_admin >/dev/null 2>&1; then
    verify_succeeded=1
  fi
fi

if [[ "$verify_succeeded" -ne 1 ]]; then
  echo "Verification failed for $CONTRACT ($CONTRACT_ID); restoring descriptor backup."
  restore_descriptor
  exit 1
fi

if [[ -n "$ADMIN_ADDRESS" ]]; then
  set +e
  VERIFIED_ADMIN="$(soroban contract invoke --id "$CONTRACT_ID" --source default --network "$NETWORK" -- --fn get_admin 2>/dev/null)"
  set -e
  if [[ -n "$VERIFIED_ADMIN" ]] && ! echo "$VERIFIED_ADMIN" | grep -q "$ADMIN_ADDRESS"; then
    echo "Admin verification mismatch; restoring descriptor backup."
    restore_descriptor
    exit 1
  fi
fi

echo "Verification succeeded: $CONTRACT_ID"
echo "DEPLOYMENT_CONTRACT_ID=$CONTRACT_ID"
