# Security Scanning and Vulnerability Management

This document describes how automated security scanning is implemented for the Teye-Contracts repository and how to interpret and respond to findings.

## Overview

Security checks are enforced in CI via the `Security Scanning` workflow:

- **Dependency vulnerability scanning** with `cargo-audit`
- **Security-focused linting** with `clippy`
- **Secret scanning** with `gitleaks`
- **Security summary reporting** in the GitHub Actions run

The workflow is defined in `.github/workflows/security.yml` and runs on:

- Pushes and pull requests targeting the main branches
- A scheduled daily run (cron)
- Manual triggers via `workflow_dispatch`

## Dependency Scanning (cargo-audit)

The `audit` job runs [`cargo-audit`](https://github.com/RustSec/rustsec/tree/main/cargo-audit) against the workspace:

- Checks `Cargo.lock` for known vulnerabilities from the RustSec advisory database
- Fails the job if any vulnerability with a published advisory is detected
- Uploads `Cargo.lock` as a build artifact for traceability

**Developer expectations:**

- Keep `Cargo.lock` committed and up to date
- When `cargo-audit` fails:
  - Prefer upgrading to a non-vulnerable version of the affected crate
  - If upgrading is not immediately possible, open a tracking issue documenting:
    - The advisory ID (e.g., `RUSTSEC-YYYY-XXXX`)
    - The impacted crate and version
    - The mitigation or compensating controls

## Security-Focused Clippy Lints

The `clippy-security` job runs `cargo clippy` with a stricter set of lints aimed at catching risky patterns:

- `-D warnings` promotes all warnings to errors
- Additional lints:
  - `clippy::unwrap_used`
  - `clippy::expect_used`
  - `clippy::panic`
  - `clippy::arithmetic_side_effects`

**Developer expectations:**

- Avoid using `unwrap`/`expect` in contract code; prefer explicit error handling
- Avoid `panic!` in on-chain code paths
- Address arithmetic lints by:
  - Using checked or saturating arithmetic where appropriate
  - Documenting invariants that guarantee safety when using plain operators

## Secret Scanning (Gitleaks)

The `secret-scanning` job runs [`gitleaks`](https://github.com/gitleaks/gitleaks) against the full Git history:

- Clones the repository with full history (`fetch-depth: 0`)
- Runs `gitleaks detect --source . --verbose --redact`
- Fails the job on any detected secret

**Developer expectations:**

- Never commit private keys, seed phrases, API tokens, or other secrets
- If a secret is accidentally committed:
  1. **Revoke** the secret immediately (rotate keys, regenerate tokens)
  2. **Replace** the secret wherever it is used
  3. Open a security incident ticket and document the impact and mitigation

## Security Summary and Reporting

The `security-summary` job aggregates the results of all security jobs and writes a human-readable summary to the GitHub Actions run:

- Lists the status of:
  - Dependency Audit
  - Clippy Security
  - Secret Scanning
- Fails the summary job (and thus the workflow) if any of the dependent jobs failed

You can view the summary in the **GitHub Actions run page** under the `Security Summary` step output.

## GitHub Security Features and Dependency Review

This repository is designed to integrate with GitHub's additional security features:

- **Dependency graph and vulnerability alerts**
- **Dependency review** on pull requests

The `security.yml` workflow includes a commented-out `dependency-review` job that can be enabled once the Dependency Graph is turned on in the repository settings:

1. Navigate to **Settings → Code security and analysis**
2. Enable **Dependency graph** and **Dependabot alerts**
3. Uncomment the `dependency-review` job in `.github/workflows/security.yml`

When enabled, dependency review will:

- Highlight risky dependency changes in pull requests
- Optionally fail builds when new dependencies introduce advisories above a chosen severity level

## Local Security Checks

Developers can run security checks locally before pushing changes:

```bash
# Run clippy with security-focused lints
cargo clippy --all-targets --all-features \
  -- -D warnings \
  -W clippy::unwrap_used \
  -W clippy::expect_used \
  -W clippy::panic \
  -W clippy::arithmetic_side_effects

# Run cargo-audit (requires cargo-audit installed)
cargo install cargo-audit --features=fix
cargo audit

# Run gitleaks against the current working tree
gitleaks detect --source . --verbose --redact
```

## Incident Response and Responsible Disclosure

If you discover a potential vulnerability:

1. Do **not** create a public GitHub issue with sensitive details.
2. Follow the project’s security or disclosure policy (if present in `SECURITY.md` or the repository description).
3. Provide:
   - A minimal reproduction or clear description
   - Potential impact
   - Any suggested mitigations

This process helps ensure that vulnerabilities are addressed quickly and responsibly.

