# JavaScript/TypeScript SDK Integration Guide

This guide demonstrates how to integrate Stellar Teye contracts into JavaScript/TypeScript applications using the Stellar SDK.

## Prerequisites

- **Node.js**: Version 18.0 or higher
- **Package Manager**: npm or yarn
- **Stellar Account**: Funded account on the target network

### Installation

```bash
npm install @stellar/stellar-sdk dotenv
# or
yarn add @stellar/stellar-sdk dotenv
```

### Environment Setup

Create a `.env` file in your project root:

```env
# Network Configuration
RPC_URL=https://soroban-testnet.stellar.org
NETWORK_PASSPHRASE=Test SDF Network ; September 2015

# Contract Configuration
TEYE_CONTRACT_ID=YOUR_CONTRACT_ID_HERE
VISION_RECORDS_CONTRACT_ID=YOUR_VISION_RECORDS_CONTRACT_ID
GOVERNOR_CONTRACT_ID=YOUR_GOVERNOR_CONTRACT_ID

# Authentication
SERVER_SECRET_KEY=your_secret_key_here
# NEVER commit this to version control!
```

## Connection Setup

### Connecting to Different Networks

```javascript
import { SorobanRpc, Networks } from "@stellar/stellar-sdk";

const networks = {
  local: {
    rpc: "http://localhost:8000/soroban/rpc",
    passphrase: Networks.STANDALONE,
  },
  testnet: {
    rpc: "https://soroban-testnet.stellar.org",
    passphrase: Networks.TESTNET,
  },
  mainnet: {
    rpc: "https://soroban.stellar.org",
    passphrase: Networks.PUBLIC,
  },
};

function createServer(network = "testnet") {
  const config = networks[network];
  return new SorobanRpc.Server(config.rpc);
}
```

### Basic Connection Example

```javascript
import { Keypair, SorobanRpc, TransactionBuilder, Networks, Contract, nativeToScVal, scValToNative } from "@stellar/stellar-sdk";
import dotenv from "dotenv";

dotenv.config();

class TeyeSDK {
  constructor(network = "testnet") {
    this.rpc = new SorobanRpc.Server(process.env.RPC_URL);
    this.networkPassphrase = network === "testnet" ? Networks.TESTNET : Networks.PUBLIC;
    this.contractId = process.env.TEYE_CONTRACT_ID;
    this.keypair = Keypair.fromSecret(process.env.SERVER_SECRET_KEY);
  }

  async getAccount() {
    return await this.rpc.getAccount(this.keypair.publicKey());
  }

  async buildAndSignTransaction(contractMethod, ...args) {
    const account = await this.getAccount();
    const contract = new Contract(this.contractId);
    
    const scArgs = args.map(arg => nativeToScVal(arg));
    
    const tx = new TransactionBuilder(account, {
      fee: "100",
      networkPassphrase: this.networkPassphrase,
    })
    .addOperation(contract.call(contractMethod, ...scArgs))
    .setTimeout(30)
    .build();

    const simulation = await this.rpc.simulateTransaction(tx);
    if (SorobanRpc.Api.isSimulationError(simulation)) {
      throw new Error(`Simulation failed: ${simulation.error}`);
    }

    const assembledTx = SorobanRpc.assembleTransaction(tx, simulation).build();
    assembledTx.sign(this.keypair);
    
    return assembledTx;
  }

  async submitTransaction(tx) {
    const response = await this.rpc.sendTransaction(tx);
    
    if (response.status === "ERROR") {
      console.error(`❌ Submission failed. Raw Error XDR: ${response.errorResultXdr}`);
      throw new Error("Transaction rejected by the network.");
    }

    // Poll for transaction completion
    let txStatus = await this.rpc.getTransaction(response.hash);
    const startTime = Date.now();
    const TIMEOUT_MS = 30000;

    while (txStatus.status === "NOT_FOUND") {
      if (Date.now() - startTime > TIMEOUT_MS) {
        throw new Error("Transaction polling timed out after 30 seconds.");
      }
      await new Promise(resolve => setTimeout(resolve, 2000));
      txStatus = await this.rpc.getTransaction(response.hash);
    }

    if (txStatus.status === "SUCCESS") {
      return scValToNative(txStatus.returnValue);
    } else {
      throw new Error("Transaction failed on-chain.");
    }
  }
}
```

