#!/usr/bin/env bash
# =============================================================================
# visualize_lineage.sh — Teye Contracts provenance DAG visualiser
#
# Usage:
#   ./scripts/visualize_lineage.sh [OPTIONS] <COMMAND> <RECORD_ID>
#
# Commands:
#   export    Export the full provenance DAG as Graphviz DOT + render to SVG
#   ancestors Trace and print ancestor nodes (BFS)
#   verify    Verify the cryptographic integrity of the provenance chain
#   actors    List all actors that appear on lineage edges
#
# Options:
#   -n, --network  <testnet|mainnet|local>  Soroban network to query (default: local)
#   -d, --depth    <uint>                   Max BFS depth              (default: 10)
#   -o, --output   <path>                   Output file prefix         (default: lineage_<id>)
#   -c, --contract <CONTRACT_ID>            Vision-records contract ID (required)
#   -s, --source   <SOURCE_ACCOUNT>         Source account to sign     (required for write ops)
#   -h, --help                              Show this help message
#
# Dependencies:
#   - stellar CLI  (https://developers.stellar.org/docs/tools/stellar-cli)
#   - jq           (https://stedolan.github.io/jq/)
#   - dot           from Graphviz (for SVG rendering, optional)
#
# Exit codes:
#   0  success
#   1  usage / argument error
#   2  network / contract invocation error
#   3  rendering error (Graphviz unavailable)
# =============================================================================
set -euo pipefail

# ── Defaults ─────────────────────────────────────────────────────────────────
NETWORK="local"
DEPTH=10
OUTPUT_PREFIX=""
CONTRACT_ID=""
SOURCE_ACCOUNT=""
COMMAND=""
RECORD_ID=""

# ── Helpers ───────────────────────────────────────────────────────────────────
die() { echo "ERROR: $*" >&2; exit 1; }
info() { echo "[lineage] $*"; }

usage() {
    grep '^#' "$0" | sed 's/^# \{0,2\}//' | head -40
    exit 0
}

require_cmd() {
    command -v "$1" >/dev/null 2>&1 || die "'$1' is required but not installed."
}

# ── Argument parsing ──────────────────────────────────────────────────────────
while [[ $# -gt 0 ]]; do
    case "$1" in
        -h|--help)      usage ;;
        -n|--network)   NETWORK="$2";  shift 2 ;;
        -d|--depth)     DEPTH="$2";    shift 2 ;;
        -o|--output)    OUTPUT_PREFIX="$2"; shift 2 ;;
        -c|--contract)  CONTRACT_ID="$2"; shift 2 ;;
        -s|--source)    SOURCE_ACCOUNT="$2"; shift 2 ;;
        export|ancestors|verify|actors)
            COMMAND="$1"; shift ;;
        [0-9]*)
            RECORD_ID="$1"; shift ;;
        *)
            die "Unknown argument: $1. Run with --help for usage." ;;
    esac
done

[[ -z "$COMMAND" ]]   && die "No command specified. Use: export | ancestors | verify | actors"
[[ -z "$RECORD_ID" ]] && die "No RECORD_ID specified."
[[ -z "$CONTRACT_ID" ]] && die "--contract <CONTRACT_ID> is required."

require_cmd stellar
require_cmd jq

OUTPUT_PREFIX="${OUTPUT_PREFIX:-lineage_${RECORD_ID}}"

# ── Network flag for Stellar CLI ──────────────────────────────────────────────
case "$NETWORK" in
    local)    NET_FLAG="--network standalone" ;;
    testnet)  NET_FLAG="--network testnet"    ;;
    mainnet)  NET_FLAG="--network mainnet"    ;;
    *)        die "Unknown network: $NETWORK" ;;
esac

# ── Contract invocation wrapper ───────────────────────────────────────────────
invoke_contract() {
    local fn="$1"; shift
    stellar contract invoke \
        $NET_FLAG \
        --id "$CONTRACT_ID" \
        -- "$fn" "$@" 2>&1
}

