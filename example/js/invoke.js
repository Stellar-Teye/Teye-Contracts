/**
 * Teye-Contracts: Node.js Integration Example
 * ----------------------------------------------
 * Demonstrates how to authenticate, simulate, and submit
 * a transaction to a Teye-Contract.
 * * Install: npm install @stellar/stellar-sdk dotenv
 */

import { Keypair, SorobanRpc, TransactionBuilder, Networks, Contract, nativeToScVal, scValToNative } from "@stellar/stellar-sdk";
import dotenv from "dotenv";

dotenv.config(); // Load environment variables

const RPC_URL = process.env.RPC_URL || "https://soroban-testnet.stellar.org";
const NETWORK_PASSPHRASE = process.env.NETWORK_PASSPHRASE || Networks.TESTNET;
const CONTRACT_ID = process.env.TEYE_CONTRACT_ID;

// DO NOT commit your secret key to source control!
// Provide it via environment variables instead.
const SECRET_KEY = process.env.SERVER_SECRET_KEY;

if (!CONTRACT_ID || !SECRET_KEY) {
    throw new Error("Missing required environment variables (TEYE_CONTRACT_ID or SERVER_SECRET_KEY). Please check your .env file.");
}

async function invokeTeyeContract() {
    console.log("Initializing connection...");
    const rpc = new SorobanRpc.Server(RPC_URL);
    const sourceKeypair = Keypair.fromSecret(SECRET_KEY);

    // 1. Fetch account state
    const account = await rpc.getAccount(sourceKeypair.publicKey());
    const contract = new Contract(CONTRACT_ID);

    // 2. Build transaction
    console.log("Building transaction...");
    const tx = new TransactionBuilder(account, {
        fee: "100",
        networkPassphrase: NETWORK_PASSPHRASE,
    })
    .addOperation(contract.call("create_record", nativeToScVal("Example Data")))
    .setTimeout(30)
    .build();

    // 3. Simulate to calculate fees
    console.log("Simulating transaction...");
    const simulation = await rpc.simulateTransaction(tx);
    if (SorobanRpc.Api.isSimulationError(simulation)) throw new Error(`Simulation failed: ${simulation.error}`);

    // 4. Assemble and Sign
    // FIX: Call .build() to convert the TransactionBuilder into a Transaction before signing
    const assembledTx = SorobanRpc.assembleTransaction(tx, simulation).build();
    assembledTx.sign(sourceKeypair);

    // 5. Submit
    console.log("Submitting to network...");
    const response = await rpc.sendTransaction(assembledTx);

    // FIX: Better error handling with XDR explanation
    if (response.status === "ERROR") {
        console.error(`❌ Submission failed. Raw Error XDR: ${response.errorResultXdr}`);
        console.error(`Decode this XDR at: https://laboratory.stellar.org/#xdr-viewer`);
        throw new Error("Transaction rejected by the network.");
    }

    // 6. Poll for success
    console.log(`Transaction sent! Hash: ${response.hash}. Polling...`);
    let txStatus = await rpc.getTransaction(response.hash);

    // FIX: Bounded retry guard to prevent infinite looping
    const startTime = Date.now();
    const TIMEOUT_MS = 30000; // 30 seconds limit

    while (txStatus.status === "NOT_FOUND") {
        if (Date.now() - startTime > TIMEOUT_MS) {
            throw new Error("Transaction polling timed out after 30 seconds.");
        }
        await new Promise(resolve => setTimeout(resolve, 2000));
        txStatus = await rpc.getTransaction(response.hash);
    }

    if (txStatus.status === "SUCCESS") {
        console.log(`✅ Success! Returned:`, scValToNative(txStatus.returnValue));
    } else {
        console.error("❌ Transaction failed on-chain.");
    }
}

invokeTeyeContract().catch(console.error);