## Contract Interaction Patterns

### Patient Registration and Profile Management

```javascript
class PatientManager extends TeyeSDK {
  async registerPatient(patientData) {
    const {
      publicKey,
      name,
      dateOfBirth,
      contactInfo,
      emergencyContact
    } = patientData;

    const tx = await this.buildAndSignTransaction(
      "register_patient",
      publicKey,
      name,
      dateOfBirth,
      contactInfo,
      emergencyContact
    );

    return await this.submitTransaction(tx);
  }

  async updatePatientProfile(patientId, updates) {
    const tx = await this.buildAndSignTransaction(
      "update_patient_profile",
      patientId,
      updates
    );

    return await this.submitTransaction(tx);
  }

  async getPatientProfile(patientId) {
    const contract = new Contract(this.contractId);
    const result = await this.rpc.getContractData(
      contract.getContractId(),
      nativeToScVal(patientId)
    );
    
    return scValToNative(result.val);
  }
}
```

### Vision Records Management

```javascript
class VisionRecordsManager extends TeyeSDK {
  async addVisionRecord(recordData) {
    const {
      patientId,
      providerId,
      recordType, // "exam", "prescription", "treatment"
      encryptedDataHash,
      metadata
    } = recordData;

    const tx = await this.buildAndSignTransaction(
      "add_vision_record",
      patientId,
      providerId,
      recordType,
      encryptedDataHash,
      metadata
    );

    return await this.submitTransaction(tx);
  }

  async updateVisionRecord(recordId, updates) {
    const tx = await this.buildAndSignTransaction(
      "update_vision_record",
      recordId,
      updates
    );

    return await this.submitTransaction(tx);
  }

  async getPatientRecords(patientId, recordType = null) {
    const contract = new Contract(this.contractId);
    const key = recordType 
      ? `patient_records_${patientId}_${recordType}`
      : `patient_records_${patientId}`;
    
    const result = await this.rpc.getContractData(
      contract.getContractId(),
      nativeToScVal(key)
    );
    
    return scValToNative(result.val);
  }
}
```

### Access Control Management

```javascript
class AccessControlManager extends TeyeSDK {
  async grantAccess(patientId, requesterId, permissions, duration = null) {
    const accessRequest = {
      patient_id: patientId,
      requester_id: requesterId,
      permissions: permissions, // ["read", "write", "share"]
      expires_at: duration ? Date.now() + duration : null,
      granted_at: Date.now()
    };

    const tx = await this.buildAndSignTransaction(
      "grant_access",
      patientId,
      requesterId,
      accessRequest
    );

    return await this.submitTransaction(tx);
  }

  async revokeAccess(patientId, requesterId) {
    const tx = await this.buildAndSignTransaction(
      "revoke_access",
      patientId,
      requesterId
    );

    return await this.submitTransaction(tx);
  }

  async checkAccess(patientId, requesterId, permission = "read") {
    const contract = new Contract(this.contractId);
    const accessKey = `access_${patientId}_${requesterId}`;
    
    try {
      const result = await this.rpc.getContractData(
        contract.getContractId(),
        nativeToScVal(accessKey)
      );
      
      const accessData = scValToNative(result.val);
      return accessData.permissions.includes(permission) && 
             (!accessData.expires_at || accessData.expires_at > Date.now());
    } catch (error) {
      return false; // No access granted
    }
  }
}
```

### Governance Operations

