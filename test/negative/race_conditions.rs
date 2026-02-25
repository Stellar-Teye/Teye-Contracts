use std::sync::Arc;
use std::thread;
use your_crate_name::*;

#[test]
fn test_race_condition_simulation() {
    let contract = Arc::new(Contract::new("owner".to_string()));

    let mut handles = vec![];

    for _ in 0..10 {
        let contract_clone = Arc::clone(&contract);
        handles.push(thread::spawn(move || {
            contract_clone.increment_counter().unwrap();
        }));
    }

    for handle in handles {
        handle.join().unwrap();
    }

    let final_value = *contract.counter.lock().unwrap();
    assert_eq!(final_value, 10);
}
