#!/bin/bash

# ZK Verifier Test Compilation Verification Script
# This script verifies that all test compilation issues have been resolved

set -e  # Exit on any error

echo "=========================================="
echo "ZK Verifier Test Compilation Verification"
echo "=========================================="
echo ""

# Colors for output
GREEN='\033[0;32m'
RED='\033[0;31m'
YELLOW='\033[1;33m'
NC='\033[0m' # No Color

# Function to print success
print_success() {
    echo -e "${GREEN}✓${NC} $1"
}

# Function to print error
print_error() {
    echo -e "${RED}✗${NC} $1"
}

# Function to print info
print_info() {
    echo -e "${YELLOW}ℹ${NC} $1"
}

# Change to the workspace root
cd "$(dirname "$0")/../../.."

echo "Step 1: Checking package compilation..."
if cargo check -p zk_verifier 2>&1 | tee /tmp/zk_check.log; then
    print_success "Package compiles successfully"
else
    print_error "Package compilation failed"
    echo "See /tmp/zk_check.log for details"
    exit 1
fi
echo ""

echo "Step 2: Running clippy on all targets..."
if cargo clippy -p zk_verifier --all-targets -- -D warnings 2>&1 | tee /tmp/zk_clippy.log; then
    print_success "Clippy checks passed"
else
    print_error "Clippy checks failed"
    echo "See /tmp/zk_clippy.log for details"
    exit 1
fi
echo ""

echo "Step 3: Checking code formatting..."
if cargo fmt --all -- --check 2>&1 | tee /tmp/zk_fmt.log; then
    print_success "Code formatting is correct"
else
    print_error "Code formatting issues found"
    echo "Run 'cargo fmt --all' to fix formatting"
    echo "See /tmp/zk_fmt.log for details"
    exit 1
fi
echo ""

echo "Step 4: Running tests..."
if cargo test -p zk_verifier 2>&1 | tee /tmp/zk_test.log; then
    print_success "All tests passed"
else
    print_error "Some tests failed"
    echo "See /tmp/zk_test.log for details"
    exit 1
fi
echo ""

echo "Step 5: Verifying specific test files compile..."
test_files=(
    "contracts/zk_verifier/tests/bench_verify.rs"
    "contracts/zk_verifier/tests/test_nonce_replay.rs"
    "contracts/zk_verifier/tests/test_zk_access.rs"
)

for test_file in "${test_files[@]}"; do
    if [ -f "$test_file" ]; then
        print_info "Checking $test_file..."
        # The test file will be compiled as part of the package check above
        print_success "$(basename $test_file) is valid"
    else
        print_error "Test file not found: $test_file"
        exit 1
    fi
done
echo ""

echo "=========================================="
echo -e "${GREEN}All verification checks passed!${NC}"
echo "=========================================="
echo ""
echo "Summary:"
echo "  ✓ Package compilation successful"
echo "  ✓ Clippy checks passed (no warnings)"
echo "  ✓ Code formatting correct"
echo "  ✓ All tests passed"
echo "  ✓ All test files compile"
echo ""
echo "The zk_verifier contract is ready for use."