```javascript
class GovernanceManager extends TeyeSDK {
  async createProposal(proposalData) {
    const {
      title,
      description,
      type, // "parameter_change", "contract_upgrade", "policy_update"
      targetContract,
      proposedChanges
    } = proposalData;

    const tx = await this.buildAndSignTransaction(
      "create_proposal",
      title,
      description,
      type,
      targetContract,
      proposedChanges
    );

    return await this.submitTransaction(tx);
  }

  async vote(proposalId, voterId, vote, reason = "") {
    const tx = await this.buildAndSignTransaction(
      "vote",
      proposalId,
      voterId,
      vote, // "for", "against", "abstain"
      reason
    );

    return await this.submitTransaction(tx);
  }

  async executeProposal(proposalId) {
    const tx = await this.buildAndSignTransaction(
      "execute_proposal",
      proposalId
    );

    return await this.submitTransaction(tx);
  }
}
```

### Staking Operations

```javascript
class StakingManager extends TeyeSDK {
  async stake(amount, lockPeriod = null) {
    const tx = await this.buildAndSignTransaction(
      "stake",
      amount,
      lockPeriod || 0
    );

    return await this.submitTransaction(tx);
  }

  async unstake(amount) {
    const tx = await this.buildAndSignTransaction(
      "unstake",
      amount
    );

    return await this.submitTransaction(tx);
  }

  async getStakeBalance(accountId) {
    const contract = new Contract(this.contractId);
    const result = await this.rpc.getContractData(
      contract.getContractId(),
      nativeToScVal(`stake_${accountId}`)
    );
    
    return scValToNative(result.val);
  }
}
```

## Error Handling

### Comprehensive Error Handling

```javascript
class TeyeErrorHandler {
  static handleSimulationError(error) {
    if (error.error.includes("insufficient_fee")) {
      return new Error("Insufficient transaction fee. Please increase the fee amount.");
    }
    if (error.error.includes("no_account")) {
      return new Error("Account not found. Please ensure the account exists and is funded.");
    }
    if (error.error.includes("contract_error")) {
      return new Error(`Contract error: ${error.error}`);
    }
    return new Error(`Simulation failed: ${error.error}`);
  }

  static handleSubmissionError(response) {
    if (response.status === "ERROR") {
      // Try to decode the error XDR for more details
      try {
        const errorData = StellarBase.xdr.TransactionResult.fromXDR(response.errorResultXdr, 'base64');
        return new Error(`Transaction failed: ${errorData.result().results()[0].value()}`);
      } catch {
        return new Error(`Transaction failed. Decode this XDR for details: https://laboratory.stellar.org/#xdr-viewer?input=${response.errorResultXdr}`);
      }
    }
    return new Error("Unknown transaction error");
  }

  static handleNetworkError(error) {
    if (error.code === 'ECONNREFUSED') {
      return new Error("Unable to connect to Stellar network. Please check your network connection.");
    }
    if (error.code === 'ETIMEDOUT') {
      return new Error("Network request timed out. Please try again.");
    }
    return error;
  }
}

// Usage in SDK methods
try {
  const result = await this.submitTransaction(tx);
  return result;
} catch (error) {
  if (error.message.includes("Simulation failed")) {
    throw TeyeErrorHandler.handleSimulationError(error);
  }
  if (error.message.includes("Transaction failed")) {
    throw TeyeErrorHandler.handleSubmissionError(error);
  }
  throw TeyeErrorHandler.handleNetworkError(error);
}
```

## Event Subscription

### Listening for Contract Events

```javascript
class EventListener extends TeyeSDK {
  constructor(network = "testnet") {
    super(network);
    this.eventFilters = new Map();
    this.isListening = false;
  }

  subscribeToContractEvents(contractId, eventTypes = []) {
    const filter = {
      type: "contract",
      contractIds: [contractId],
      topics: eventTypes.length > 0 ? eventTypes : undefined,
    };
    
    this.eventFilters.set(contractId, filter);
  }

