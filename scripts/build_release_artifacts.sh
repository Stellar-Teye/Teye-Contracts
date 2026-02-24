#!/usr/bin/env bash

set -euo pipefail

ROOT_DIR="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
DIST_DIR="${ROOT_DIR}/dist"

cd "${ROOT_DIR}"

rm -rf "${DIST_DIR}"
mkdir -p "${DIST_DIR}"

contract_dirs="$(find contracts -mindepth 1 -maxdepth 1 -type d | sort)"

if [ -z "${contract_dirs}" ]; then
  echo "No contract directories found in contracts/"
  exit 1
fi

while IFS= read -r contract_dir; do
  [ -z "${contract_dir}" ] && continue
  package_name="$(basename "${contract_dir}")"
  echo "Building package: ${package_name}"
  cargo build --release --target wasm32-unknown-unknown -p "${package_name}"

  wasm_path="target/wasm32-unknown-unknown/release/${package_name}.wasm"
  if [ ! -f "${wasm_path}" ]; then
    echo "Expected wasm artifact not found: ${wasm_path}"
    exit 1
  fi

  cp "${wasm_path}" "${DIST_DIR}/${package_name}.wasm"
done <<EOF
${contract_dirs}
EOF

(
  cd "${DIST_DIR}"
  sha256sum ./*.wasm > SHA256SUMS.txt
)

echo "Release artifacts created in ${DIST_DIR}"
