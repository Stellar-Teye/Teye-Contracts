# Security Policy

## Supported Versions

We provide security updates for the following contract and SDK versions:

| Version | Supported          |
| ------- | ------------------ |
| 2.x     | :white_check_mark: |
| 1.x     | :x:                |

(Update the table to match your release strategy.)

---

## Reporting a Vulnerability

We take security vulnerabilities seriously. We encourage responsible disclosure and will work with reporters to understand and address issues before public disclosure.

### How to Report

- **Preferred:** Report vulnerabilities privately via **GitHub Security Advisories** for this repository.
  1. Go to the **Security** tab of the repository.
  2. Click **Report a vulnerability** (or use “Advisories” → “New draft advisory”).
  3. Describe the vulnerability, impact, steps to reproduce, and any suggested fix.
  4. Submit the draft advisory. Only maintainers and you can see it until it is published.

- **Alternative:** If you cannot use GitHub, send an email to the maintainers (provide a contact address in this section, e.g. `security@example.com`) with:
  - Subject line: `[Teye-Contracts] Security: brief description`
  - Description of the vulnerability and affected code (contract name, function, version).
  - Steps to reproduce and proof-of-concept if possible.
  - Impact (e.g. unauthorized access, fund loss, data leak).
  - Suggested mitigation or patch if you have one.

**Do not** open a public GitHub issue for a security vulnerability.

### What to Expect

- **Acknowledgment:** We will acknowledge receipt of your report within **5 business days**.
- **Assessment:** We will confirm whether the report is in scope and whether we consider it a valid vulnerability. We may ask for clarification or additional details.
- **Updates:** We will provide updates on our progress and timeline for a fix (if applicable) within **14 days** of acknowledgment, and will keep you informed of major changes.
- **Fix and disclosure:** We will work on a fix and coordinate with you on the timing of a security advisory and any CVE assignment. We credit reporters in advisories unless they prefer to remain anonymous.

### Scope

- **In scope:** Smart contracts in this repository (`vision_records`, `zk_verifier`, `staking`, `identity`, and shared libraries used by them), including:
  - Access control and authorization bypass
  - Reentrancy, integer overflow/underflow, or other logic errors
  - Storage or key collision issues
  - Incorrect ZK proof verification or key handling
  - Token handling or reward math errors
  - Identity and recovery logic flaws
- **Out of scope:** General dependency vulnerabilities (e.g. in dev tools) that do not affect the deployed contracts, and issues in other repositories or off-chain systems unless they directly impact contract security.

### Safe Harbor

We support safe harbor for security researchers who:

- Make a good-faith effort to avoid privacy violations, destruction of data, and disruption of services.
- Do not exploit the vulnerability beyond what is necessary to demonstrate it.
- Report the vulnerability to us in line with this policy.

We will not pursue legal action or support law enforcement action against researchers who comply with this policy and do not violate applicable laws.

---

## Security Advisories

Published security advisories will be listed under the **Security** tab → **Advisories**. Each advisory will include:

- Affected versions
- Description and impact
- Severity (e.g. Critical, High, Medium, Low)
- Mitigation or upgrade path
- Credits (with permission)

---

## Security Audit and Checklist

- A **security audit checklist** covering integer overflow, access control, storage key collisions, reentrancy, input validation, and event emission for all public functions is maintained in [docs/security-audit-checklist.md](docs/security-audit-checklist.md).
- A **STRIDE threat model** for the healthcare data system is in [docs/threat-model.md](docs/threat-model.md).

We recommend running the checklist on any change that adds or modifies public contract functions and before major releases.

---

*Last updated: February 2025.*
