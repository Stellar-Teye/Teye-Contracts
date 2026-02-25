# Python SDK Integration Guide

This guide demonstrates how to integrate Stellar Teye contracts into Python applications using the stellar-sdk.

## Prerequisites

- **Python**: Version 3.8 or higher
- **Package Manager**: pip
- **Stellar Account**: Funded account on the target network

### Installation

```bash
pip install "stellar-sdk>=13.0.0" python-dotenv requests
```

### Environment Setup

Create a `.env` file:

```env
# Network Configuration
RPC_URL=https://soroban-testnet.stellar.org
NETWORK_PASSPHRASE=Test SDF Network ; September 2015

# Contract Configuration
TEYE_CONTRACT_ID=YOUR_CONTRACT_ID_HERE
VISION_RECORDS_CONTRACT_ID=YOUR_VISION_RECORDS_CONTRACT_ID

# Authentication
SERVER_SECRET_KEY=your_secret_key_here
```

## Connection Setup

```python
import os
import time
from stellar_sdk import SorobanServer, Keypair, Network, TransactionBuilder, scval
from stellar_sdk.contract import Contract
from stellar_sdk.exceptions import SorobanRpcErrorResponse
from dotenv import load_dotenv

load_dotenv()

class TeyeSDK:
    def __init__(self, network="testnet"):
        self.server = SorobanServer(os.getenv("RPC_URL"))
        self.network_passphrase = Network.TESTNET_NETWORK_PASSPHRASE
        self.contract_id = os.getenv("TEYE_CONTRACT_ID")
        self.keypair = Keypair.from_secret(os.getenv("SERVER_SECRET_KEY"))
        
    def get_account(self):
        return self.server.load_account(self.keypair.public_key)
    
    def build_transaction(self, contract_method, *args):
        account = self.get_account()
        contract = Contract(self.contract_id)
        
        sc_args = [scval.to_primitive(arg) for arg in args]
        
        tx = (
            TransactionBuilder(account, network_passphrase=self.network_passphrase)
            .append_operation(contract.call(contract_method, *sc_args))
            .set_timeout(30)
            .build()
        )
        
        return tx
    
    def simulate_and_sign(self, tx):
        simulation = self.server.simulate_transaction(tx)
        
        if simulation.error:
            raise Exception(f"Simulation failed: {simulation.error}")
        
        tx = self.server.prepare_transaction(tx, simulation)
        tx.sign(self.keypair)
        return tx
    
    def submit_transaction(self, tx):
        response = self.server.send_transaction(tx)
        
        if response.status == "ERROR":
            raise Exception(f"Transaction failed: {response.error_result_xdr}")
        
        # Poll for completion
        start_time = time.time()
        timeout = 30
        
        while time.time() - start_time < timeout:
            tx_status = self.server.get_transaction(response.hash)
            if tx_status.status == "SUCCESS":
                return scval.from_native(tx_status.result.value)
            elif tx_status.status == "FAILED":
                raise Exception("Transaction failed on-chain")
            time.sleep(2)
        
        raise Exception("Transaction timeout")
```

## Contract Interaction Patterns

### Patient Management

```python
class PatientManager(TeyeSDK):
    def register_patient(self, patient_data):
        tx = self.build_transaction(
            "register_patient",
            patient_data["public_key"],
            patient_data["name"],
            patient_data["date_of_birth"],
            patient_data["contact_info"],
            patient_data["emergency_contact"]
        )
        signed_tx = self.simulate_and_sign(tx)
        return self.submit_transaction(signed_tx)
    
    def get_patient_profile(self, patient_id):
        key = scval.to_primitive(patient_id)
        contract_data = self.server.get_contract_data(
            self.contract_id,
            key
        )
        return scval.from_native(contract_data.val)
```

### Vision Records

