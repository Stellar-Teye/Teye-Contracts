# Deployment Security

This document describes the least-privilege deployment procedure for Teye
contracts. The goal is to ensure the deployer key never retains permanent admin
access after initialization completes.

## Overview

Soroban contracts are initialized with an admin address that controls
privileged operations (user management, provider verification, error log
clearing). A compromised admin key puts the entire contract at risk.

The secure deployment workflow uses a **temporary deployer key** for the
one-time deployment and initialization, then immediately transfers admin
rights to a **permanent admin key** stored in a hardware wallet or
multi-signature account.

## Key Ceremony

### 1. Generate the permanent admin key

Create the permanent admin keypair on a secure, air-gapped machine or
hardware wallet. Record the public address only.

```bash
# Example using soroban CLI on an air-gapped machine
soroban keys generate permanent-admin
soroban keys address permanent-admin
# -> GBXXX...  (the permanent admin public address)
```

Store the secret key in a hardware wallet or encrypted vault. It should
never exist on the deployment machine.

### 2. Generate a temporary deployer key

On the deployment machine, create a disposable keypair used only for this
deployment session.

```bash
soroban keys generate deployer-tmp
soroban keys address deployer-tmp
```

Fund the deployer key with enough XLM to cover deployment fees.

### 3. Deploy and transfer

Use the `--admin` flag to deploy, initialize, and transfer in one step:

```bash
./scripts/deploy.sh testnet vision_records --admin GBXXX...
```

Or use the Makefile target:

```bash
ADMIN_ADDRESS=GBXXX... make deploy-secure
```

The script will:
1. Build and deploy the WASM binary using the deployer key.
2. Initialize the contract with the deployer as temporary admin.
3. Call `transfer_admin` to hand off admin rights to the permanent address.
4. Verify the transfer by querying `get_admin`.

### 4. Verify the transfer

Confirm the admin was transferred:

```bash
soroban contract invoke \
    --id <CONTRACT_ID> \
    --network testnet \
    -- \
    get_admin
```

The output must show the permanent admin address, not the deployer.

### 5. Destroy the deployer key

After verifying the transfer, remove the temporary deployer key from the
deployment machine:

```bash
soroban keys rm deployer-tmp
```

## What `transfer_admin` does

The `transfer_admin` contract function:

- Requires the current admin to authenticate the call.
- Updates the `ADMIN` storage key to the new address.
- Grants the `Admin` RBAC role to the new address.
- Downgrades the old admin to `Patient` role (no system permissions).

After transfer, the deployer key cannot perform any privileged operations.

## Manual deployment (without `--admin`)

If you run `deploy.sh` without `--admin`, the contract is deployed but **not
initialized**. You must initialize and transfer manually:

```bash
# Initialize with deployer as temp admin
soroban contract invoke \
    --id <CONTRACT_ID> \
    --source deployer-tmp \
    --network testnet \
    -- \
    initialize --admin <DEPLOYER_ADDRESS>

# Transfer to permanent admin
soroban contract invoke \
    --id <CONTRACT_ID> \
    --source deployer-tmp \
    --network testnet \
    -- \
    transfer_admin \
    --current_admin <DEPLOYER_ADDRESS> \
    --new_admin <PERMANENT_ADMIN_ADDRESS>
```

## Mainnet checklist

Before deploying to mainnet:

- [ ] Permanent admin key generated on air-gapped hardware.
- [ ] Deployer key is a fresh, single-use keypair.
- [ ] Deployment uses `--admin` flag or equivalent manual transfer.
- [ ] `get_admin` returns the permanent admin address post-deployment.
- [ ] Deployer key deleted from all machines after successful transfer.
- [ ] Deployment record saved in `deployments/` with `admin_transferred: true`.