# ── Convert relationship discriminant to label ────────────────────────────────
kind_label() {
    case "$1" in
        1) echo "created"         ;;
        2) echo "derived_from"    ;;
        3) echo "modified_by"     ;;
        4) echo "shared_with"     ;;
        5) echo "aggregated_into" ;;
        6) echo "cross_contract"  ;;
        *) echo "unknown($1)"     ;;
    esac
}

# ── Edge style by relationship kind ──────────────────────────────────────────
edge_style() {
    case "$1" in
        1) echo 'color="#2ecc71",style=bold'        ;; # created      → green
        2) echo 'color="#3498db",style=dashed'      ;; # derived_from → blue dashed
        3) echo 'color="#e67e22"'                   ;; # modified_by  → orange
        4) echo 'color="#9b59b6",style=dotted'      ;; # shared_with  → purple dotted
        5) echo 'color="#e74c3c",style=bold'        ;; # aggregated   → red
        6) echo 'color="#1abc9c",style=dashed'      ;; # cross-chain  → teal
        *) echo 'color="#95a5a6"'                   ;;
    esac
}

# ── Commands ──────────────────────────────────────────────────────────────────

cmd_export() {
    info "Exporting provenance DAG for record $RECORD_ID (depth=$DEPTH)…"

    local raw
    raw=$(invoke_contract export_record_dag \
        --record_id "$RECORD_ID" \
        --max_depth "$DEPTH") \
        || die "Contract invocation failed:\n$raw"

    local dot_file="${OUTPUT_PREFIX}.dot"
    local svg_file="${OUTPUT_PREFIX}.svg"

    # Parse node IDs and edges from the JSON output.
    local node_ids edge_list

    node_ids=$(echo "$raw" | jq -r '.node_ids[]? // empty' 2>/dev/null || echo "")
    edge_list=$(echo "$raw" | jq -c '.edges[]? // empty' 2>/dev/null || echo "")
    local root_id depth_reached
    root_id=$(echo "$raw"    | jq -r '.root_record_id // "?"' 2>/dev/null || echo "?")
    depth_reached=$(echo "$raw" | jq -r '.depth_reached // 0' 2>/dev/null || echo 0)

    info "Root: $root_id | Depth reached: $depth_reached"

    # Build Graphviz DOT representation.
    {
        echo "digraph provenance {"
        echo "  graph [label=\"Provenance DAG — Record ${root_id}  (depth=${depth_reached})\","
        echo "         fontname=\"Helvetica\", fontsize=16, bgcolor=\"#0f0f12\","
        echo "         fontcolor=\"white\", rankdir=LR];"
        echo "  node  [shape=box, style=\"filled,rounded\", fontname=\"Helvetica\","
        echo "         fontsize=12, fillcolor=\"#1e1e2e\", fontcolor=\"white\","
        echo "         color=\"#7c7cff\"];"
        echo "  edge  [fontname=\"Helvetica\", fontsize=10, fontcolor=\"#cccccc\"];"
        echo ""

        # Nodes
        while IFS= read -r nid; do
            [[ -z "$nid" ]] && continue
            local shape='box'
            [[ "$nid" == "$root_id" ]] && shape='doubleoctagon'
            echo "  n${nid} [label=\"Record\\n${nid}\", shape=${shape}];"
        done <<< "$node_ids"

        echo ""

        # Edges: each entry is a 4-tuple [edge_id, source_id, target_id, kind_u32]
        while IFS= read -r etuple; do
            [[ -z "$etuple" ]] && continue
            local eid src tgt kind lbl style
            eid=$(echo "$etuple" | jq -r '.[0]')
            src=$(echo "$etuple" | jq -r '.[1]')
            tgt=$(echo "$etuple" | jq -r '.[2]')
            kind=$(echo "$etuple" | jq -r '.[3]')
            lbl=$(kind_label "$kind")
            style=$(edge_style "$kind")
            echo "  n${src} -> n${tgt} [label=\"${lbl}\", tooltip=\"edge_id=${eid}\", ${style}];"
        done <<< "$edge_list"

        echo "}"
    } > "$dot_file"

    info "DOT file written to: $dot_file"

    # Render to SVG if Graphviz is available.
    if command -v dot >/dev/null 2>&1; then
        dot -Tsvg "$dot_file" -o "$svg_file" \
            || { info "Graphviz rendering failed; DOT file still available."; exit 3; }
        info "SVG rendered to:     $svg_file"

        # Try to open in the default browser/viewer.
        if command -v open >/dev/null 2>&1; then
            open "$svg_file"
        elif command -v xdg-open >/dev/null 2>&1; then
            xdg-open "$svg_file"
        fi
    else
        info "Graphviz 'dot' not found — skipping SVG render."
        info "Install with:  brew install graphviz  or  apt install graphviz"
    fi
}

