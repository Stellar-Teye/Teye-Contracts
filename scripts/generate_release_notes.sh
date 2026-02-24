#!/usr/bin/env bash

set -euo pipefail

if [ "${#}" -lt 2 ]; then
  echo "Usage: $0 <version> <tag>"
  exit 64
fi

version="${1}"
tag="${2}"
changelog_file="CHANGELOG.md"
dist_dir="dist"
checksums_file="${dist_dir}/SHA256SUMS.txt"
output_file="${dist_dir}/RELEASE_NOTES.md"

if [ ! -f "${changelog_file}" ]; then
  echo "Missing changelog file: ${changelog_file}"
  exit 1
fi

if [ ! -f "${checksums_file}" ]; then
  echo "Missing checksums file: ${checksums_file}"
  exit 1
fi

mkdir -p "${dist_dir}"

changelog_section="$(awk -v version="${version}" '
  BEGIN { capture = 0 }
  $0 ~ "^## \\[" version "\\]" { capture = 1; next }
  capture && /^## \[/ { capture = 0 }
  capture { print }
' "${changelog_file}")"

{
  echo "# ${tag}"
  echo
  echo "## Summary"
  echo "Automated release for version ${version}."
  echo
  echo "## Changelog"
  if [ -n "${changelog_section}" ]; then
    printf '%s\n' "${changelog_section}"
  else
    echo "- See the full \`${changelog_file}\` for details."
  fi
  echo
  echo "## Artifacts"
  while read -r checksum artifact_path; do
    artifact_name="$(basename "${artifact_path}")"
    echo "- \`${artifact_name}\`"
    echo "  - SHA256: \`${checksum}\`"
  done < "${checksums_file}"
} > "${output_file}"

echo "Generated ${output_file}"
