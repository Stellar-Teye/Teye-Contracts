# Data Flow Diagrams

This document contains sequence diagrams for critical Teye workflows.

## Table of Contents

- [1. Patient Registration to First Record to Access Delegation](#1-patient-registration-to-first-record-to-access-delegation)
- [2. Governance Proposal Lifecycle](#2-governance-proposal-lifecycle)
- [3. AI Analysis Pipeline](#3-ai-analysis-pipeline)
- [4. Cross-Chain Bridge Transfer](#4-cross-chain-bridge-transfer)
- [5. Emergency Access Grant and Auto-Revocation](#5-emergency-access-grant-and-auto-revocation)
- [6. ZK Proof Verification](#6-zk-proof-verification)

## 1. Patient Registration to First Record to Access Delegation

```mermaid
sequenceDiagram
  autonumber
  participant P as Patient
  participant Pr as Provider
  participant VR as vision_records
  participant ID as identity
  participant EV as events
  participant OFF as Off-chain Encrypted Storage

  P->>ID: initialize / identity registration
  P->>VR: register_user(patient)
  VR-->>EV: publish_user_registered

  Pr->>VR: register_user(provider)
  Pr->>OFF: encrypt exam payload
  Pr->>VR: add_record(patient, provider, data_hash)
  VR-->>EV: publish_record_added

  P->>VR: grant_access(patient, grantee, level, expires)
  VR-->>EV: publish_access_granted
  Pr->>VR: get_record(record_id)
  VR-->>Pr: hashed metadata + authorization result
```

## 2. Governance Proposal Lifecycle

```mermaid
sequenceDiagram
  autonumber
  participant M as Member/Staker
  participant GOV as governor
  participant STK as staking
  participant TIME as timelock
  participant TRE as treasury

  M->>GOV: create_proposal(actions)
  GOV->>STK: query_staked(voter)
  STK-->>GOV: stake + age data

  M->>GOV: commit_vote(proposal_id, commitment)
  M->>GOV: reveal_vote(choice, salt)
  GOV-->>GOV: tally/quorum/pass checks

  GOV->>TIME: queue execution (timelock phase)
  TIME-->>GOV: timelock expired
  GOV->>TRE: execute treasury/governance action
  TRE-->>GOV: execution success/failure
```

## 3. AI Analysis Pipeline

```mermaid
sequenceDiagram
  autonumber
  participant Pr as Provider
  participant AI as ai_integration
  participant INF as Off-chain AI Inference
  participant AN as analytics
  participant VR as vision_records

  Pr->>AI: submit_analysis_request(patient, record_id, input_hash)
  AI-->>INF: request event (off-chain listener)
  INF-->>INF: run model inference
  INF->>AI: submit_result(output_hash, confidence, anomaly_score)

  alt anomaly_score >= threshold
    AI-->>AI: mark request Flagged
    AI->>AN: aggregate flag metrics
  else normal
    AI-->>AI: status Completed
  end

  AI->>VR: optional downstream workflow trigger
  VR-->>Pr: authorized state/read result
```

## 4. Cross-Chain Bridge Transfer

```mermaid
sequenceDiagram
  autonumber
  participant SRC as Source Chain App
  participant REL as Relayer
  participant CC as cross_chain
  participant ZK as zk_verifier
  participant VR as vision_records
  participant DST as Destination Consumer

  SRC->>SRC: lock/export source state
  SRC-->>REL: signed transfer payload + proof bundle
  REL->>CC: process_message(message_id, payload)
  CC->>ZK: verify integrity/proof constraints
  ZK-->>CC: valid / invalid

  alt valid
    CC->>VR: grant/import record linkage
    CC->>DST: emit processed transfer event (mint/import equivalent)
  else invalid
    CC-->>REL: reject message
  end
```

## 5. Emergency Access Grant and Auto-Revocation

```mermaid
sequenceDiagram
  autonumber
  participant Prov as Emergency Provider
  participant Pat as Patient
  participant VR as vision_records
  participant EV as events
  participant JOB as Expiry Worker/Trigger

  Prov->>VR: grant_emergency_access(patient, condition, attestation, duration)
  VR-->>EV: EMRG_GRT event

  Prov->>VR: access_record_via_emergency(patient, record_id)
  VR-->>EV: EMRG_USE audit event
  VR-->>Prov: temporary authorized access

  alt explicit revocation
    Pat->>VR: revoke_emergency_access(access_id)
    VR-->>EV: EMRG_REV event
  else auto-expiry
    JOB->>VR: expire_emergency_accesses()
    VR-->>EV: expiration/revocation events
  end
```

## 6. ZK Proof Verification

```mermaid
sequenceDiagram
  autonumber
  participant U as User
  participant P as Off-chain Prover
  participant ZV as zk_verifier
  participant ID as identity
  participant APP as Integrator App

  U->>P: provide witness/private attributes
  P-->>U: Groth16 proof + public inputs
  U->>ZV: verify_access(request)
  ZV->>ZV: validate request shape + nonce + VK
  ZV->>ZV: verify proof components

  alt proof valid
    ZV-->>ID: optional credential path integration
    ZV-->>APP: success + audit record reference
  else proof invalid
    ZV-->>APP: rejection + AccessRejectedEvent
  end
```
