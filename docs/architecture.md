# 🏗️ Architecture Overview

## System Architecture

Stellar Teye is designed as a modular, secure, and scalable vision care records platform built on the Stellar blockchain using Soroban smart contracts.

## Core Components

### 1. Vision Records Contract

The main smart contract handling:
- Patient registration and management
- Vision record storage (encrypted hashes)
- Access control and permissions
- Provider authentication

### 2. Access Control Layer

Role-based access control (RBAC) system:
- **Patients**: Own their data, grant/revoke access
- **Optometrists**: Read/write access to granted records
- **Ophthalmologists**: Specialist access with additional privileges
- **Admins**: System administration and governance

### 3. Storage Layer

```
┌─────────────────────────────────────────────────────────┐
│                    Stellar Blockchain                     │
│  ┌─────────────────────────────────────────────────────┐  │
│  │              Soroban Smart Contracts                 │  │
│  │  ┌─────────────┐  ┌─────────────┐  ┌─────────────┐  │  │
│  │  │   Vision    │  │   Access    │  │  Governance │  │  │
│  │  │   Records   │  │   Control   │  │   (Future)  │  │  │
│  │  └─────────────┘  └─────────────┘  └─────────────┘  │  │
│  └─────────────────────────────────────────────────────┘  │
└─────────────────────────────────────────────────────────┘
```

## Data Flow

1. **Record Creation**:
   - Provider creates examination data
   - Data is encrypted off-chain
   - Hash is stored on-chain
   - Record ID is returned

2. **Record Access**:
   - Requester checks access level
   - If authorized, retrieves hash
   - Decrypts data off-chain

3. **Access Management**:
   - Patient grants access to provider
   - Access level and duration stored
   - Automatic expiration enforced

## Security Model

- **On-chain**: Access control, audit logs, hashes
- **Off-chain**: Encrypted data, private keys
- **No PHI on-chain**: Only hashes and metadata

## Future Extensions

- Governor contract for governance
- Timelock for delayed execution
- Multi-sig treasury
- Cross-chain bridges

## AI Integration Layer

The `ai_integration` contract extends the architecture with a controlled interface to AI vision systems:

- Provider registry with admin approval and status controls
- Request/response contracts for off-chain AI pipelines
- Result verification workflow before downstream consumption
- Threshold-based anomaly flagging for triage

See `docs/ai_integration.md` for API and data structures.
