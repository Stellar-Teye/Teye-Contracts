#![no_main]

use arbitrary::Arbitrary;
use libfuzzer_sys::fuzz_target;
use soroban_sdk::{testutils::Address as _, Address, Env, String};
use vision_records::{RecordType, Role, VisionRecordsContract, VisionRecordsContractClient};

#[derive(Arbitrary, Debug)]
pub enum FuzzAction {
    RegisterUser { name_len: u8, role: u8 },
    AddRecord { record_type: u8, hash_len: u8 },
}

fuzz_target!(|actions: Vec<FuzzAction>| {
    let env = Env::default();
    let admin = Address::generate(&env);

    // Register contract
    let contract_id = env.register(VisionRecordsContract, ());
    let client = VisionRecordsContractClient::new(&env, &contract_id);

    // Initialize
    let _ = client.try_initialize(&admin);

    let mut users = vec![admin.clone()];
    let provider = Address::generate(&env);
    users.push(provider.clone());

    let _ = client.try_register_user(
        &admin,
        &provider,
        &Role::Optometrist,
        &String::from_str(&env, "Provider"),
    );

    for action in actions {
        match action {
            FuzzAction::RegisterUser { name_len, role } => {
                let user = Address::generate(&env);
                let role_enum = match role % 4 {
                    0 => Role::Patient,
                    1 => Role::Optometrist,
                    2 => Role::Ophthalmologist,
                    _ => Role::Admin,
                };

                // create random string
                let mut name_bytes = vec![b'a'; name_len as usize];
                if name_len > 100 {
                    name_bytes.truncate(100);
                }

                let name = match std::str::from_utf8(&name_bytes) {
                    Ok(s) => String::from_str(&env, s),
                    Err(_) => String::from_str(&env, "test"),
                };

                // Random caller
                let caller = &users[name_len as usize % users.len()];

                let _ = client.try_register_user(caller, &user, &role_enum, &name);
                users.push(user);
            }
            FuzzAction::AddRecord {
                record_type,
                hash_len,
            } => {
                let r_type = match record_type % 6 {
                    0 => RecordType::Examination,
                    1 => RecordType::Prescription,
                    2 => RecordType::Diagnosis,
                    3 => RecordType::Treatment,
                    4 => RecordType::Surgery,
                    _ => RecordType::LabResult,
                };

                // Create random hash string. Use exact 32-char length if hash_len % 2 == 0 to hit success paths,
                // or random length to hit errors.
                let hash_str = if hash_len % 2 == 0 {
                    "a".repeat(32)
                } else {
                    "a".repeat((hash_len as usize % 50) + 1)
                };
                let hash = String::from_str(&env, &hash_str);

                let patient = &users[hash_len as usize % users.len()];
                let caller = &users[(hash_len as usize + 1) % users.len()];

                let _ = client.try_add_record(caller, patient, &provider, &r_type, &hash);
            }
        }
    }
});
