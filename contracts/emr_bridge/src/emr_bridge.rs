use soroban_sdk::{Env, BytesN, String, Vec};
use crate::storage::{get_records, set_records, Record};

pub fn ingest_record(env: Env, id: BytesN<32>, data: String) {
    let mut records = get_records(&env);

    if records.contains_key(id.clone()) {
        panic!("DuplicateRecord");
    }

    let mut history = Vec::new(&env);

    history.push_back(Record {
        data,
        version: 1,
    });

    records.set(id, history);
    set_records(&env, records);
}

pub fn update_record(env: Env, id: BytesN<32>, data: String) {
    let mut records = get_records(&env);

    let mut history = records.get(id.clone()).expect("Record not found");

    let new_version = history.len() as u32 + 1;

    history.push_back(Record {
        data,
        version: new_version,
    });

    records.set(id, history);
    set_records(&env, records);
}