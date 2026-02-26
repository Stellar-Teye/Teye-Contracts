# Access Control Matrix

This document provides a comprehensive mapping of permissions across all contracts and user roles in the Stellar Teye platform.

## 🎭 Role Hierarchy

### User Roles

| Role | Description | Rank | Scope |
|------|-------------|-------|-------|
| **Patient** | Owner of health records | 1 | Personal data only |
| **Optometrist** | Eye care provider | 2 | Patient records with consent |
| **Ophthalmologist** | Eye surgeon/specialist | 3 | Extended medical access |
| **Provider** | Healthcare institution | 3 | Institutional access |
| **Admin** | System administrator | 4 | Contract configuration |
| **Governor** | Governance participant | 4 | Protocol governance |
| **SuperAdmin** | Platform administrator | 5 | Full system access |

### Admin Tiers (from `contracts/common/src/admin_tiers.rs`)

| Tier | Level | Capabilities |
|------|--------|-------------|
| **OperatorAdmin** | 1 | Pause/unpause operations |
| **ContractAdmin** | 2 | Contract configuration, user management |
| **SuperAdmin** | 3 | Full control, admin promotion/demotion |

## 📋 Permission Matrix

### Vision Records Contract (`contracts/vision_records/`)

| Function | Patient | Optometrist | Ophthalmologist | Admin | Governor | SuperAdmin | Conditions |
|-----------|----------|-------------|----------------|--------|-----------|-------------|-------------|
| `register_patient` | ✅ | ❌ | ❌ | ✅ | ❌ | ✅ Own data only |
| `update_patient_profile` | ✅ | ❌ | ❌ | ✅ | ❌ | ✅ Own data only |
| `add_vision_record` | ❌ | ✅ | ✅ | ✅ | ❌ | ⚠️ With patient consent |
| `update_vision_record` | ❌ | ✅ | ✅ | ✅ | ❌ | ⚠️ Own records only |
| `get_patient_records` | ✅ | ⚠️ | ⚠️ | ✅ | ❌ | ⚠️ With consent/authorization |
| `grant_access` | ✅ | ❌ | ❌ | ✅ | ❌ | ✅ Own data only |
| `revoke_access` | ✅ | ❌ | ❌ | ✅ | ❌ | ✅ Own data only |
| `emergency_access` | ❌ | ⚠️ | ⚠️ | ✅ | ❌ | ⚠️ Emergency justification |
| `initialize` | ❌ | ❌ | ❌ | ❌ | ❌ | ✅ SuperAdmin only |
| `pause` | ❌ | ❌ | ❌ | ⚠️ | ❌ | ✅ OperatorAdmin+ |
| `upgrade` | ❌ | ❌ | ❌ | ❌ | ❌ | ✅ SuperAdmin only |

### Governor Contract (`contracts/governor/`)

| Function | Patient | Optometrist | Ophthalmologist | Admin | Governor | SuperAdmin | Conditions |
|-----------|----------|-------------|----------------|--------|-----------|-------------|-------------|
| `create_proposal` | ❌ | ❌ | ❌ | ❌ | ✅ | ⚠️ Staking required |
| `vote` | ❌ | ❌ | ❌ | ❌ | ✅ | ⚠️ Token holder |
| `execute_proposal` | ❌ | ❌ | ❌ | ❌ | ✅ | ⚠️ After voting period |
| `delegate` | ❌ | ❌ | ❌ | ❌ | ✅ | ✅ Token holder |
| `initialize` | ❌ | ❌ | ❌ | ❌ | ❌ | ✅ SuperAdmin only |
| `pause` | ❌ | ❌ | ❌ | ⚠️ | ❌ | ✅ OperatorAdmin+ |

### Staking Contract (`contracts/staking/`)

| Function | Patient | Optometrist | Ophthalmologist | Admin | Governor | SuperAdmin | Conditions |
|-----------|----------|-------------|----------------|--------|-----------|-------------|-------------|
| `stake` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ Sufficient balance |
| `unstake` | ✅ | ✅ | ✅ | ✅ | ✅ | ⚠️ After lock period |
| `claim_rewards` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ Available rewards |
| `get_stake_info` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ Own data only |
| `update_reward_rate` | ❌ | ❌ | ❌ | ✅ | ❌ | ✅ ContractAdmin+ |
| `initialize` | ❌ | ❌ | ❌ | ❌ | ❌ | ✅ SuperAdmin only |

### Treasury Contract (`contracts/treasury/`)

| Function | Patient | Optometrist | Ophthalmologist | Admin | Governor | SuperAdmin | Conditions |
|-----------|----------|-------------|----------------|--------|-----------|-------------|-------------|
| `transfer_funds` | ❌ | ❌ | ❌ | ⚠️ | ❌ | ✅ ContractAdmin+ |
| `approve_spending` | ❌ | ❌ | ❌ | ❌ | ✅ | ⚠️ Governance approval |
| `get_balance` | ✅ | ✅ | ✅ | ✅ | ✅ | ✅ Public read |
| `initialize` | ❌ | ❌ | ❌ | ❌ | ❌ | ✅ SuperAdmin only |
| `emergency_withdraw` | ❌ | ❌ | ❌ | ⚠️ | ❌ | ✅ SuperAdmin only |

## 🔐 Progressive Authorization

### Auth Levels (from `contracts/common/src/progressive_auth.rs`)

| Level | Score Range | Requirements | Use Cases |
|-------|-------------|---------------|-----------|
| **Level 1** | 0-100 | Basic auth | Routine operations |
| **Level 2** | 101-500 | + Time delay | Sensitive operations |
| **Level 3** | 501-1000 | + Multisig | High-risk operations |
| **Level 4** | 1000+ | + ZK proof | Critical operations |

## 📝 References

- [Admin Tiers Implementation](../../contracts/common/src/admin_tiers.rs)
- [Progressive Auth Implementation](../../contracts/common/src/progressive_auth.rs)
- [Access Control Design ADR](../adr/0002-access-control-design.md)

---

**Last Updated**: 2025-02-25  
**Version**: 1.0
