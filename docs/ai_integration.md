# AI Integration Contract

This document describes the `ai_integration` Soroban contract used to connect vision record workflows with AI analysis providers.

## Objectives

- Define an on-chain contract for AI analysis requests.
- Register and manage approved AI providers.
- Persist analysis results with explicit verification state.
- Flag high-risk outcomes through anomaly detection thresholds.

## Architecture

The contract separates the AI lifecycle into four stages:

1. Provider registration.
2. Analysis request submission.
3. Analysis result persistence.
4. Human or governance verification.

Only hashes and metadata are stored on-chain. Raw images and model outputs remain off-chain.

## Core Types

- `AiProvider`
  - Provider metadata and operational status.
  - Includes an `operator` address authorized to submit analysis results.
- `AnalysisRequest`
  - Input contract for AI processing (record reference, task type, input hash).
  - Tracks request status (`Pending`, `Completed`, `Flagged`, `Rejected`).
- `AnalysisResult`
  - Output contract for AI processing (output hash, confidence score, anomaly score).
  - Tracks verification (`Unverified`, `Verified`, `Rejected`).

## Contract API

- `initialize(admin, anomaly_threshold_bps)`
- `register_provider(caller, provider_id, operator, name, model, endpoint_hash)`
- `set_provider_status(caller, provider_id, status)`
- `submit_analysis_request(caller, provider_id, patient, record_id, input_hash, task_type)`
- `store_analysis_result(caller, request_id, output_hash, confidence_bps, anomaly_score_bps)`
- `verify_analysis_result(caller, request_id, accepted, verification_hash)`
- Query methods:
  - `get_provider`
  - `get_analysis_request`
  - `get_analysis_result`
  - `get_flagged_requests`
  - `get_anomaly_threshold`

## Verification and Trust Model

- Admin controls provider registration and global anomaly threshold.
- Provider `operator` address is the only actor allowed to submit result payloads for that provider.
- Verification is performed by admin, allowing separation between AI inference and final acceptance.
- Rejected verification sets request status to `Rejected`.

## Anomaly Detection Integration

- `anomaly_threshold_bps` is configured in basis points (`0..=10_000`).
- Results with `anomaly_score_bps >= anomaly_threshold_bps` are marked `Flagged`.
- Flagged request IDs are indexed for downstream triage workflows.

## Testing

Coverage includes:

- Provider registration and status management.
- End-to-end request and result flow.
- Verification transitions.
- Anomaly threshold flagging behavior.
- Admin authorization checks.

Tests live in:

- `contracts/ai_integration/src/test.rs`
- `contracts/ai_integration/tests/core.rs`
