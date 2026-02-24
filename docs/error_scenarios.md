# Error Handling & Negative Test Scenarios

## 1. Unauthorized Access

Expected Behavior:

- Caller not equal to owner
- Function returns: ContractError::Unauthorized

---

## 2. Invalid Input

Expected Behavior:

- Value exceeds allowed range
- Returns: ContractError::InvalidInput

---

## 3. Resource Exhaustion

Expected Behavior:

- Resource request exceeds threshold
- Returns: ContractError::ResourceExhausted

---

## 4. Race Conditions

Expected Behavior:

- Concurrent access protected via Mutex
- No data race
- Counter increments safely

---

All negative tests located in:

tests/negative/