```python
class VisionRecordsManager(TeyeSDK):
    def add_vision_record(self, record_data):
        tx = self.build_transaction(
            "add_vision_record",
            record_data["patient_id"],
            record_data["provider_id"],
            record_data["record_type"],
            record_data["encrypted_data_hash"],
            record_data["metadata"]
        )
        signed_tx = self.simulate_and_sign(tx)
        return self.submit_transaction(signed_tx)
    
    def get_patient_records(self, patient_id, record_type=None):
        key = f"patient_records_{patient_id}_{record_type}" if record_type else f"patient_records_{patient_id}"
        sc_key = scval.to_primitive(key)
        
        try:
            contract_data = self.server.get_contract_data(self.contract_id, sc_key)
            return scval.from_native(contract_data.val)
        except:
            return []
```

### Access Control

```python
class AccessControlManager(TeyeSDK):
    def grant_access(self, patient_id, requester_id, permissions, duration=None):
        access_request = {
            "patient_id": patient_id,
            "requester_id": requester_id,
            "permissions": permissions,
            "expires_at": int(time.time() * 1000) + duration if duration else None,
            "granted_at": int(time.time() * 1000)
        }
        
        tx = self.build_transaction("grant_access", patient_id, requester_id, access_request)
        signed_tx = self.simulate_and_sign(tx)
        return self.submit_transaction(signed_tx)
    
    def check_access(self, patient_id, requester_id, permission="read"):
        access_key = f"access_{patient_id}_{requester_id}"
        
        try:
            sc_key = scval.to_primitive(access_key)
            contract_data = self.server.get_contract_data(self.contract_id, sc_key)
            access_data = scval.from_native(contract_data.val)
            
            has_permission = permission in access_data["permissions"]
            not_expired = not access_data["expires_at"] or access_data["expires_at"] > time.time() * 1000
            
            return has_permission and not_expired
        except:
            return False
```

## Async Patterns

```python
import asyncio
import aiohttp
from stellar_sdk.aio import AioSorobanServer

class AsyncTeyeSDK:
    def __init__(self, network="testnet"):
        self.server = AioSorobanServer(os.getenv("RPC_URL"))
        self.network_passphrase = Network.TESTNET_NETWORK_PASSPHRASE
        self.contract_id = os.getenv("TEYE_CONTRACT_ID")
        self.keypair = Keypair.from_secret(os.getenv("SERVER_SECRET_KEY"))
    
    async def get_account_async(self):
        return await self.server.load_account(self.keypair.public_key)
    
    async def submit_transaction_async(self, tx):
        response = await self.server.send_transaction(tx)
        
        if response.status == "ERROR":
            raise Exception(f"Transaction failed: {response.error_result_xdr}")
        
        # Async polling
        start_time = time.time()
        timeout = 30
        
        while time.time() - start_time < timeout:
            tx_status = await self.server.get_transaction(response.hash)
            if tx_status.status == "SUCCESS":
                return scval.from_native(tx_status.result.value)
            elif tx_status.status == "FAILED":
                raise Exception("Transaction failed on-chain")
            await asyncio.sleep(2)
        
        raise Exception("Transaction timeout")

# Usage
async def main():
    sdk = AsyncTeyeSDK()
    # Use async methods here
    pass

# asyncio.run(main())
```

## Event Subscription

```python
class EventListener(TeyeSDK):
    def __init__(self, network="testnet"):
        super().__init__(network)
        self.event_filters = {}
        self.listening = False
    
    def subscribe_to_events(self, contract_id, event_types=None):
        from stellar_sdk.soroban_rpc import EventFilter, EventFilterType
        
        self.event_filters[contract_id] = EventFilter(
            type=EventFilterType.CONTRACT,
            contract_ids=[contract_id],
            topics=event_types
        )
    
    def start_polling(self, callback, poll_interval=5):
        if self.listening:
            raise Exception("Already polling for events")
        
        self.listening = True
        last_ledger = self.server.get_latest_ledger().sequence
        
        def poll():
            if not self.listening:
                return
            
            try:
                for contract_id, event_filter in self.event_filters.items():
                    events = self.server.get_events(
                        start_ledger=last_ledger,
                        filters=[event_filter],
                        limit=100
                    )
                    
                    for event in events.events:
                        callback({
                            "contract_id": contract_id,
                            "event_type": event.type,
                            "value": scval.from_native(event.value),
                            "ledger": event.ledger
                        })
                
                last_ledger = self.server.get_latest_ledger().sequence
            except Exception as e:
                print(f"Event polling error: {e}")
            
            if self.listening:
                time.sleep(poll_interval)
                poll()
        
        poll()
    
    def stop_polling(self):
        self.listening = False
```

