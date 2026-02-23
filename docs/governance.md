# Platform Governance

## Overview

This platform implements decentralized governance using:

- Governance token with voting power
- Proposal creation and voting
- Execution timelock for approved proposals
- Delegation support

## Process

1. **Propose**: Any user above the proposal threshold can submit a proposal.
2. **Vote**: Token holders vote for or against within the voting period.
3. **Quorum**: Proposals require at least 4% of tokens to vote.
4. **Execution**: Approved proposals are executed via the timelock contract after a delay.
5. **Delegation**: Users can delegate voting power to others.

## Smart Contracts

- `PlatformGovernor.sol` — governance logic
- `PlatformTimelock.sol` — timelock execution
