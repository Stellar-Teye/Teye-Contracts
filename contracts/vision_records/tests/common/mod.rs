use soroban_sdk::{testutils::Address as _, Address, Env, String};
use vision_records::{RecordType, Role, VisionRecordsContract, VisionRecordsContractClient};

pub struct TestContext {
    pub env: Env,
    pub client: VisionRecordsContractClient<'static>,
    pub admin: Address,
}

/// Creates a mocked Soroban environment, deploys the contract, and initializes admin.
pub fn setup_test_env() -> TestContext {
    let env = Env::default();
    env.mock_all_auths();

    let contract_id = env.register(VisionRecordsContract, ());
    let client = VisionRecordsContractClient::new(&env, &contract_id);

    let admin = Address::generate(&env);
    client.initialize(&admin);

    TestContext { env, client, admin }
}

/// Creates and registers a user for tests and returns its address.
pub fn create_test_user(ctx: &TestContext, role: Role, name: &str) -> Address {
    let user = Address::generate(&ctx.env);
    let name = String::from_str(&ctx.env, name);
    ctx.client.register_user(&ctx.admin, &user, &role, &name);
    user
}

/// Creates a record and returns the generated record id.
pub fn create_test_record(
    ctx: &TestContext,
    caller: &Address,
    patient: &Address,
    provider: &Address,
    record_type: RecordType,
    data_hash: &str,
) -> u64 {
    let hash = String::from_str(&ctx.env, data_hash);
    ctx.client
        .add_record(caller, patient, provider, &record_type, &hash)
}