  async startEventPolling(callback, pollInterval = 5000) {
    if (this.isListening) {
      throw new Error("Event polling is already active");
    }

    this.isListening = true;
    let lastLedger = (await this.rpc.getLatestLedger()).sequence;

    const poll = async () => {
      if (!this.isListening) return;

      try {
        for (const [contractId, filter] of this.eventFilters) {
          const events = await this.rpc.getEvents({
            startLedger: lastLedger,
            filters: [filter],
            limit: 100,
          });

          if (events.events.length > 0) {
            for (const event of events.events) {
              callback({
                contractId,
                eventType: event.type,
                topic: event.topic,
                value: scValToNative(event.value),
                ledger: event.ledger,
                timestamp: event.timestamp,
              });
            }
          }
        }

        lastLedger = (await this.rpc.getLatestLedger()).sequence;
      } catch (error) {
        console.error("Event polling error:", error);
      }

      setTimeout(poll, pollInterval);
    };

    poll();
  }

  stopEventPolling() {
    this.isListening = false;
  }
}

// Example usage
const eventListener = new EventListener("testnet");
eventListener.subscribeToContractEvents(process.env.TEYE_CONTRACT_ID, [
  "patient_registered",
  "record_added",
  "access_granted",
  "access_revoked"
]);

eventListener.startEventPolling((event) => {
  console.log(`🔔 ${event.eventType}:`, event.value);
  
  switch (event.eventType) {
    case "patient_registered":
      // Handle new patient registration
      break;
    case "record_added":
      // Handle new vision record
      break;
    case "access_granted":
      // Handle access grant
      break;
    case "access_revoked":
      // Handle access revocation
      break;
  }
});
```

## Full Working Example

### End-to-End Patient Registration → Record Creation → Access Grant Flow

```javascript
import dotenv from "dotenv";
dotenv.config();

class TeyeHealthcareApp {
  constructor() {
    this.sdk = new TeyeSDK("testnet");
    this.patientManager = new PatientManager("testnet");
    this.recordsManager = new VisionRecordsManager("testnet");
    this.accessManager = new AccessControlManager("testnet");
  }

  async onboardNewPatient(patientData) {
    try {
      console.log("🏥 Starting patient onboarding...");
      
      // Step 1: Register patient
      console.log("📝 Registering patient...");
      const patientResult = await this.patientManager.registerPatient(patientData);
      console.log("✅ Patient registered:", patientResult);

      // Step 2: Create initial vision record
      console.log("👁️ Creating initial vision record...");
      const visionRecord = {
        patientId: patientData.publicKey,
        providerId: process.env.PROVIDER_PUBLIC_KEY,
        recordType: "exam",
        encryptedDataHash: "0x" + Array(64).fill(0).map(() => Math.floor(Math.random() * 16).toString(16)).join(''),
        metadata: {
          examDate: new Date().toISOString(),
          examType: "comprehensive",
          providerName: "Dr. Smith"
        }
      };
      
      const recordResult = await this.recordsManager.addVisionRecord(visionRecord);
      console.log("✅ Vision record created:", recordResult);

      // Step 3: Grant access to primary care provider
      console.log("🔐 Granting access to provider...");
      const accessResult = await this.accessManager.grantAccess(
        patientData.publicKey,
        process.env.PROVIDER_PUBLIC_KEY,
        ["read", "write"],
        30 * 24 * 60 * 60 * 1000 // 30 days
      );
      console.log("✅ Access granted:", accessResult);

      return {
        patient: patientResult,
        record: recordResult,
        access: accessResult
      };

    } catch (error) {
      console.error("❌ Onboarding failed:", error.message);
      throw error;
    }
  }

  async getPatientSummary(patientId) {
    try {
      console.log("📊 Retrieving patient summary...");
      
      // Get patient profile
      const profile = await this.patientManager.getPatientProfile(patientId);
      
      // Get all vision records
      const records = await this.recordsManager.getPatientRecords(patientId);
      
      // Get access permissions
      const accessList = await this.getAccessList(patientId);

      return {
        profile,
        records,
        accessList
      };

    } catch (error) {
      console.error("❌ Failed to retrieve patient summary:", error.message);
      throw error;
    }
  }

  async getAccessList(patientId) {
    // This would typically query all access grants for a patient
    // Implementation depends on your contract's data structure
    return [];
  }
}

