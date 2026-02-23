use your_crate_name::state_v1::StateV1;
use your_crate_name::migrate::migrate_v1_to_v2;

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
