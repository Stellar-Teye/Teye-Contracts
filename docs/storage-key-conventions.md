# Storage Key Conventions

Soroban storage keys defined using the `symbol_short!` macro are restricted to a maximum of **9 characters**.

## Restrictions
- **Length**: Maximum 9 characters.
- **Characters**: `a-z`, `A-Z`, `0-9`, and `_`.
- **Scope**: Keys must be unique within a single contract. Duplicate keys across different contracts are acceptable but should be avoided for shared logic.

## Naming Conventions
- Use uppercase for all storage keys.
- Use descriptive abbreviations if the name exceeds 9 characters.
- Avoid generic names like `DATA` or `KEY`.

### Examples
| Full Name | Short Key |
|-----------|-----------|
| `ADMINISTRATOR` | `ADMIN` |
| `INITIALIZED` | `INIT` |
| `PENDING_ADMIN` | `PEND_ADM` |
| `STAKE_TOKEN` | `STK_TOK` |
| `REWARD_RATE` | `RWD_RATE` |

## CI/CD Integration
Collisions are automatically checked in CI using `scripts/check_storage_keys.sh`.
