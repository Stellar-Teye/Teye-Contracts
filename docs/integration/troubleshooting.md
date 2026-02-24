# Troubleshooting Teye-Contracts Integrations

If you run into issues while integrating, check these common errors:

### 1. `tx_bad_seq` (Bad Sequence Number)
* **Cause:** You submitted multiple transactions from the same source account simultaneously, or your cached sequence number is outdated.
* **Fix:** Ensure you fetch the latest account state via `getAccount` immediately before building a new transaction. If sending in parallel, implement a queue system to manage sequence numbers.

### 2. `soroban_auth_failed`
* **Cause:** The transaction payload lacks the required signature for the address being modified in the contract.
* **Fix:** Ensure that if your contract uses `address.require_auth()`, the transaction is signed by that specific address, not just the fee-paying source account.

### 3. Exceeded Resource Limits (CPU/Memory)
* **Cause:** Your contract invocation exceeded the standard Soroban budget limits.
* **Fix:** Use the `simulateTransaction` RPC call before submitting. The simulation response will provide the exact resource requirements, which you can then append to your final assembled transaction.