use soroban_sdk::{contracterror, contracttype, BytesN};

pub type OptionIndex = u32;

#[contracttype]
#[derive(Clone)]
pub enum DataKey {
    Admin,
    OptionCount,
    Tally(OptionIndex),
    Nullifier(BytesN<32>),
    Closed,
    MerkleRoot,
    VerificationKey,
}

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum VoteError {
    BallotNotOpen = 1,
    InvalidProof = 2,
    NullifierAlreadyUsed = 3,
    InvalidOption = 4,
    Unauthorized = 5,
    MerkleRootNotSet = 6,
}
