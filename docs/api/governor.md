# Governor Contract API Reference

## Contract Purpose

Implements OpenZeppelin Governor pattern for decentralized governance. Enables token-holder voting on proposals with integration to the Treasury for fund allocation. Built on Solidity/EVM (primarily for interoperability documentation).

## Overview

The Governor contract coordinates:

1. **Proposal Creation:** Token holders propose actions
2. **Voting:** Token-weighted voting aligned with staking
3. **Timelock:** Delay before execution (security/governance pause)
4. **Execution:** Premier approved proposals via Treasury

---

## Key Parameters

| Parameter              | Value         | Purpose                                        |
| ---------------------- | ------------- | ---------------------------------------------- |
| **Voting Delay**       | 1 block       | Block delay after proposal before voting opens |
| **Voting Period**      | 45,818 blocks | ~1 week of voting time                         |
| **Proposal Threshold** | 1e18 tokens   | Minimum to create proposal                     |
| **Quorum**             | 4%            | Minimum participation to validate vote         |
| **Timelock**           | 2 days        | Delay between passage and execution            |

---

## Governance Flow

```
Token Holder (≥1M tokens)
    ↓
1. Propose Action
    ↓
2. Voting Period (1 week)
    - For / Against / Abstain voting
    - Requires 4% quorum
    ↓
3. If Passed → Timelock Queue (2 days)
    ↓
4. Execute Action
    - Transfer Treasury funds
    - Upgrade parameters
    - etc.
```

---

## Core Functions

### Proposal Management

#### `propose(address[] targets, uint256[] values, bytes[] calldatas, string memory description) -> uint256`

Create a governance proposal.

**Parameters:**

