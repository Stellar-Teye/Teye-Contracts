#!/usr/bin/env bash

set -euo pipefail

if [ "${#}" -ne 1 ]; then
  echo "Usage: $0 <version>"
  exit 64
fi

version="${1}"
cargo_toml="Cargo.toml"

if [ ! -f "${cargo_toml}" ]; then
  echo "Unable to locate ${cargo_toml}"
  exit 1
fi

if ! printf '%s' "${version}" | grep -Eq '^[0-9]+\.[0-9]+\.[0-9]+([.-][0-9A-Za-z.-]+)?$'; then
  echo "Invalid semantic version: ${version}"
  exit 1
fi

VERSION="${version}" perl -0777 -i -pe 's/(\[workspace\.package\][^\[]*?version\s*=\s*")[^"]+(")/$1$ENV{VERSION}$2/s' "${cargo_toml}"

echo "Updated workspace version to ${version} in ${cargo_toml}"
