# Teye SDK Quick Reference

This quick reference provides a comparison of SDK features and common operations across JavaScript, Python, and Rust implementations.

## Feature Comparison Matrix

| Feature | JavaScript/TypeScript | Python | Rust |
|---------|----------------------|--------|------|
| **Installation** | `npm install @stellar/stellar-sdk` | `pip install stellar-sdk` | `cargo add soroban-sdk` |
| **Async Support** | ✅ Native async/await | ✅ AsyncIO support | ✅ Async support |
| **Type Safety** | ✅ TypeScript types | ⚠️ Runtime typing | ✅ Compile-time safety |
| **ZK Integration** | ✅ External libraries | ✅ External libraries | ✅ Built-in ZK SDK |
| **Contract-to-Contract** | ❌ Not applicable | ❌ Not applicable | ✅ Native support |
| **Event Streaming** | ✅ Built-in event polling | ✅ Event polling | ✅ Event system |
| **Testing Framework** | Jest/Mocha | unittest/pytest | Built-in testutils |
| **Documentation** | ✅ JSDoc support | ✅ Docstrings | ✅ Rustdoc |
| **Bundle Size** | Medium | Small | Large (WASM) |

## Common Operations Cheat Sheet

### Connection Setup

| Operation | JavaScript | Python | Rust |
|-----------|-----------|--------|------|
| **Initialize SDK** | `new TeyeSDK("testnet")` | `TeyeSDK("testnet")` | `Client::new(&env, &contract_id)` |
| **Set Network** | `Networks.TESTNET` | `Network.TESTNET_NETWORK_PASSPHRASE` | `Env::default()` |
| **Load Account** | `rpc.getAccount(keypair.publicKey())` | `server.load_account(keypair.public_key)` | Not needed in contracts |

### Patient Management

| Operation | JavaScript | Python | Rust |
|-----------|-----------|--------|------|
| **Register Patient** | `patientManager.registerPatient(data)` | `patient_manager.register_patient(data)` | `teye_client.register_patient(...)` |
| **Get Profile** | `patientManager.getPatientProfile(id)` | `patient_manager.get_patient_profile(id)` | `teye_client.get_patient_profile(&id)` |
| **Update Profile** | `patientManager.updatePatientProfile(id, updates)` | `patient_manager.update_patient_profile(id, updates)` | `teye_client.update_patient_profile(&id, &updates)` |

### Vision Records

| Operation | JavaScript | Python | Rust |
|-----------|-----------|--------|------|
| **Add Record** | `recordsManager.addVisionRecord(data)` | `records_manager.add_vision_record(data)` | `teye_client.add_vision_record(...)` |
| **Get Records** | `recordsManager.getPatientRecords(id, type)` | `records_manager.get_patient_records(id, type)` | `teye_client.get_patient_records(&id)` |
| **Update Record** | `recordsManager.updateVisionRecord(id, updates)` | `records_manager.update_vision_record(id, updates)` | `teye_client.update_vision_record(&id, &updates)` |

### Access Control

| Operation | JavaScript | Python | Rust |
|-----------|-----------|--------|------|
| **Grant Access** | `accessManager.grantAccess(patient, requester, permissions, duration)` | `access_manager.grant_access(patient, requester, permissions, duration)` | `teye_client.grant_access(&patient, &requester, &access_request)` |
| **Revoke Access** | `accessManager.revokeAccess(patient, requester)` | `access_manager.revoke_access(patient, requester)` | `teye_client.revoke_access(&patient, &requester)` |
| **Check Access** | `accessManager.checkAccess(patient, requester, permission)` | `access_manager.check_access(patient, requester, permission)` | Custom implementation needed |

### Governance

| Operation | JavaScript | Python | Rust |
|-----------|-----------|--------|------|
| **Create Proposal** | `governanceManager.createProposal(data)` | `governance_manager.create_proposal(data)` | `teye_client.create_proposal(...)` |
| **Vote** | `governanceManager.vote(proposalId, voterId, vote, reason)` | `governance_manager.vote(proposal_id, voter_id, vote, reason)` | `teye_client.vote(&proposal_id, &voter_id, &vote, &reason)` |
| **Execute Proposal** | `governanceManager.executeProposal(proposalId)` | `governance_manager.execute_proposal(proposal_id)` | `teye_client.execute_proposal(&proposal_id)` |

### Staking

| Operation | JavaScript | Python | Rust |
|-----------|-----------|--------|------|
| **Stake** | `stakingManager.stake(amount, lockPeriod)` | `staking_manager.stake(amount, lock_period)` | `teye_client.stake(&amount, &lock_period)` |
| **Unstake** | `stakingManager.unstake(amount)` | `staking_manager.unstake(amount)` | `teye_client.unstake(&amount)` |
| **Get Balance** | `stakingManager.getStakeBalance(accountId)` | `staking_manager.get_stake_balance(account_id)` | `teye_client.get_stake_balance(&account_id)` |

