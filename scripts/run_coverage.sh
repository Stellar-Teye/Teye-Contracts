#!/usr/bin/env bash
# Run code coverage locally and print a summary.
# Usage: ./scripts/run_coverage.sh [--html]
#
# Dependencies: cargo-tarpaulin
#   Install via: cargo install cargo-tarpaulin

set -euo pipefail

HTML=false
if [ "${1:-}" = "--html" ]; then
  HTML=true
fi

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$ROOT_DIR"

COVERAGE_DIR="$ROOT_DIR/coverage"
mkdir -p "$COVERAGE_DIR"

echo "Running coverage (this may take a few minutes)..."

TARPAULIN_ARGS=(
  --workspace
  --out xml
  --output-dir "$COVERAGE_DIR"
  --timeout 600
  --exclude-files "tests/*" "fuzz/*" "contracts/benches/*" "example/*"
  --engine llvm
  --no-fail-fast
)

if [ "$HTML" = true ]; then
  TARPAULIN_ARGS+=(--out html)
fi

cargo tarpaulin "${TARPAULIN_ARGS[@]}" 2>&1 | tee "$COVERAGE_DIR/tarpaulin.log"

echo ""
echo "Coverage report: $COVERAGE_DIR/cobertura.xml"
if [ "$HTML" = true ]; then
  echo "HTML report:     $COVERAGE_DIR/tarpaulin-report.html"
fi
