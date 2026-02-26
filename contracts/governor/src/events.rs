//! Structured event publishing for the Governor contract.

use soroban_sdk::{symbol_short, Address, Env};

use crate::proposal::{Proposal, ProposalPhase};
use crate::voting::VoteChoice;

pub fn publish_proposal_created(env: &Env, proposal: &Proposal) {
    env.events().publish(
        (symbol_short!("PROP_NEW"), proposal.id),
        (
            proposal.proposer.clone(),
            proposal.proposal_type.clone(),
            proposal.title.clone(),
        ),
    );
}

pub fn publish_phase_transition(env: &Env, proposal_id: u64, new_phase: &ProposalPhase) {
    env.events().publish(
        (symbol_short!("PROP_PHS"), proposal_id),
        new_phase.clone(),
    );
}

pub fn publish_vote_committed(env: &Env, proposal_id: u64, voter: &Address) {
    env.events().publish(
        (symbol_short!("VOTE_COM"), proposal_id),
        voter.clone(),
    );
}

pub fn publish_vote_revealed(
    env: &Env,
    proposal_id: u64,
    voter: &Address,
    choice: &VoteChoice,
    power: i128,
) {
    env.events().publish(
        (symbol_short!("VOTE_REV"), proposal_id),
        (voter.clone(), choice.clone(), power),
    );
}

pub fn publish_delegation_set(env: &Env, voter: &Address, delegate: &Address) {
    env.events()
        .publish((symbol_short!("DEL_SET"),), (voter.clone(), delegate.clone()));
}

pub fn publish_delegation_revoked(env: &Env, voter: &Address) {
    env.events()
        .publish((symbol_short!("DEL_REV"),), voter.clone());
}

pub fn publish_proposal_executed(env: &Env, proposal_id: u64) {
    env.events()
        .publish((symbol_short!("PROP_EXE"),), proposal_id);
}
