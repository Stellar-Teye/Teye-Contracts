#!/bin/bash
# Storage Key Collision Detection Script (Refined)
# Scans for storage key definitions (const or let) using symbol_short!
# and checks for duplicates within each contract's source.

ROOT_DIR=$(git rev-parse --show-toplevel 2>/dev/null || pwd)
CONTRACTS_DIR="$ROOT_DIR/contracts"
EXIT_CODE=0

echo "Checking for storage key collisions in $CONTRACTS_DIR..."

for contract in "$CONTRACTS_DIR"/*/; do
    if [ -d "$contract/src" ]; then
        contract_name=$(basename "$contract")

        # We want to find actual DEFINITIONS or unique usages that represent keys.
        # This is tricky without a full parser, but we can look for:
        # 1. const DEFINITION: Symbol = symbol_short!("...")
        # 2. let key = symbol_short!("...")
        # We should ignore tests and event files if they are just REPRODUCING the symbol
        # for publication or testing purposes.

        # For now, let's grep for all symbol_short! and filter out common false positives
        # or just be more specific about what constitutes a "definition".

        # If the same symbol is used multiple times, it might be fine (e.g. get/set).
        # But if it's defined as a CONST multiple times, that's a collision.

        const_definitions=$(grep -r "const .*Symbol = symbol_short!(" "$contract/src" | \
                            sed -n 's/.*symbol_short!("\([^"]*\)").*/\1/p' | \
                            sort)

        duplicates=$(echo "$const_definitions" | uniq -d)

        if [ -n "$duplicates" ]; then
            echo "❌ Collision(s) found in contract: $contract_name"
            while IFS= read -r dup; do
                echo "   Symbol: $dup"
                grep -r "const .*symbol_short!(\"$dup\")" "$contract/src" | sed 's/^/      /'
            done <<< "$duplicates"
            EXIT_CODE=1
        fi
    fi
done

if [ $EXIT_CODE -eq 0 ]; then
    echo "✅ No storage key collisions detected."
fi

exit $EXIT_CODE
