# Makefile Command Reference

This reference documents every Makefile target and usage examples.

## Build Targets

- `make all` - Build and test everything.
  - Example: `make all`
- `make build` - Build all contracts and WASM.
  - Example: `make build`
- `make build-release` - Build optimized WASM artifacts.
  - Example: `make build-release`

## Test Targets

- `make test` - Run all tests.
  - Example: `make test`
- `make test-unit` - Run unit tests only.
  - Example: `make test-unit`
- `make test-integration` - Run integration tests only.
  - Example: `make test-integration`

## Deployment Targets

- `make deploy-local` - Deploy to local network.
  - Example: `make deploy-local`
- `make deploy-testnet` - Deploy to testnet.
  - Example: `make deploy-testnet`
- `make deploy-mainnet` - Deploy to mainnet (requires confirmation).
  - Example: `make deploy-mainnet`
- `make deploy-secure` - Deploy with admin transfer.
  - Example: `ADMIN_ADDRESS=GXXX... NETWORK=testnet make deploy-secure`

## Utility Targets

- `make clean` - Remove build artifacts.
  - Example: `make clean`
- `make fmt` - Format code.
  - Example: `make fmt`
- `make fmt-check` - Verify formatting.
  - Example: `make fmt-check`
- `make lint` - Run Clippy.
  - Example: `make lint`
- `make lint-fix` - Run Clippy with fixes.
  - Example: `make lint-fix`
- `make setup` - Setup dev environment.
  - Example: `make setup`
- `make docs` - Build Rust docs.
  - Example: `make docs`
- `make check` - Run fmt-check, lint, and tests.
  - Example: `make check`
- `make watch` - Watch and re-run tests on changes.
  - Example: `make watch`
- `make help` - Print target list.
  - Example: `make help`

## Network Targets

- `make start-local` - Start local Soroban network.
  - Example: `make start-local`
- `make stop-local` - Stop local Soroban network.
  - Example: `make stop-local`
