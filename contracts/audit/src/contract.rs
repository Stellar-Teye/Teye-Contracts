use soroban_sdk::{
    contract, contractimpl, contracttype, contracterror, symbol_short, Address, Env, Error,
    IntoVal, Symbol, Val, Vec,
};

/// Typed error enum for the AuditContract.
///
/// Using `#[contracterror]` ensures the SDK encodes these as u32 contract
/// errors so callers can match on them with `try_*` client methods.
#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum AuditContractError {
    /// `initialize` was called on an already-initialized contract.
    AlreadyInitialized = 1,
    /// The requested segment does not exist.
    SegmentNotFound = 2,
    /// The segment already exists.
    SegmentAlreadyExists = 3,
    /// Identity verification failed.
    IdentityCheckFailed = 4,
    /// Vault balance check failed.
    VaultBalanceInsufficient = 5,
    /// Compliance check failed.
    ComplianceCheckFailed = 6,
}

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
    /// Initialize the contract with an admin address.
    ///
    /// # Errors
    /// Returns [`AuditContractError::AlreadyInitialized`] if called more than once.
    pub fn initialize(env: Env, admin: Address) -> Result<(), AuditContractError> {
        if env.storage().instance().has(&ADMIN) {
            return Err(AuditContractError::AlreadyInitialized);
        }
        env.storage().instance().set(&ADMIN, &admin);
        Ok(())
    }

    pub fn create_segment(env: Env, segment_id: Symbol) -> Result<(), Error> {
        let admin: Address = env.storage().instance().get(&ADMIN).unwrap();
        admin.require_auth();

        if env
            .storage()
            .persistent()
            .has(&(SEGMENTS, segment_id.clone()))
        {
            return Err(Error::from_contract_error(
                AuditContractError::SegmentAlreadyExists as u32,
            ));
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
            .ok_or(Error::from_contract_error(
                AuditContractError::SegmentNotFound as u32,
            ))?;

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
            .ok_or(Error::from_contract_error(
                AuditContractError::SegmentNotFound as u32,
            ))?;

        Ok(segment_info.entries)
    }

    pub fn get_entry_count(env: Env, segment_id: Symbol) -> Result<u64, Error> {
        let segment_info: SegmentInfo = env
            .storage()
            .persistent()
            .get(&(SEGMENTS, segment_id))
            .ok_or(Error::from_contract_error(
                AuditContractError::SegmentNotFound as u32,
            ))?;

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
            return Err(Error::from_contract_error(
                AuditContractError::IdentityCheckFailed as u32,
            ));
        }

        let balance =
            Self::check_vault_balance(env.clone(), vault_contract, actor.clone(), vault_method)?;
        if balance < 0 {
            return Err(Error::from_contract_error(
                AuditContractError::VaultBalanceInsufficient as u32,
            ));
        }

        let compliant = Self::check_compliance(
            env.clone(),
            compliance_contract,
            compliance_action,
            compliance_method,
        )?;
        if !compliant {
            return Err(Error::from_contract_error(
                AuditContractError::ComplianceCheckFailed as u32,
            ));
        }

        Self::append_entry(env, segment_id, actor, action, target, result)
    }
}
