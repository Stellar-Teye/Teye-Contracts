#!/usr/bin/env bash
# scripts/migrate.sh
# ─────────────────────────────────────────────────────────────────────────────
# Contract Upgrade Migration Script for Stellar Teye / Teye-Contracts
#
# Usage:
#   ./scripts/migrate.sh [COMMAND] [OPTIONS]
#
# Commands:
#   status                    Show current on-chain schema version
#   dry-run  --to <version>   Validate migration without committing
#   forward  --to <version>   Migrate forward to target version
#   rollback --to <version>   Roll back to target version
#   canary   --pct <0-100> --version <v>  Set canary traffic split
#   canary-off                Disable canary (route 100% to stable)
#   help                      Show this help message
#
# Options:
#   --network   <local|testnet|futurenet>   Target Stellar network  [default: local]
#   --identity  <name>                      Soroban identity to use  [default: default]
#   --contract  <contract-id>               Override contract ID from env
#
# Environment variables:
#   TEYE_CONTRACT_ID   On-chain contract address (required if not passed via --contract)
#   SOROBAN_RPC_URL    Override RPC endpoint
# ─────────────────────────────────────────────────────────────────────────────

set -euo pipefail

# ── Colour helpers ────────────────────────────────────────────────────────────
RED='\033[0;31m'; GREEN='\033[0;32m'; YELLOW='\033[1;33m'
CYAN='\033[0;36m'; BOLD='\033[1m'; RESET='\033[0m'

info()    { echo -e "${CYAN}[INFO]${RESET}  $*"; }
success() { echo -e "${GREEN}[OK]${RESET}    $*"; }
warn()    { echo -e "${YELLOW}[WARN]${RESET}  $*"; }
error()   { echo -e "${RED}[ERROR]${RESET} $*" >&2; exit 1; }
header()  { echo -e "\n${BOLD}${CYAN}=== $* ===${RESET}"; }

# ── Defaults ──────────────────────────────────────────────────────────────────
NETWORK="${NETWORK:-local}"
IDENTITY="${IDENTITY:-default}"
CONTRACT_ID="${TEYE_CONTRACT_ID:-}"
COMMAND=""
TARGET_VERSION=""
CANARY_PCT=""
CANARY_VERSION=""

# ── Argument parsing ──────────────────────────────────────────────────────────
while [[ $# -gt 0 ]]; do
    case "$1" in
        status|dry-run|forward|rollback|canary|canary-off|help)
            COMMAND="$1"; shift ;;
        --network)    NETWORK="$2";        shift 2 ;;
        --identity)   IDENTITY="$2";       shift 2 ;;
        --contract)   CONTRACT_ID="$2";    shift 2 ;;
        --to)         TARGET_VERSION="$2"; shift 2 ;;
        --pct)        CANARY_PCT="$2";     shift 2 ;;
        --version)    CANARY_VERSION="$2"; shift 2 ;;
        *) error "Unknown argument: $1" ;;
    esac
done

# ── Validation helpers ────────────────────────────────────────────────────────
require_contract() {
    [[ -n "$CONTRACT_ID" ]] || \
        error "Contract ID not set. Export TEYE_CONTRACT_ID or pass --contract."
}

require_target_version() {
    [[ -n "$TARGET_VERSION" ]] || \
        error "Target version not specified. Use --to <version>."
    [[ "$TARGET_VERSION" =~ ^[0-9]+$ ]] || \
        error "--to must be a positive integer, got: $TARGET_VERSION"
}

require_soroban() {
    command -v soroban &>/dev/null || \
        error "soroban CLI not found. Install it with: cargo install soroban-cli"
}

# ── Network RPC map ───────────────────────────────────────────────────────────
rpc_for_network() {
    case "$1" in
        local)      echo "http://localhost:8000/soroban/rpc" ;;
        testnet)    echo "https://soroban-testnet.stellar.org:443" ;;
        futurenet)  echo "https://rpc-futurenet.stellar.org:443" ;;
        *)          error "Unknown network: $1. Use local | testnet | futurenet" ;;
    esac
}

RPC_URL="${SOROBAN_RPC_URL:-$(rpc_for_network "$NETWORK")}"

# ── Soroban invoke wrapper ────────────────────────────────────────────────────
soroban_invoke() {
    local fn_name="$1"; shift
    soroban contract invoke \
        --id          "$CONTRACT_ID" \
        --network     "$NETWORK" \
        --source      "$IDENTITY" \
        --rpc-url     "$RPC_URL" \
        -- "$fn_name" "$@"
}

# ─────────────────────────────────────────────────────────────────────────────
# Commands
# ─────────────────────────────────────────────────────────────────────────────

cmd_status() {
    require_soroban
    require_contract
    header "Schema Version Status"
    info "Network   : $NETWORK"
    info "Contract  : $CONTRACT_ID"
    local ver
    ver=$(soroban_invoke stored_version 2>&1) || \
        error "Failed to query contract: $ver"
    success "Current on-chain schema version: ${BOLD}v${ver}${RESET}"
}

