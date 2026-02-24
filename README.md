# Stellar Teye - Decentralized Vision Care Records on Stellar

[![Rust](https://img.shields.io/badge/Rust-1.78%2B-orange.svg)](https://www.rust-lang.org/)
[![Soroban](https://img.shields.io/badge/Soroban-v23.1.4-blue.svg)](https://soroban.stellar.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-green.svg)](LICENSE)
[![CI](https://github.com/Stellar-Teye/Teye-Contracts/actions/workflows/ci.yml/badge.svg)](https://github.com/Stellar-Teye/Teye-Contracts/actions)

Stellar Teye is a decentralized smart contract system for secure, encrypted, and role-based management of vision care and optometry records on the Stellar blockchain using Soroban and Rust. The project enables eye care providers and patients to maintain control over sensitive vision data while ensuring privacy, immutability, and auditability. Built specifically for optometry clinics, ophthalmology practices, and vision research institutions transitioning to blockchain-based record keeping.

The platform provides a comprehensive solution for modern vision care data management, combining the security benefits of blockchain technology with practical healthcare workflows. It's designed for optometrists, ophthalmologists, vision research institutions, and patients who need to maintain confidentiality while enabling secure data sharing between authorized parties.

---

## Table of Contents

- [Project Overview](#-project-overview)
- [Setup Instructions](#-setup-instructions)
  - [Prerequisites](#-prerequisites)
  - [Quick Start](#-quick-start)
  - [Environment Setup](#-environment-setup)
  - [Running Tests](#-running-tests)
  - [Network Configuration](#-network-configuration)
- [Features](#-features)
- [Project Structure](#-project-structure)
- [Usage Examples](#-usage-examples)
- [Deployment](#-deployment)
- [CI/CD Pipeline](#-cicd-pipeline)
- [Helpful Links](#-helpful-links)
- [Contribution Guidelines](#-contribution-guidelines)
- [License](#-license)
- [Support](#-support)

---

## Project Overview

Stellar Teye transforms vision care record management by leveraging Stellar's blockchain infrastructure to create an immutable, secure, and patient-centric eye care data ecosystem. The system addresses critical healthcare challenges including data breaches, interoperability issues, and patient privacy concerns through cryptographic security and decentralized governance.

### Key Benefits

| Benefit | Description |
|---------|-------------|
| **Enhanced Security** | Military-grade encryption protects sensitive vision data |
| **Patient Control** | Patients grant and revoke access to their records |
| **Interoperability** | Standardized format enables seamless data exchange |
| **Audit Trail** | Complete, immutable history of all record access and modifications |
| **Vision-Specific** | Tailored for optometry and ophthalmology workflows |

### Target Users

- Optometry clinics and ophthalmology practices
- Vision research institutions
- Eye care insurance providers
- Patients seeking control over their vision data
- Optical retail chains and laboratories

---

## Setup Instructions

### Prerequisites

Before you begin, ensure you have the following installed:

- **Rust 1.78.0+** - [Install Rust](https://www.rust-lang.org/tools/install)
- **Soroban CLI v23.1.4+** - [Install Soroban](https://soroban.stellar.org/docs/getting-started/installation)
- **Git** - For version control
- **Make** - For using the provided Makefile (optional but recommended)

### Quick Start

Get up and running in under 5 minutes:

```bash
# Clone the repository
git clone https://github.com/Stellar-Teye/Teye-Contracts.git
cd Teye-Contracts

# Run the automated setup script
chmod +x setup.sh
./setup.sh

# Or use the Makefile for step-by-step setup
make setup
```

### Environment Setup

#### Option 1: Automated Setup (Recommended)

The `setup.sh` script handles everything automatically:

```bash
./setup.sh
```

This script will:
- Install Rust 1.78.0 and required targets
- Install Soroban CLI v23.1.4
- Set up project structure
- Configure Soroban networks (local, testnet, futurenet)
- Build the project and run tests
- Generate default identity

#### Option 2: Manual Setup

```bash
# Install Rust targets and components
rustup target add wasm32-unknown-unknown
rustup component add rustfmt clippy rust-src

# Install Soroban CLI
cargo install --locked soroban-cli

# Configure Soroban
soroban config identity generate default
soroban config network add local \
    --rpc-url http://localhost:8000/soroban/rpc \
    --network-passphrase "Standalone Network ; February 2017"

# Build the project
cargo build --all-targets

# Run tests to verify setup
cargo test --all
```

### Running Tests

Ensure everything is working correctly:

```bash
# Run all tests
make test

# Or use cargo directly
cargo test --all

# Run specific test types
make test-unit          # Unit tests only
make test-integration   # Integration tests only
```

### Network Configuration

The project supports multiple Stellar networks:

```bash
# Start local development network
make start-local
# or
soroban network start local

# Deploy to local network
make deploy-local

# Stop local network
make stop-local
```

**Available Networks:**

| Network | URL | Purpose |
|---------|-----|---------|
| Local | `http://localhost:8000/soroban/rpc` | Development |
| Testnet | `https://soroban-testnet.stellar.org:443` | Testing |
| Futurenet | `https://rpc-futurenet.stellar.org:443` | Staging |

---

## Features

- Encrypted on-chain vision care records storage
- Role-based access control (patients, optometrists, ophthalmologists, admins)
- Immutable timestamping and full history tracking
- Comprehensive eye exam and prescription data management
- Public key-based identity verification
- Fully testable, modular, and CI-enabled
- Gas-efficient contract design
- Decentralized governance with Governor + Timelock (proposals, voting, queued execution)
- Vision health analytics and trend tracking
- Multi-provider record sharing
- AI-powered vision analysis integration with provider verification and anomaly flagging

---

## Project Structure

```
Teye-Contracts/
│
├── contracts/
│   └── vision_records/
│       ├── src/
│       │   └── lib.rs           # Main contract logic
│       └── Cargo.toml           # Contract dependencies
│
├── scripts/                     # Deployment and interaction scripts
│   ├── deploy.sh               # Contract deployment
│   ├── interact.sh             # Contract interaction
│   └── test_scripts/           # Test utilities
│
├── tests/
│   ├── integration/            # Integration tests
│   └── unit/                   # Unit tests
│
├── docs/                       # Documentation
│   ├── api.md                  # API reference
│   └── architecture.md         # Architecture details
│
├── .github/
│   └── workflows/
│       └── ci.yml              # Continuous integration
│
├── setup.sh                    # Automated setup script
├── Makefile                    # Build automation
├── Dockerfile                  # Docker support
├── Cargo.toml                  # Workspace configuration
└── README.md                   # This file
```

---

## Usage Examples

### Basic Contract Interaction

```rust
// Register a new patient
let patient_id = client.register_patient(&env, &patient_address, &patient_data);

// Add a vision exam record
let record_id = client.add_exam_record(&env, &patient_id, &exam_data);

// Grant access to an optometrist
client.grant_access(&env, &patient_id, &optometrist_address, &AccessLevel::Read);

// Retrieve vision records
let records = client.get_patient_records(&env, &patient_id);
```

### Using the Makefile

```bash
# Build all contracts
make build

# Run all tests
make test

# Deploy to testnet
make deploy-testnet

# Generate documentation
make docs
```

---

## Deployment

### Local Development

```bash
make deploy-local
```

### Testnet Deployment

```bash
# Configure testnet identity
soroban config identity generate deployer

# Fund the account (use Stellar Laboratory)
# Deploy contracts
make deploy-testnet
```

### Production Deployment

```bash
# Ensure all tests pass
make test

# Build optimized contracts
make build-release

# Deploy to mainnet (requires proper key management)
make deploy-mainnet
```

---

## CI/CD Pipeline

### GitHub Actions Workflows

| Workflow | Trigger | Purpose |
|----------|---------|---------|
| `ci.yml` | Push/PR | Build, test, lint |
| `security.yml` | Push/PR/Daily | Security scanning |
| `deploy-testnet.yml` | Push to develop | Auto-deploy to testnet |
| `deploy-mainnet.yml` | Manual | Production deployment |

### Deployment Scripts

```bash
# Deploy with rollback support
./scripts/deploy_with_rollback.sh testnet

# Monitor deployment status
./scripts/monitor_deployments.sh

# Check deployment status
./scripts/deployment_status.sh
```

---

## Helpful Links

### Documentation

- [API Reference](docs/api.md)
- [Architecture Overview](docs/architecture.md)
- [Security Model](docs/security.md)

### Repository Resources

- [Contributing Guide](CONTRIBUTING.md)
- [Code of Conduct](CODE_OF_CONDUCT.md)
- [Changelog](CHANGELOG.md)

### External Resources

- [Stellar Developer Docs](https://developers.stellar.org/)
- [Soroban Documentation](https://soroban.stellar.org/docs)
- [Rust Documentation](https://doc.rust-lang.org/)

---

## Contribution Guidelines

We welcome contributions! Please follow these guidelines:

### Getting Started

1. **Fork** the repository
2. **Clone** your fork locally
3. **Create** a feature branch: `git checkout -b feature/your-feature`
4. **Make** your changes
5. **Test** your changes: `cargo test --all`
6. **Commit** with a descriptive message
7. **Push** to your fork
8. **Submit** a Pull Request

### Development Workflow

```bash
# Create a feature branch
git checkout -b feature/issue-number-description

# Make changes and test
cargo fmt
cargo clippy
cargo test --all

# Commit changes
git commit -m "feat: description of changes (closes #issue-number)"

# Push and create PR
git push origin feature/issue-number-description
```

### Code Standards

- Follow Rust style guidelines (`cargo fmt`)
- Pass all Clippy lints (`cargo clippy`)
- Write tests for new functionality
- Update documentation as needed
- Use conventional commit messages

### Review Process

1. All PRs require at least one approval
2. CI must pass before merging
3. Maintain test coverage above 80%
4. Address all review comments

### Definition of Done

- [ ] Code compiles without warnings
- [ ] All tests pass
- [ ] Documentation updated
- [ ] PR description is complete
- [ ] Linked to relevant issue

---

## License

This project is licensed under the MIT License - see the [LICENSE](LICENSE) file for details.

---

## Support

- **Issues**: [GitHub Issues](https://github.com/Stellar-Teye/Teye-Contracts/issues)
- **Discussions**: [GitHub Discussions](https://github.com/Stellar-Teye/Teye-Contracts/discussions)
- **Discord**: [Join our Discord](https://discord.gg/stellar-teye)

---

<p align="center">
  Built with on Stellar
</p>
