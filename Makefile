.PHONY: all build test clean fmt lint setup deploy-local deploy-testnet deploy-mainnet deploy-secure

# Default target
all: build test

# Build all contracts
build:
	cargo build --all-targets
	cargo build --target wasm32-unknown-unknown --release

# Build optimized for production
build-release:
	cargo build --target wasm32-unknown-unknown --release
	@echo "Optimized WASM files are in target/wasm32-unknown-unknown/release/"

# Run all tests
test:
	cargo test --all

# Run unit tests only
test-unit:
	cargo test --lib

# Run integration tests only
test-integration:
	cargo test --test '*'

# Clean build artifacts
clean:
	cargo clean

# Format code
fmt:
	cargo fmt --all

# Check formatting
fmt-check:
	cargo fmt --all -- --check

# Run clippy linter
lint:
	cargo clippy --all-targets --all-features -- -D warnings

# Run clippy with fixes
lint-fix:
	cargo clippy --fix --allow-dirty --allow-staged

# Setup development environment
setup:
	rustup target add wasm32-unknown-unknown
	rustup component add rustfmt clippy rust-src
	cargo install --locked soroban-cli || true
	@echo "Setup complete!"

# Start local Soroban network
start-local:
	soroban network start local

# Stop local Soroban network
stop-local:
	soroban network stop local

# Deploy to local network
deploy-local:
	./scripts/deploy.sh local

# Deploy to testnet
deploy-testnet:
	./scripts/deploy.sh testnet

# Deploy to mainnet (requires confirmation)
deploy-mainnet:
	@echo "WARNING: Deploying to mainnet!"
	@read -p "Are you sure? (yes/no): " confirm && [ "$$confirm" = "yes" ]
	./scripts/deploy.sh mainnet

# Secure deploy with admin transfer (least-privilege)
# Usage: ADMIN_ADDRESS=GXXX... NETWORK=testnet make deploy-secure
deploy-secure:
ifndef ADMIN_ADDRESS
	$(error ADMIN_ADDRESS is required. Usage: ADMIN_ADDRESS=GXXX... make deploy-secure)
endif
	./scripts/deploy.sh $(or $(NETWORK),testnet) vision_records --admin $(ADMIN_ADDRESS)

# Generate documentation
docs:
	cargo doc --no-deps --open

# Check everything before commit
check: fmt-check lint test
	@echo "All checks passed!"

# Watch for changes and run tests
watch:
	cargo watch -x test

# Display help
help:
	@echo "Available targets:"
	@echo "  all           - Build and test"
	@echo "  build         - Build all contracts"
	@echo "  build-release - Build optimized WASM"
	@echo "  test          - Run all tests"
	@echo "  test-unit     - Run unit tests"
	@echo "  test-integration - Run integration tests"
	@echo "  clean         - Clean build artifacts"
	@echo "  fmt           - Format code"
	@echo "  lint          - Run clippy"
	@echo "  setup         - Setup dev environment"
	@echo "  deploy-local  - Deploy to local network"
	@echo "  deploy-testnet - Deploy to testnet"
	@echo "  deploy-secure - Deploy with admin transfer (least-privilege)"
	@echo "  check         - Run all checks"
