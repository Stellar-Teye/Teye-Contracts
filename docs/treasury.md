# Treasury Management Contract

This document describes the on-chain treasury management system implemented in the `treasury` Soroban contract.

## Goals

- Provide a **multi-signature** treasury for platform funds.
- Support **spending proposals** with configurable thresholds.
- Track **fund allocation** by category for reporting and governance.
- Integrate cleanly with existing governance and monitoring.

## Design Overview

The treasury contract manages a single token balance (a SAC-compatible asset) held by the contract itself:

- **Configuration**:
  - `admin`: address with configuration authority (initially set at `initialize`).
  - `token`: address of the asset contract representing treasury funds.
  - `signers`: set of addresses allowed to create, approve, and execute proposals.
  - `threshold`: number of distinct signer approvals required to execute a proposal.
- **Spending Proposals**:
  - Created by any signer with:
    - Destination address
    - Amount
    - Category (for reporting)
    - Human-readable description
    - Expiration timestamp
  - Automatically records the proposer’s approval.
- **Execution**:
  - Requires at least `threshold` approvals from configured signers.
  - Transfers funds from the treasury to the destination.
  - Updates per-category allocation totals.

## Data Structures

- **`TreasuryConfig`**:
  - `admin: Address`
  - `token: Address`
  - `signers: Vec<Address>`
  - `threshold: u32`

- **`ProposalStatus`**:
  - `Pending`
  - `Executed`
  - `Expired`

- **`Proposal`**:
  - `id: u64`
  - `proposer: Address`
  - `to: Address`
  - `amount: i128`
  - `category: Symbol`
  - `description: String`
  - `approvals: Vec<Address>` (distinct signers)
  - `status: ProposalStatus`
  - `created_at: u64`
  - `expires_at: u64`

- **`AllocationSummary`**:
  - `category: Symbol`
  - `total_spent: i128`

## Key Contract Methods

- **Configuration**
  - `initialize(admin, token, signers, threshold)`:
    - Sets up treasury configuration.
    - Validations:
      - At least one signer.
      - `threshold` > 0 and `threshold <= signers.len()`.
    - May only be called once; subsequent calls panic with `"already initialized"`.

  - `get_config() -> TreasuryConfig`:
    - Returns the current configuration.

- **Proposals**
  - `create_proposal(proposer, to, amount, category, description, expires_at) -> Proposal`:
    - `proposer` must:
      - Be a configured signer.
      - Call with `require_auth()`.
    - Validations:
      - `amount > 0`
      - `expires_at` is strictly in the future.
    - Side effects:
      - Assigns a new monotonically increasing proposal ID.
      - Auto-approves by adding `proposer` to `approvals`.
      - Stores proposal with `status = Pending`.

  - `get_proposal(id) -> Option<Proposal>`:
    - Returns the stored proposal if present.

  - `approve_proposal(signer, id)`:
    - `signer` must:
      - Be a configured signer.
      - Call with `require_auth()`.
    - Validations:
      - Proposal exists and is `Pending`.
      - Proposal is not expired:
        - If expired at approval time, status is set to `Expired` and the call panics.
      - Duplicate approvals are ignored (no double counting).
    - Side effects:
      - Adds `signer` to the `approvals` vector if not already present.

- **Execution**
  - `execute_proposal(signer, id)`:
    - `signer` must be a configured signer and call with `require_auth()`.
    - Validations:
      - Proposal exists and is `Pending`.
      - Proposal is not expired (otherwise marked `Expired` and panics).
      - Number of distinct approvals ≥ `threshold`.
    - Side effects:
      - Transfers `amount` of `token` from the treasury contract address to `to`.
      - Marks proposal `status = Executed`.
      - Increments category-specific allocation:
        - `total_spent[category] += amount`.

- **Reporting**
  - `get_allocation_for_category(category) -> AllocationSummary`:
    - Returns how much has been spent for `category` across all executed proposals.

## Example Workflow

1. **Configure Treasury**

   - Deploy the treasury contract and call:

     ```text
     initialize(admin, token, [signer1, signer2, signer3], threshold = 2)
     ```

2. **Create Proposal**

   - `signer1` proposes:

     ```text
     create_proposal(
       proposer   = signer1,
       to         = vendor_address,
       amount     = 1_000,
       category   = "OPS",
       description= "Operations budget for Q2",
       expires_at = <timestamp>
     )
     ```

   - Proposal is stored with ID `N` and already has one approval (from `signer1`).

3. **Approve Proposal**

   - `signer2` calls:

     ```text
     approve_proposal(signer2, N)
     ```

4. **Execute Proposal**

   - Once approvals ≥ threshold (2 in this example), any signer can call:

     ```text
     execute_proposal(signer1, N)
     ```

   - Tokens are transferred from treasury to `vendor_address`, the proposal becomes `Executed`, and the `"OPS"` allocation is incremented.

5. **Report Allocation**

   - To retrieve total operations spend:

     ```text
     get_allocation_for_category("OPS") -> AllocationSummary { total_spent, ... }
     ```

## Security and Governance Considerations

- Treasury holds potentially significant funds; key practices:
  - Use **hardware-backed** keys for all signers.
  - Combine on-chain multi-sig with off-chain governance (e.g., Governor + Timelock).
  - Configure `threshold` according to risk (e.g., 2-of-3 or 3-of-5).
- Monitoring:
  - Use the existing monitoring stack (`docs/monitoring.md`) to:
    - Track treasury proposal execution events.
    - Alert on large or unusual outflows by category.

## Testing

Treasury behaviour is covered by unit tests in:

- `contracts/treasury/src/test.rs`

These tests verify:

- Proper initialization and configuration
- End-to-end flow: create → approve → execute proposal
- Correct token transfers for executed proposals
- Allocation tracking for categories
- Failure behaviour for expired proposals

