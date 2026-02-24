#!/bin/bash
# Script: check_storage_keys.sh
# Purpose: Detect duplicate symbol_short! storage keys within each contract

set -e

# Find all contracts
CONTRACT_DIRS=$(find contracts -mindepth 2 -maxdepth 2 -type d)

EXIT_CODE=0

for CONTRACT in $CONTRACT_DIRS; do
    echo "Checking $CONTRACT..."
    # Grep for symbol_short! usages, extract key names
    KEYS=$(grep -rho 'symbol_short!\([A-Z0-9_]*\)' "$CONTRACT" | sed 's/symbol_short!\([A-Z0-9_]*\)/\1/' | sort)
    if [ -z "$KEYS" ]; then
        echo "  No symbol_short! keys found."
        continue
    fi
    # Check for duplicates
    DUPES=$(echo "$KEYS" | uniq -d)
    if [ -n "$DUPES" ]; then
        echo "  Duplicate keys detected: $DUPES"
        EXIT_CODE=1
    else
        echo "  No duplicates found."
    fi
    echo
done

exit $EXIT_CODE
