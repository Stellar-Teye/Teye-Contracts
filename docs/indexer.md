# Tracking Vision Records Events

The `VisionRecordsContract` natively emits events for critical state-changing actions to assist off-chain indexers and user interfaces. This guide will clarify the event subjects, topics, and payload structures for developers indexing the ledger.

## Overview

Events emitted by this contract contain up to three topics:
- **Topic 0**: Identifies the action type (always a `Symbol`).
- **Topic 1**: Primary associated identity (usually the user, admin, or patient).
- **Topic 2**: Secondary associated entity (e.g. grantee or provider).

Payload data comes in the form of strongly-typed structs.

## Emitted Events

### 1. Contract Initialized (`INIT`)
Fired exactly once when the registry is bootstrapped with an admin.
- **Topics**: `[Symbol("INIT")]`
- **Payload**:
  ```rust
  {
      admin: Address,
      timestamp: u64
  }
  ```

### 2. User Registered (`USR_REG`)
Fired when the admin sets up a new patient or provider.
- **Topics**: `[Symbol("USR_REG"), user: Address]`
- **Payload**:
  ```rust
  {
      user: Address,
      role: Role,
      name: String
  }
  ```

### 3. Record Added (`REC_ADD`)
Fired when a provider registers a vision record under a patient.
- **Topics**: `[Symbol("REC_ADD"), patient: Address, provider: Address]`
- **Payload**:
  ```rust
  {
      record_id: u64,
      patient: Address,
      provider: Address,
      record_type: RecordType
  }
  ```

### 4. Access Granted (`ACC_GRT`)
Fired when a patient delegates access to another party.
- **Topics**: `[Symbol("ACC_GRT"), patient: Address, grantee: Address]`
- **Payload**:
  ```rust
  {
      patient: Address,
      grantee: Address,
      level: AccessLevel,
      duration_seconds: u64,
      expires_at: u64
  }
  ```

### 5. Access Revoked (`ACC_REV`)
Fired when a patient rescinds a previous delegation.
- **Topics**: `[Symbol("ACC_REV"), patient: Address, grantee: Address]`
- **Payload**:
  ```rust
  {
      patient: Address,
      grantee: Address
  }
  ```

## Indexing Strategy
Indexers should specifically listen for the smart contract's `contract_id` on the ledger, parsing occurrences of `ContractEvent` elements matching these exact predefined topics. Parsing the `data` portion requires decoding the `Val` objects to represent the structured maps natively represented by Soroban structures.