cmd_dry_run() {
    require_soroban
    require_contract
    require_target_version
    header "Dry-Run Migration to v${TARGET_VERSION}"
    info "Validating migration without committing changes..."
    local result
    result=$(soroban_invoke dry_run_migration \
        --target_version "$TARGET_VERSION" 2>&1) || {
        error "Dry-run failed: $result"
    }
    success "Dry-run passed — migration to v${TARGET_VERSION} is safe."
    echo "$result"
}

cmd_forward() {
    require_soroban
    require_contract
    require_target_version
    header "Forward Migration → v${TARGET_VERSION}"

    # Step 1: dry-run first
    info "Running pre-migration validation (dry-run)..."
    soroban_invoke dry_run_migration \
        --target_version "$TARGET_VERSION" &>/dev/null || {
        error "Pre-migration validation FAILED. Aborting upgrade."
    }
    success "Validation passed."

    # Step 2: confirm
    warn "This will migrate on-chain data on ${NETWORK}."
    read -rp "  Type 'yes' to proceed: " CONFIRM
    [[ "$CONFIRM" == "yes" ]] || { info "Aborted."; exit 0; }

    # Step 3: apply
    info "Applying migration..."
    local result
    result=$(soroban_invoke migrate_forward \
        --target_version "$TARGET_VERSION" 2>&1) || {
        error "Migration FAILED: $result"
    }
    success "Migration to v${TARGET_VERSION} complete."
    echo "$result"
}

cmd_rollback() {
    require_soroban
    require_contract
    require_target_version
    header "Rollback → v${TARGET_VERSION}"

    warn "Rolling back on-chain data on ${NETWORK} to v${TARGET_VERSION}."
    read -rp "  Type 'rollback' to confirm: " CONFIRM
    [[ "$CONFIRM" == "rollback" ]] || { info "Aborted."; exit 0; }

    info "Applying rollback..."
    local result
    result=$(soroban_invoke migrate_rollback \
        --target_version "$TARGET_VERSION" 2>&1) || {
        error "Rollback FAILED: $result"
    }
    success "Rollback to v${TARGET_VERSION} complete."
    echo "$result"
}

cmd_canary() {
    require_soroban
    require_contract
    [[ -n "$CANARY_PCT" && -n "$CANARY_VERSION" ]] || \
        error "Canary requires --pct <0-100> and --version <v>"
    [[ "$CANARY_PCT" =~ ^[0-9]+$ && "$CANARY_PCT" -le 100 ]] || \
        error "--pct must be 0-100, got: $CANARY_PCT"

    header "Canary Deployment: ${CANARY_PCT}% → v${CANARY_VERSION}"
    info "Setting canary split on $NETWORK..."
    soroban_invoke set_canary \
        --percentage    "$CANARY_PCT" \
        --new_version   "$CANARY_VERSION"
    success "${CANARY_PCT}% of traffic will be routed to v${CANARY_VERSION}."
}

cmd_canary_off() {
    require_soroban
    require_contract
    header "Disabling Canary"
    soroban_invoke set_canary --percentage 0 --new_version 0
    success "Canary disabled — 100% traffic on stable version."
}

cmd_help() {
    cat <<'HELPTEXT'

  Stellar Teye — Migration Script
  ─────────────────────────────────────────────────────────────────────────────

  Usage:
    ./scripts/migrate.sh COMMAND [OPTIONS]

  Commands:
    status                     Show current on-chain schema version
    dry-run  --to <v>          Validate migration (no writes)
    forward  --to <v>          Migrate data forward to version <v>
    rollback --to <v>          Roll back data to version <v>
    canary   --pct <n> --version <v>   Route n% of traffic to version v
    canary-off                 Disable canary; restore 100% stable
    help                       Show this help

  Options:
    --network   local|testnet|futurenet   (default: local)
    --identity  <soroban-identity>        (default: default)
    --contract  <contract-id>             (or export TEYE_CONTRACT_ID)
    --to        <target-version>          Required for dry-run/forward/rollback

  Examples:
    # Check what version is running
    TEYE_CONTRACT_ID=C... ./scripts/migrate.sh status --network testnet

    # Validate before upgrading
    TEYE_CONTRACT_ID=C... ./scripts/migrate.sh dry-run --to 3 --network testnet

    # Upgrade to v3
    TEYE_CONTRACT_ID=C... ./scripts/migrate.sh forward --to 3 --network testnet

    # Roll back to v2 if something went wrong
    TEYE_CONTRACT_ID=C... ./scripts/migrate.sh rollback --to 2 --network testnet

    # Send 10% of traffic to v3 (canary)
    TEYE_CONTRACT_ID=C... ./scripts/migrate.sh canary --pct 10 --version 3

    # Turn off canary
    TEYE_CONTRACT_ID=C... ./scripts/migrate.sh canary-off

HELPTEXT
}

# ─────────────────────────────────────────────────────────────────────────────
# Dispatch
# ─────────────────────────────────────────────────────────────────────────────

case "$COMMAND" in
    status)     cmd_status ;;
    dry-run)    cmd_dry_run ;;
    forward)    cmd_forward ;;
    rollback)   cmd_rollback ;;
    canary)     cmd_canary ;;
    canary-off) cmd_canary_off ;;
    help|"")    cmd_help ;;
    *)          error "Unknown command: $COMMAND. Run with 'help' to see usage." ;;
esac