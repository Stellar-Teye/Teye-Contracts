# Integrating with Vision Records Events

The `VisionRecordsContract` uses Soroban's event framework to emit metadata alongside ledger changes. This document outlines how client applications (e.g., frontends, backend APIs) should listen to and handle these events.

## Why Use Events?

Events provide a lightweight, queryable history of actions without needing to replay entire transactions or store voluminous data on-chain. When building a UI for patients or providers:
- **Instant feedback**: Display "Access Granted" or "Record Added" toasts directly from the transaction result.
- **Audit Trails**: Construct a history of who accessed what and when by filtering the parsed events.

## Retrieving Events

Applications using `@stellar/stellar-sdk` or `@stellar/freighter-api` can retrieve events directly from the transaction receipt or by querying an RPC node for the contract's history.

### RPC Querying
Send a `getEvents` JSON-RPC request to a Soroban RPC node matching the `VisionRecordsContract` ID.
Filter by `startLedger` and `topics` arrays. For instance, to get all "User Registered" events, set the `topics` filter to `[ "USR_REG" ]`.

To get events for a specific patient, provide the patient's parsed `ScAddress` as the exact match on the second topic: `[ "REC_ADD", "CA...PATIENT_ID" ]`.

## Event Schemas

Below represent the logical structures of the `data` portion of the events:

### `InitializedEvent`
```typescript
interface InitializedEvent {
  admin: string; // ScAddress as string
  timestamp: number;
}
```

### `UserRegisteredEvent`
```typescript
interface UserRegisteredEvent {
  user: string; // ScAddress as string
  role: number; // Enum: 0 = Admin, 1 = Patient, 2 = Optometrist, 3 = Ophthalmologist
  name: string; // ScString
}
```

### `RecordAddedEvent`
```typescript
interface RecordAddedEvent {
  record_id: number; // u64
  patient: string; // ScAddress
  provider: string; // ScAddress
  record_type: number; // Enum: 0 = Examination, 1 = Prescription, etc.
}
```

### `AccessGrantedEvent`
```typescript
interface AccessGrantedEvent {
  patient: string;
  grantee: string;
  level: number; // Enum: 0 = None, 1 = Read, 2 = Write
  duration_seconds: number;
  expires_at: number;
}
```

### `AccessRevokedEvent`
```typescript
interface AccessRevokedEvent {
  patient: string;
  grantee: string;
}
```

## Parsing Events
Soroban returns events as `xdr.ContractEvent`. Use the Soroban SDK to decode the `xdr.ScVal` payload payloads back into the typed data structures defined above. Events with the `data` portion encoded as structs represent `xdr.ScMap` natively.

Example unpacking in JS:
```javascript
import { xdr, scValToNative } from '@stellar/stellar-sdk';

const decodedPayload = scValToNative(event.value());
console.log(decodedPayload.record_id);
```
