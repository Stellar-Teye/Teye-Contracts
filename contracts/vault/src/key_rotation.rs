use soroban_sdk::{Address, contracttype};

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct RotationEvent {
    pub owner: Address,
    pub previous_epoch: u32,
    pub new_epoch: u32,
    pub rotated_at: u64,
}
