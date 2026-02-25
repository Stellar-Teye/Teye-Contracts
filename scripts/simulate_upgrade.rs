#[derive(Debug, Clone)]
struct StateV1 {
    owner: String,
    balance: u64,
}

#[derive(Debug, Clone)]
struct StateV2 {
    owner: String,
    balance: u64,
    version: u32,
}

fn migrate_v1_to_v2(old: StateV1) -> StateV2 {
    StateV2 {
        owner: old.owner,
        balance: old.balance,
        version: 2,
    }
}

fn main() {
    println!("--- Simulating Upgrade ---");

    let old_state = StateV1 {
        owner: "admin".to_string(),
        balance: 10_000,
    };

    println!("Old State: {:?}", old_state);

    let new_state = migrate_v1_to_v2(old_state);

    println!("New State: {:?}", new_state);

    println!("Upgrade simulation complete.");
}
