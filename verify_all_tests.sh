#!/bin/bash

# Comprehensive Test Verification Script
# Verifies all packages affected by zk_verifier export fixes

set -e

GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
BLUE='\033[0;34m'
NC='\033[0m'

print_success() {
    echo -e "${GREEN}✓${NC} $1"
}

print_error() {
    echo -e "${RED}✗${NC} $1"
}

print_info() {
    echo -e "${YELLOW}ℹ${NC} $1"
}

print_header() {
    echo -e "${BLUE}========================================${NC}"
    echo -e "${BLUE}$1${NC}"
    echo -e "${BLUE}========================================${NC}"
}

PACKAGES=("zk_verifier" "zk_voting" "zk_prover" "identity")
FAILED=0

print_header "Verifying All Test Compilations"
echo ""

for package in "${PACKAGES[@]}"; do
    print_info "Checking package: $package"
    
    # Check compilation
    if cargo check -p "$package" --all-targets 2>&1 | tee "/tmp/${package}_check.log" > /dev/null; then
        print_success "$package: Compilation successful"
    else
        print_error "$package: Compilation failed"
        echo "See /tmp/${package}_check.log for details"
        FAILED=$((FAILED + 1))
    fi
    
    # Run clippy
    if cargo clippy -p "$package" --all-targets -- -D warnings 2>&1 | tee "/tmp/${package}_clippy.log" > /dev/null; then
        print_success "$package: Clippy checks passed"
    else
        print_error "$package: Clippy checks failed"
        echo "See /tmp/${package}_clippy.log for details"
        FAILED=$((FAILED + 1))
    fi
    
    echo ""
done

print_header "Summary"
echo ""

if [ $FAILED -eq 0 ]; then
    print_success "All packages verified successfully!"
    echo ""
    echo "Packages checked:"
    for package in "${PACKAGES[@]}"; do
        echo "  ✓ $package"
    done
    echo ""
    echo "You can now run tests with:"
    echo "  cargo test -p zk_verifier"
    echo "  cargo test -p zk_voting"
    echo "  cargo test -p zk_prover"
    echo "  cargo test -p identity"
    exit 0
else
    print_error "$FAILED package(s) failed verification"
    echo ""
    echo "Check the log files in /tmp/ for details"
    exit 1
fi