- `targets` - Contract addresses to call
- `values` - ETH values for each call
- `calldatas` - Encoded function calls (ABI encoded)
- `description` - Proposal description (ipfs:// recommended)

**Returns:** `proposalId` - Unique proposal identifier

**Requirements:**

- Caller must have ≥ proposal threshold tokens
- Same-length arrays for targets/values/calldatas

**Example (Treasury Transfer):**

```solidity
address[] memory targets = new address[](1);
targets[0] = treasury;

uint256[] memory values = new uint256[](1);
values[0] = 0;

bytes[] memory calldatas = new bytes[](1);
calldatas[0] = abi.encodeWithSignature(
    "transfer(address,uint256)",
    recipient,
    1_000_000e18  // 1M tokens
);

string memory description = "ipfs://QmXxx...";

uint256 proposalId = governor.propose(
    targets,
    values,
    calldatas,
    description
);
```

---

### Voting

#### `castVote(uint256 proposalId, uint8 support) -> uint96`

Cast a vote on an active proposal.

**Parameters:**

- `proposalId` - Proposal to vote on
- `support` - Vote direction:
  - `0` = Against
  - `1` = For
  - `2` = Abstain

**Returns:** Weight (token amount used for vote)

**Requirements:**

- Proposal must be in voting period
- Caller must have voting power (from staking contract)

**Example:**

```solidity
uint96 weight = governor.castVote(proposalId, 1);  // Vote For
```

---

#### `castVoteWithReason(uint256 proposalId, uint8 support, string calldata reason) -> uint96`

Cast a vote with supporting context.

**Parameters:**
Same as `castVote` plus:

- `reason` - Comment explaining vote (stored in events)

---

### Execution

#### `queue(address[] targets, uint256[] values, bytes[] calldatas, bytes32 descriptionHash) -> bytes32`

Queue an approved proposal for execution (after timelock).

**Parameters:**

- `targets`, `values`, `calldatas` - Same as proposal
- `descriptionHash` - Hash of proposal description (keccak256(description))

**Returns:** `operationId` - Unique identifier

**Requirements:**

- Proposal must have achieved quorum and passed
- Caller is typically a DAO operator (not token-permissioned)

---

#### `execute(address[] targets, uint256[] values, bytes[] calldatas, bytes32 descriptionHash)`

Execute a queued proposal (after timelock expires).

**Parameters:**
Same as `queue`

**Execution:**

1. Verifies timelock expired
2. Performs each call in sequence
3. Emits execution events

**Example:**

```solidity
bytes32 descHash = keccak256(abi.encode(
    targets, values, calldatas, description
));

governor.queue(targets, values, calldatas, descHash);

// Wait 2 days...

governor.execute(targets, values, calldatas, descHash);
```

---

### Proposal State

#### `state(uint256 proposalId) -> ProposalState`

Get current phase of a proposal.

**Returns:** State enum:

- `Pending` - Created, voting not yet open
- `Active` - Voting in progress (within voting period)
- `Cancelled` - Proposal cancelled
- `Defeated` - Voting failed (insufficient support)
- `Succeeded` - Passed voting period with sufficient votes
- `Queued` - Queued for timelock
- `Expired` - Timelock expired without execution
- `Executed` - Successfully executed

---

#### `proposalDeadline(uint256 proposalId) -> uint256`

Get the block number when voting closes for a proposal.

---

#### `proposalSnapshot(uint256 proposalId) -> uint256`

Get the block number at which voting power is measured (voting delay after proposal).

---

## Quorum & Voting Math

**Voting Power:** Token balance at proposal snapshot block

**Quorum Requirement:**

```
quorumRequired = totalTokenSupply * 4%
```

**Passage requirement:**

```
votesFor > votesAgainst  // Simple majority
votesFor + votesAgainst + votesAbstain ≥ quorumRequired
```

---

## Integration with Treasury

Governor can call `treasury::governor_spend()` to allocate funds:

```solidity
// In proposal calldatas:
treasury.governor_spend(
    recipient,
    amount
)

// Treasury verifies caller is registered governor
// Executes spend without additional multisig approval
```

---

## Events

| Event               | Parameters                                                                                      | Significance                 |
| ------------------- | ----------------------------------------------------------------------------------------------- | ---------------------------- |
| `ProposalCreated`   | proposalId, proposer, targets, values, signatures, calldatas, startBlock, endBlock, description | Proposal initiated           |
| `VoteCast`          | voter, proposalId, support, weight, reason                                                      | Vote recorded                |
| `ProposalQueued`    | proposalId, eta                                                                                 | Proposed queued for timelock |
| `ProposalExecuted`  | proposalId                                                                                      | Proposal executed            |
| `ProposalCancelled` | proposalId                                                                                      | Proposal cancelled           |

---

## Extensions & Customizations

Governor is built with OpenZeppelin extensions:

| Extension                       | Purpose                                  |
| ------------------------------- | ---------------------------------------- |
| **GovernorSettings**            | Voting delay, period, proposal threshold |
| **GovernorCountingSimple**      | For/Against/Abstain vote tallying        |
| **GovernorVotes**               | Token-based voting power                 |
| **GovernorVotesQuorumFraction** | Percentage-based quorum                  |
| **GovernorTimelockControl**     | Timelock integration for delay           |

---

## Security Considerations

1. **Voting Delay:** 1-block delay prevents flash-loan voting from same transaction
2. **Voting Period:** 1-week timeline provides ample deliberation
3. **Quorum:** 4% requirement prevents super-minority governance
4. **Timelock:** 2-day delay allows token holders to exit if unhappy with outcome
5. **Proposal Threshold:** 1M+ tokens prevents spam proposals

---

## Typical DAO Governance Workflow

```
Week 1: Proposal Discussion (off-chain, forum/discord)
         Token holder snapshot included

Block 0: Proposal submitted
         → ProposalCreated event

Block 1+: Voting opens
         → VoteCast events (1 week)

Block 50819: Voting closes
            → Check quorum & vote tally
            → If passed, proposal moves to Succeeded

Day 1: Proposal queued
       → ProposalQueued event
       → Timelock started (2 days)

Day 3: Timelock expired
       → Proposal executed
       → ProposalExecuted event
       → Funds transferred / parameters updated
```

---

## Related Documentation

- [Governance Design](../docs/governance.md)
- [Treasury Integration](./treasury.md)
- [Staking (Voting Power)](./staking.md)
- [OpenZeppelin Governor Docs](https://docs.openzeppelin.com/contracts/4.x/governance)
