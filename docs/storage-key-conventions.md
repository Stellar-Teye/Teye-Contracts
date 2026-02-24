# Storage Key Naming Conventions

## Overview
Soroban storage keys using `symbol_short!` are limited to 9 characters. To prevent collisions, each contract should use unique, descriptive keys and reserved prefixes.

## Reserved Prefixes
- **staking**: STK_, ADMIN, INIT, STK_TOK, ...
- **vision_records**: VIS_, ADMIN, PAUSED, ...
- **zk_verifier**: ZKV_, AUDIT, ...

## Guidelines
- Keys must be unique within each contract.
- Use contract-specific prefixes (e.g., STK_, VIS_, ZKV_) for new keys.
- Avoid generic names (e.g., ADMIN, INIT) unless contextually unique.
- Document all keys in contract README or docs.

## Example
| Contract         | Key         | Purpose           |
|------------------|-------------|-------------------|
| staking          | ADMIN       | Admin address     |
| staking          | STK_TOK     | Staking token     |
| vision_records   | ADMIN       | Admin address     |
| vision_records   | PAUSED      | Pause state       |
| zk_verifier      | AUDIT       | Audit record      |

## Future-proofing
- Run `check_storage_keys.sh` in CI to catch collisions.
- Update this document when adding new keys.

## References
- [Soroban symbol_short! macro](https://docs.rs/soroban-sdk/latest/soroban_sdk/macro.symbol_short.html)