cmd_ancestors() {
    info "Tracing ancestors of record $RECORD_ID (depth=$DEPTH)…"

    local raw
    raw=$(invoke_contract trace_record_ancestors \
        --record_id "$RECORD_ID" \
        --max_depth "$DEPTH") \
        || die "Contract invocation failed:\n$raw"

    local total truncated
    total=$(echo     "$raw" | jq -r '.total_visited // 0')
    truncated=$(echo "$raw" | jq -r '.truncated // false')

    echo ""
    printf "%-8s  %-6s  %-20s  %s\n" "RecordID" "Depth" "Creator" "Via"
    printf '%0.s─' {1..70}; echo ""

    echo "$raw" | jq -c '.nodes[]?' 2>/dev/null | while IFS= read -r n; do
        local rid depth creator via_kind
        rid=$(echo       "$n" | jq -r '.node.record_id // "-"')
        depth=$(echo     "$n" | jq -r '.depth // 0')
        creator=$(echo   "$n" | jq -r '.node.creator // "-"' | cut -c1-20)
        via_kind=$(echo  "$n" | jq -r '.via_edge.kind // "genesis"')
        [[ "$via_kind" =~ ^[0-9]+$ ]] && via_kind=$(kind_label "$via_kind")
        printf "%-8s  %-6s  %-20s  %s\n" "$rid" "$depth" "$creator" "$via_kind"
    done

    echo ""
    info "Total ancestors visited: $total  (truncated: $truncated)"
}

cmd_verify() {
    info "Verifying provenance integrity for record $RECORD_ID (depth=$DEPTH)…"

    local raw
    raw=$(invoke_contract verify_record_provenance \
        --record_id "$RECORD_ID" \
        --max_depth "$DEPTH") \
        || die "Contract invocation failed:\n$raw"

    echo ""
    echo "Result: $raw"
    echo ""

    if echo "$raw" | grep -qi '"Valid"'; then
        info "✅  Provenance chain is intact and untampered."
        exit 0
    elif echo "$raw" | grep -qi '"Tampered"'; then
        local at
        at=$(echo "$raw" | jq -r '.Tampered // "?"' 2>/dev/null || echo "?")
        die "❌  Commitment mismatch at record $at — history may have been tampered!"
    elif echo "$raw" | grep -qi '"MissingAncestor"'; then
        local missing
        missing=$(echo "$raw" | jq -r '.MissingAncestor // "?"' 2>/dev/null || echo "?")
        die "⚠️  Missing ancestor node $missing — chain has a gap."
    else
        die "Unknown verification result: $raw"
    fi
}

cmd_actors() {
    info "Collecting lineage actors for record $RECORD_ID (depth=$DEPTH)…"

    # Build the ancestor traversal and collect unique actors from edges.
    local raw
    raw=$(invoke_contract trace_record_ancestors \
        --record_id "$RECORD_ID" \
        --max_depth "$DEPTH") \
        || die "Contract invocation failed:\n$raw"

    echo ""
    echo "Actor addresses found on provenance edges:"
    echo "$raw" | jq -r '.nodes[]?.via_edge.actor // empty' 2>/dev/null | sort -u | \
        while IFS= read -r addr; do
            [[ -n "$addr" ]] && echo "  • $addr"
        done
    echo ""
}

# ── Dispatch ──────────────────────────────────────────────────────────────────
case "$COMMAND" in
    export)    cmd_export    ;;
    ancestors) cmd_ancestors ;;
    verify)    cmd_verify    ;;
    actors)    cmd_actors    ;;
    *)         die "Unknown command: $COMMAND" ;;
esac
