use soroban_sdk::{
    contract, contractimpl, contracttype, symbol_short, Address, Env, Error, IntoVal, Symbol, Val,
    Vec,
};

#[contract]
pub struct AuditContract;

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AuditLogEntry {
    pub sequence: u64,
    pub timestamp: u64,
    pub actor: Address,
    pub action: Symbol,
    pub target: Symbol,
    pub result: Symbol,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SegmentInfo {
    pub entries: Vec<AuditLogEntry>,
    pub next_sequence: u64,
}

const ADMIN: Symbol = symbol_short!("ADMIN");
const SEGMENTS: Symbol = symbol_short!("SEGMENTS");

#[contractimpl]
impl AuditContract {
    pub fn initialize(env: Env, admin: Address) {
        if env.storage().instance().has(&ADMIN) {
            panic!("Already initialized");
        }
        env.storage().instance().set(&ADMIN, &admin);
    }

    pub fn create_segment(env: Env, segment_id: Symbol) -> Result<(), Error> {
        let admin: Address = env.storage().instance().get(&ADMIN).unwrap();
        admin.require_auth();

        if env
            .storage()
            .persistent()
            .has(&(SEGMENTS, segment_id.clone()))
        {
            return Err(Error::from_contract_error(1)); // Segment already exists
        }

        let segment_info = SegmentInfo {
            entries: Vec::new(&env),
            next_sequence: 1,
        };

        env.storage()
            .persistent()
            .set(&(SEGMENTS, segment_id), &segment_info);
        Ok(())
    }

    pub fn append_entry(
        env: Env,
        segment_id: Symbol,
        actor: Address,
        action: Symbol,
        target: Symbol,
        result: Symbol,
    ) -> Result<u64, Error> {
        let mut segment_info: SegmentInfo = env
            .storage()
            .persistent()
            .get(&(SEGMENTS, segment_id.clone()))
            .ok_or(Error::from_contract_error(2))?; // Segment not found

        let sequence = segment_info.next_sequence;
        let entry = AuditLogEntry {
            sequence,
            timestamp: env.ledger().timestamp(),
            actor,
            action,
            target,
            result,
        };

        segment_info.entries.push_back(entry);
        segment_info.next_sequence += 1;

        env.storage()
            .persistent()
            .set(&(SEGMENTS, segment_id), &segment_info);
        Ok(sequence)
    }

    pub fn get_entries(env: Env, segment_id: Symbol) -> Result<Vec<AuditLogEntry>, Error> {
        let segment_info: SegmentInfo = env
            .storage()
            .persistent()
            .get(&(SEGMENTS, segment_id))
            .ok_or(Error::from_contract_error(2))?; // Segment not found

        Ok(segment_info.entries)
    }

    pub fn get_entry_count(env: Env, segment_id: Symbol) -> Result<u64, Error> {
        let segment_info: SegmentInfo = env
            .storage()
            .persistent()
            .get(&(SEGMENTS, segment_id))
            .ok_or(Error::from_contract_error(2))?; // Segment not found

        Ok(segment_info.entries.len() as u64)
    }

    pub fn verify_identity(
        env: Env,
        identity_contract: Address,
        actor: Address,
        method: Symbol,
    ) -> Result<bool, Error> {
        let mut args: Vec<Val> = Vec::new(&env);
        args.push_back(actor.into_val(&env));
        let result: bool = env.invoke_contract(&identity_contract, &method, args);
        Ok(result)
    }

    pub fn check_vault_balance(
        env: Env,
        vault_contract: Address,
        account: Address,
        method: Symbol,
    ) -> Result<i128, Error> {
        let mut args: Vec<Val> = Vec::new(&env);
        args.push_back(account.into_val(&env));
        let balance: i128 = env.invoke_contract(&vault_contract, &method, args);
        Ok(balance)
    }

    pub fn check_compliance(
        env: Env,
        compliance_contract: Address,
        action: Symbol,
        method: Symbol,
    ) -> Result<bool, Error> {
        let mut args: Vec<Val> = Vec::new(&env);
        args.push_back(action.into_val(&env));
        let compliant: bool = env.invoke_contract(&compliance_contract, &method, args);
        Ok(compliant)
    }

    pub fn append_entry_with_checks(
        env: Env,
        segment_id: Symbol,
        actor: Address,
        action: Symbol,
        target: Symbol,
        result: Symbol,
        identity_contract: Address,
        identity_method: Symbol,
        vault_contract: Address,
        vault_method: Symbol,
        compliance_contract: Address,
        compliance_action: Symbol,
        compliance_method: Symbol,
    ) -> Result<u64, Error> {
        let identity_ok = Self::verify_identity(
            env.clone(),
            identity_contract,
            actor.clone(),
            identity_method,
        )?;
        if !identity_ok {
            return Err(Error::from_contract_error(3));
        }

        let balance =
            Self::check_vault_balance(env.clone(), vault_contract, actor.clone(), vault_method)?;
        if balance < 0 {
            return Err(Error::from_contract_error(4));
        }

        let compliant = Self::check_compliance(
            env.clone(),
            compliance_contract,
            compliance_action,
            compliance_method,
        )?;
        if !compliant {
            return Err(Error::from_contract_error(5));
        }

        let seq = Self::append_entry(
            env.clone(),
            segment_id.clone(),
            actor,
            action,
            target,
            result,
        )?;

        Ok(seq)
    }
}