## Full Working Example

```python
import asyncio
from datetime import datetime

class TeyeHealthcareApp:
    def __init__(self):
        self.sdk = TeyeSDK("testnet")
        self.patient_manager = PatientManager("testnet")
        self.records_manager = VisionRecordsManager("testnet")
        self.access_manager = AccessControlManager("testnet")
    
    def onboard_patient(self, patient_data):
        try:
            print("🏥 Starting patient onboarding...")
            
            # Register patient
            print("📝 Registering patient...")
            patient_result = self.patient_manager.register_patient(patient_data)
            print(f"✅ Patient registered: {patient_result}")
            
            # Create vision record
            print("👁️ Creating vision record...")
            vision_record = {
                "patient_id": patient_data["public_key"],
                "provider_id": os.getenv("PROVIDER_PUBLIC_KEY"),
                "record_type": "exam",
                "encrypted_data_hash": "0x" + "0" * 64,
                "metadata": {
                    "exam_date": datetime.now().isoformat(),
                    "exam_type": "comprehensive"
                }
            }
            
            record_result = self.records_manager.add_vision_record(vision_record)
            print(f"✅ Vision record created: {record_result}")
            
            # Grant access
            print("🔐 Granting access...")
            access_result = self.access_manager.grant_access(
                patient_data["public_key"],
                os.getenv("PROVIDER_PUBLIC_KEY"),
                ["read", "write"],
                30 * 24 * 60 * 60 * 1000  # 30 days
            )
            print(f"✅ Access granted: {access_result}")
            
            return {
                "patient": patient_result,
                "record": record_result,
                "access": access_result
            }
            
        except Exception as e:
            print(f"❌ Onboarding failed: {e}")
            raise

def main():
    app = TeyeHealthcareApp()
    
    patient_data = {
        "public_key": "GABCDEFGHIJKLMNOPQRSTUVWXYZ123456789",
        "name": "John Doe",
        "date_of_birth": "1990-01-01",
        "contact_info": {"email": "john.doe@example.com"},
        "emergency_contact": {"name": "Jane Doe", "phone": "+1-555-0124"}
    }
    
    try:
        result = app.onboard_patient(patient_data)
        print("🎉 Patient onboarded successfully!")
    except Exception as e:
        print(f"💥 Application error: {e}")

if __name__ == "__main__":
    main()
```

## Testing

```python
import unittest
from unittest.mock import Mock, patch

class TestTeyeSDK(unittest.TestCase):
    def setUp(self):
        self.sdk = TeyeSDK("testnet")
    
    def test_connection_setup(self):
        self.assertEqual(self.sdk.server.server_url, "https://soroban-testnet.stellar.org")
        self.assertIsNotNone(self.sdk.keypair)
    
    @patch('stellar_sdk.SorobanServer.simulate_transaction')
    def test_transaction_simulation(self, mock_simulate):
        mock_simulate.return_value = Mock(error=None)
        tx = Mock()
        result = self.sdk.simulate_and_sign(tx)
        self.assertIsNotNone(result)

if __name__ == '__main__':
    unittest.main()
```

## Best Practices

1. **Environment Variables**: Use `.env` for sensitive configuration
2. **Error Handling**: Wrap all contract calls in try-catch blocks
3. **Async Operations**: Use async patterns for better performance
4. **Rate Limiting**: Implement client-side rate limiting
5. **Testing**: Write comprehensive unit and integration tests

## References

- [Stellar Python SDK](https://stellar-sdk.readthedocs.io/)
- [Example Python Code](../../../example/python/)
- [Contract API Documentation](../../api/)