// Usage example
async function main() {
  const app = new TeyeHealthcareApp();
  
  const patientData = {
    publicKey: "GABCDEFGHIJKLMNOPQRSTUVWXYZ123456789",
    name: "John Doe",
    dateOfBirth: "1990-01-01",
    contactInfo: {
      email: "john.doe@example.com",
      phone: "+1-555-0123"
    },
    emergencyContact: {
      name: "Jane Doe",
      phone: "+1-555-0124",
      relationship: "spouse"
    }
  };

  try {
    // Onboard new patient
    const onboardingResult = await app.onboardNewPatient(patientData);
    console.log("🎉 Patient onboarded successfully!");

    // Get patient summary
    const summary = await app.getPatientSummary(patientData.publicKey);
    console.log("📋 Patient summary:", summary);

  } catch (error) {
    console.error("💥 Application error:", error);
  }
}

// Run the example
if (require.main === module) {
  main().catch(console.error);
}

export default TeyeHealthcareApp;
```

## Testing

### Unit Testing with Jest

```javascript
import { TeyeSDK } from '../src/teye-sdk';

describe('TeyeSDK', () => {
  let sdk;
  
  beforeEach(() => {
    sdk = new TeyeSDK('testnet');
  });

  test('should create connection to testnet', () => {
    expect(sdk.rpc.serverUrl).toBe('https://soroban-testnet.stellar.org');
    expect(sdk.networkPassphrase).toBe('Test SDF Network ; September 2015');
  });

  test('should build transaction correctly', async () => {
    const tx = await sdk.buildAndSignTransaction('test_method', 'test_arg');
    expect(tx).toBeDefined();
    expect(tx.signatures).toHaveLength(1);
  });
});
```

### Integration Testing

```javascript
describe('Patient Management Integration', () => {
  let patientManager;
  let testPatientId;

  beforeAll(async () => {
    patientManager = new PatientManager('testnet');
    testPatientId = 'GTEST123456789ABCDEFGHIJKLMNOPQRSTUVWXYZ';
  });

  test('should register and retrieve patient', async () => {
    const patientData = {
      publicKey: testPatientId,
      name: 'Test Patient',
      dateOfBirth: '1990-01-01',
      contactInfo: { email: 'test@example.com' },
      emergencyContact: { name: 'Emergency Contact', phone: '+1-555-0123' }
    };

    const registrationResult = await patientManager.registerPatient(patientData);
    expect(registrationResult).toBeDefined();

    const profile = await patientManager.getPatientProfile(testPatientId);
    expect(profile.name).toBe('Test Patient');
  });
});
```

## Best Practices

1. **Security**: Never commit private keys to version control
2. **Error Handling**: Always wrap contract calls in try-catch blocks
3. **Network Management**: Use environment variables for network configuration
4. **Transaction Fees**: Monitor and adjust fees based on network conditions
5. **Event Monitoring**: Implement proper event listeners for real-time updates
6. **Testing**: Write comprehensive unit and integration tests
7. **Rate Limiting**: Implement client-side rate limiting to avoid network abuse

## Troubleshooting

### Common Issues

1. **Account Not Found**: Ensure the account is funded before making transactions
2. **Insufficient Fee**: Increase the transaction fee amount
3. **Network Timeout**: Check network connectivity and RPC endpoint status
4. **Contract Not Found**: Verify contract IDs are correct for the target network
5. **Permission Denied**: Ensure the calling account has proper permissions

### Debug Tools

- [Stellar Laboratory](https://laboratory.stellar.org/) for XDR decoding
- [Soroban Explorer](https://soroban-explorer.stellar.org/) for transaction tracking
- Network-specific RPC endpoints for debugging

## References

- [Stellar SDK Documentation](https://stellar.github.io/js-stellar-sdk/)
- [Soroban Documentation](https://soroban.stellar.org/docs/)
- [Example JavaScript Code](../../../example/js/)
- [Contract API Documentation](../../api/)
