# Pre-Deployment Checklist

Use this checklist before any `testnet`, `futurenet`, or `mainnet` deployment.

## 1. Environment Verification

- [ ] Confirm repo is clean or intentionally dirty for this release: `git status --short`
- [ ] Confirm Rust matches workspace requirement (`1.78+`): `rustc --version`
- [ ] Confirm Soroban CLI is installed and available: `soroban --version`
- [ ] Confirm required target is installed: `rustup target list --installed | rg wasm32-unknown-unknown`
- [ ] Confirm network aliases exist: `soroban network ls`
- [ ] Confirm deployer identity exists: `soroban keys ls`
- [ ] Confirm deployer address resolves: `soroban keys address default`
- [ ] Confirm RPC connectivity for target network:
  - `curl -s -X POST -H 'Content-Type: application/json' -d '{"jsonrpc":"2.0","id":1,"method":"getHealth"}' https://rpc-futurenet.stellar.org`
  - Expected output contains either `"status":"healthy"` or `"jsonrpc":"2.0"`

## 2. Security Audit Checklist

- [ ] Review and complete `docs/security-audit-checklist.md`
- [ ] Confirm known-risk entries are accepted or remediated for this release
- [ ] Confirm `docs/deployment-security.md` key ceremony steps are planned
- [ ] Confirm incident contacts are current in `docs/emergency-contacts.md`

## 3. Test and Quality Gates

- [ ] Run unit/integration suites: `cargo test --all`
- [ ] Run lints: `cargo clippy --all-targets --all-features -- -D warnings`
- [ ] Optional formatting gate: `cargo fmt --all -- --check`
- [ ] Run coverage script: `./scripts/run_coverage.sh`
- [ ] Verify coverage meets release threshold (recommended: `>= 90%` for critical contracts)
- [ ] Run upgrade regression tests before upgrade releases:
  - `cargo test --test upgrade -- --nocapture` (if configured in CI)
  - `cargo test test_upgrade -- --nocapture` (project-specific variants)

## 4. Contract Binary Verification

- [ ] Build release artifacts: `./scripts/build_release_artifacts.sh`
- [ ] Verify checksums file exists: `test -f dist/SHA256SUMS.txt`
- [ ] Compute and compare hash for each target contract:
  - `sha256sum target/wasm32-unknown-unknown/release/<contract>.wasm`
  - `rg "<contract>.wasm" dist/SHA256SUMS.txt`
- [ ] Confirm hash in deployment descriptor matches built artifact post-deploy:
  - `cat deployments/<network>_<contract>.json`

## 5. Key Management Verification

- [ ] Confirm permanent admin address is approved for this release
- [ ] Confirm deploy command will use `--admin <PERMANENT_ADMIN_ADDRESS>`
- [ ] Confirm multisig signers/threshold for contracts that require governance controls
- [ ] Confirm backup admin path exists (cold key or emergency multisig)
- [ ] Confirm temporary deployer key deletion plan after successful production deploy

## 6. Release Metadata Readiness

- [ ] Version bumped (if needed): `./scripts/prepare_release_version.sh <semver>`
- [ ] Artifacts built: `./scripts/build_release_artifacts.sh`
- [ ] Release notes generated: `./scripts/generate_release_notes.sh <version> <tag>`
- [ ] Deployment window and rollback owner assigned

## Exit Criteria

Do not proceed unless all checklist items above are complete and explicitly signed off by the deployment owner.
