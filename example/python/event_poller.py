"""
Teye-Contracts: Soroban Event Poller Example
----------------------------------------------
Polls the Soroban RPC for events emitted by your contract.

Install: pip install "stellar-sdk>=13.0.0" pydantic requests
"""
import time
import pydantic
from requests.exceptions import ConnectionError
from stellar_sdk import SorobanServer
from stellar_sdk.soroban_rpc import EventFilter, EventFilterType
from stellar_sdk.exceptions import SorobanRpcErrorResponse

RPC_URL = "https://soroban-testnet.stellar.org"
CONTRACT_ID = "C..."
POLL_INTERVAL_SECONDS = 5

def main():
    server = SorobanServer(RPC_URL)

    try:
        start_ledger = server.get_latest_ledger().sequence
    except ConnectionError as e:
        print(f"Network Connection failed: {e}")
        return

    contract_filter = EventFilter(type=EventFilterType.CONTRACT, contract_ids=[CONTRACT_ID])

    while True:
        try:
            current_ledger = server.get_latest_ledger().sequence
            if current_ledger >= start_ledger:

                # Reset cursor for the new ledger tracking range
                last_cursor = None
                fetching_pages = True

                while fetching_pages:

                    # Protocol 23: start_ledger and cursor are mutually exclusive.
                    if last_cursor:
                        # Page 2+: Query by cursor only
                        response = server.get_events(
                            cursor=last_cursor,
                            filters=[contract_filter],
                            limit=100
                        )
                    else:
                        # Page 1: Query by start_ledger only
                        response = server.get_events(
                            start_ledger=start_ledger,
                            filters=[contract_filter],
                            limit=100
                        )

                    if response.events:
                        for event in response.events:
                            print(f"🔔 Processed Event [{event.id}] in Ledger {event.ledger}")

                        # Protocol 23: paging_token removed from events. Use top-level cursor.
                        last_cursor = response.cursor

                        if len(response.events) < 100:
                            fetching_pages = False
                    else:
                        fetching_pages = False

                # Advance the ledger track after paginating through all events
                start_ledger = current_ledger + 1

        # Targeted Exception Handling
        except SorobanRpcErrorResponse as e:
            print(f"RPC Error: {e}")
        except ConnectionError as e:
            print(f"Network failure: {e}")
        except pydantic.ValidationError as e:
            print(f"Payload validation error: {e}")
        except TypeError as e:
            print(f"Pagination type error: {e}")
        except Exception:
            raise

        time.sleep(POLL_INTERVAL_SECONDS)

if __name__ == "__main__":
    main()