## Data Type Mapping

| Concept | JavaScript | Python | Rust |
|---------|-----------|--------|------|
| **Address** | `String` | `str` | `Address` |
| **Timestamp** | `number` | `int` | `u64` |
| **Hash** | `string` | `str` | `BytesN<32>` |
| **Boolean** | `boolean` | `bool` | `bool` |
| **Integer** | `number` | `int` | `u64/i64` |
| **String** | `string` | `str` | `String` |
| **Array/List** | `Array<T>` | `list` | `Vec<T>` |
| **Map/Object** | `Object` | `dict` | `Map<K, V>` |

## Error Handling Patterns

### JavaScript/TypeScript
```javascript
try {
  const result = await patientManager.registerPatient(data);
  return result;
} catch (error) {
  if (error.message.includes("Simulation failed")) {
    throw new Error(`Contract error: ${error.message}`);
  }
  throw error;
}
```

### Python
```python
try:
    result = patient_manager.register_patient(data)
    return result
except Exception as e:
    if "Simulation failed" in str(e):
        raise Exception(f"Contract error: {e}")
    raise
```

### Rust
```rust
match teye_client.register_patient(&public_key, &name, &dob, &contact, &emergency) {
    Ok(result) => result,
    Err(e) => {
        env.events().publish(
            Symbol::new(&env, "error"),
            e.to_val(),
        );
        return Err(ContractError::RegistrationFailed);
    }
}
```

## Event Subscription

### JavaScript
```javascript
eventListener.subscribeToContractEvents(contractId, ["patient_registered"]);
eventListener.startEventPolling((event) => {
  console.log(`Event: ${event.eventType}`, event.value);
});
```

### Python
```python
event_listener.subscribe_to_events(contract_id, ["patient_registered"])
event_listener.start_polling(lambda event: print(f"Event: {event['event_type']}", event["value"]))
```

### Rust
```rust
// Events are handled through the Soroban event system
env.events().publish(
    Symbol::new(&env, "patient_registered"),
    patient_id.to_val(),
);
```

## Network Configuration

| Network | JavaScript RPC | Python RPC | Rust Env |
|---------|---------------|------------|----------|
| **Local** | `http://localhost:8000/soroban/rpc` | `http://localhost:8000/soroban/rpc` | `Env::default()` |
| **Testnet** | `https://soroban-testnet.stellar.org` | `https://soroban-testnet.stellar.org | Test configuration |
| **Mainnet** | `https://soroban.stellar.org` | `https://soroban.stellar.org` | Production config |

## Testing Quick Start

### JavaScript
```bash
npm test
# Using Jest
npm run test:integration
```

### Python
```bash
python -m unittest
# Using pytest
pytest tests/
```

### Rust
```bash
cargo test
# Integration tests
cargo test --test integration
```

## Common Gotchas

| Issue | JavaScript | Python | Rust |
|-------|-----------|--------|------|
| **Async/Await** | Required for all network calls | Use `asyncio` for async operations | Not needed in contracts |
| **Type Conversion** | `nativeToScVal()` / `scValToNative()` | `scval.to_primitive()` / `scval.from_native()` | Automatic with SDK |
| **Error XDR** | Manual decoding required | Manual decoding required | Built-in error types |
| **Gas Fees** | Set in transaction builder | Set in transaction builder | Handled by SDK |
| **Network Switching** | Change RPC URL and passphrase | Change RPC URL and passphrase | Change env configuration |

## Performance Tips

### JavaScript
- Use connection pooling for high-frequency calls
- Implement client-side caching for frequently accessed data
- Batch operations where possible

### Python
- Use async patterns for concurrent operations
- Implement connection reuse
- Consider using `uvloop` for better performance

### Rust
- Optimize storage layouts to reduce gas costs
- Use efficient data structures
- Minimize cross-contract calls

## Security Best Practices

1. **Never commit private keys** - Use environment variables
2. **Validate all inputs** - Both client-side and contract-side
3. **Implement rate limiting** - Prevent abuse
4. **Use proper error handling** - Don't leak sensitive information
5. **Regular security audits** - Especially for contract interactions
6. **Keep dependencies updated** - Address security vulnerabilities

## Links to Language-Specific Guides

- [JavaScript/TypeScript Guide](javascript.md)
- [Python Guide](python.md)
- [Rust Guide](rust.md)

## Additional Resources

- [Stellar SDK Documentation](https://stellar.github.io/js-stellar-sdk/)
- [Soroban Documentation](https://soroban.stellar.org/docs/)
- [Example Code](../../../example/)
- [Contract API Reference](../../api/)
- [Security Guidelines](../security/)
