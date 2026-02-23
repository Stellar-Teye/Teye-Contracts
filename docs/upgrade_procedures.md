# Contract Upgrade Procedures

## 1. Versioning Strategy

- State structs are versioned (StateV1, StateV2)
- New fields must have default migration values
- No field removals without migration

---

## 2. Migration Steps

1. Deploy new contract version
2. Run migration logic
3. Validate state correctness
4. Enable new features

---

## 3. Authorization Rules

- Only owner may authorize upgrades
- Unauthorized attempts must fail

---

## 4. Testing Strategy

Upgrade tests cover:

- State migration correctness
- Backward compatibility
- Authorization validation
- Upgrade simulation

Located in:

tests/upgrade/
