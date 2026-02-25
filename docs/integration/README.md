# Teye-Contracts Integration Guide

Welcome to the Teye-Contracts integration guide! This document provides everything third-party developers need to interact with our Stellar-based smart contracts seamlessly.



## 🚀 Quickstart

To interact with Teye-Contracts, you need to connect to the Stellar RPC network and instantiate the contract using its ID.

**Prerequisites:**
* A Stellar Keypair (Funded on Testnet/Mainnet)
* The Teye-Contract ID: `C...[Insert Contract ID]`
* A Soroban RPC URL (e.g., `https://soroban-testnet.stellar.org`)

### Authentication Flow
Teye-Contracts strictly adhere to Stellar's native authentication (`soroban_auth`). Third-party apps should NEVER ask for a user's secret key.

1. **Frontend (Web3):** Use a wallet provider like [Freighter](https://freighter.app/) to request the user's signature for a transaction payload.
   👉 *See `example/js/freighter_auth.js` for a complete implementation.*
2. **Backend (Server-to-Server):** Use a dedicated server Keypair to sign transactions locally before submitting them to the Soroban RPC.
   👉 *See `example/js/invoke.js` for automated signing flows.*

---

## 💻 SDK Examples

Here is how you can invoke the Teye-Contracts across different tech stacks. You can find the full, runnable scripts in our `/examples` directory.

* **JavaScript:** `example/js/invoke.js` (Backend/Node.js) and `example/js/freighter_auth.js` (Frontend/React/Vanilla).
* **Python:** `example/python/webhook_listener.py` (Backend polling and automation).
* **Rust:** `example/rust/src/main.rs` (Native cross-contract integrations).

---

## 🪝 Webhook Integration (Listening to Events)

Teye-Contracts emit standard Soroban events when state changes occur. To build a third-party app that reacts to these changes (like a dashboard or notification system), you should poll the Soroban RPC `getEvents` endpoint.

**Webhook Implementation Steps:**
1. Store the `startLedger` of your last successful poll.
2. Query the RPC `getEvents` endpoint filtered by the Teye-Contract ID.
3. Process the returned data and update your local database.
4. Update your `startLedger` for the next cron cycle.

👉 *See `example/python/webhook_listener.py` for a complete polling implementation.*
