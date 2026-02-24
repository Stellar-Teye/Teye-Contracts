#![no_std]

#[cfg(test)]
mod test;

use soroban_sdk::{
    contract, contracterror, contractimpl, contracttype, symbol_short, Address, Env, String,
    Symbol, Vec,
};

const ADMIN: Symbol = symbol_short!("ADMIN");
const INITIALIZED: Symbol = symbol_short!("INIT");
const THRESHOLD_BPS: Symbol = symbol_short!("THRESH");
const REQUEST_COUNTER: Symbol = symbol_short!("REQCTR");
const PROVIDER_KEY: Symbol = symbol_short!("PROV");
const REQUEST_KEY: Symbol = symbol_short!("REQ");
const RESULT_KEY: Symbol = symbol_short!("RES");
const FLAGGED_KEY: Symbol = symbol_short!("FLAGGED");

const MAX_BPS: u32 = 10_000;

#[contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum AiIntegrationError {
    NotInitialized = 1,
    AlreadyInitialized = 2,
    Unauthorized = 3,
    ProviderNotFound = 4,
    ProviderAlreadyExists = 5,
    ProviderInactive = 6,
    InvalidInput = 7,
    RequestNotFound = 8,
    InvalidState = 9,
    ResultAlreadyExists = 10,
    ResultNotFound = 11,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProviderStatus {
    Active,
    Paused,
    Retired,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AiProvider {
    pub provider_id: u32,
    pub operator: Address,
    pub name: String,
    pub model: String,
    pub endpoint_hash: String,
    pub status: ProviderStatus,
    pub registered_at: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RequestStatus {
    Pending,
    Completed,
    Flagged,
    Rejected,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AnalysisRequest {
    pub request_id: u64,
    pub provider_id: u32,
    pub requester: Address,
    pub patient: Address,
    pub record_id: u64,
    pub input_hash: String,
    pub task_type: String,
    pub requested_at: u64,
    pub status: RequestStatus,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum VerificationState {
    Unverified,
    Verified,
    Rejected,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct AnalysisResult {
    pub request_id: u64,
    pub provider_id: u32,
    pub output_hash: String,
    pub confidence_bps: u32,
    pub anomaly_score_bps: u32,
    pub completed_at: u64,
    pub verification_state: VerificationState,
    pub verification_hash: Option<String>,
    pub verified_at: Option<u64>,
    pub verified_by: Option<Address>,
}

#[contract]
pub struct AiIntegrationContract;

#[contractimpl]
impl AiIntegrationContract {
    pub fn initialize(
        env: Env,
        admin: Address,
        anomaly_threshold_bps: u32,
    ) -> Result<(), AiIntegrationError> {
        if env.storage().instance().has(&INITIALIZED) {
            return Err(AiIntegrationError::AlreadyInitialized);
        }
        if anomaly_threshold_bps > MAX_BPS {
            return Err(AiIntegrationError::InvalidInput);
        }

        admin.require_auth();
        env.storage().instance().set(&ADMIN, &admin);
        env.storage()
            .instance()
            .set(&THRESHOLD_BPS, &anomaly_threshold_bps);
        env.storage().instance().set(&INITIALIZED, &true);
        env.storage().instance().set(&REQUEST_COUNTER, &0u64);

        Ok(())
    }

    pub fn get_admin(env: Env) -> Result<Address, AiIntegrationError> {
        Self::require_initialized(&env)?;
        env.storage()
            .instance()
            .get(&ADMIN)
            .ok_or(AiIntegrationError::NotInitialized)
    }

    pub fn is_initialized(env: Env) -> bool {
        env.storage().instance().has(&INITIALIZED)
    }

    pub fn set_anomaly_threshold(
        env: Env,
        caller: Address,
        anomaly_threshold_bps: u32,
    ) -> Result<(), AiIntegrationError> {
        Self::require_initialized(&env)?;
        Self::require_admin(&env, &caller)?;

        if anomaly_threshold_bps > MAX_BPS {
            return Err(AiIntegrationError::InvalidInput);
        }

        env.storage()
            .instance()
            .set(&THRESHOLD_BPS, &anomaly_threshold_bps);
        Ok(())
    }

    pub fn get_anomaly_threshold(env: Env) -> Result<u32, AiIntegrationError> {
        Self::require_initialized(&env)?;
        env.storage()
            .instance()
            .get(&THRESHOLD_BPS)
            .ok_or(AiIntegrationError::NotInitialized)
    }

    pub fn register_provider(
        env: Env,
        caller: Address,
        provider_id: u32,
        operator: Address,
        name: String,
        model: String,
        endpoint_hash: String,
    ) -> Result<(), AiIntegrationError> {
        Self::require_initialized(&env)?;
        Self::require_admin(&env, &caller)?;

        if provider_id == 0 {
            return Err(AiIntegrationError::InvalidInput);
        }
        Self::validate_non_empty(&name)?;
        Self::validate_non_empty(&model)?;
        Self::validate_non_empty(&endpoint_hash)?;

        let key = (PROVIDER_KEY, provider_id);
        if env.storage().persistent().has(&key) {
            return Err(AiIntegrationError::ProviderAlreadyExists);
        }

        let provider = AiProvider {
            provider_id,
            operator,
            name,
            model,
            endpoint_hash,
            status: ProviderStatus::Active,
            registered_at: env.ledger().timestamp(),
        };

        env.storage().persistent().set(&key, &provider);
        Ok(())
    }

    pub fn set_provider_status(
        env: Env,
        caller: Address,
        provider_id: u32,
        status: ProviderStatus,
    ) -> Result<(), AiIntegrationError> {
        Self::require_initialized(&env)?;
        Self::require_admin(&env, &caller)?;

        let key = (PROVIDER_KEY, provider_id);
        let mut provider: AiProvider = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(AiIntegrationError::ProviderNotFound)?;

        provider.status = status;
        env.storage().persistent().set(&key, &provider);

        Ok(())
    }

    pub fn get_provider(env: Env, provider_id: u32) -> Result<AiProvider, AiIntegrationError> {
        Self::require_initialized(&env)?;
        let key = (PROVIDER_KEY, provider_id);
        env.storage()
            .persistent()
            .get(&key)
            .ok_or(AiIntegrationError::ProviderNotFound)
    }

    #[allow(clippy::arithmetic_side_effects)]
    pub fn submit_analysis_request(
        env: Env,
        caller: Address,
        provider_id: u32,
        patient: Address,
        record_id: u64,
        input_hash: String,
        task_type: String,
    ) -> Result<u64, AiIntegrationError> {
        Self::require_initialized(&env)?;
        caller.require_auth();
        Self::ensure_provider_active(&env, provider_id)?;
        Self::validate_non_empty(&input_hash)?;
        Self::validate_non_empty(&task_type)?;

        let mut counter: u64 = env
            .storage()
            .instance()
            .get(&REQUEST_COUNTER)
            .unwrap_or(0u64);
        counter = counter.saturating_add(1);
        env.storage().instance().set(&REQUEST_COUNTER, &counter);

        let request = AnalysisRequest {
            request_id: counter,
            provider_id,
            requester: caller,
            patient,
            record_id,
            input_hash,
            task_type,
            requested_at: env.ledger().timestamp(),
            status: RequestStatus::Pending,
        };

        let key = (REQUEST_KEY, counter);
        env.storage().persistent().set(&key, &request);

        Ok(counter)
    }

    pub fn store_analysis_result(
        env: Env,
        caller: Address,
        request_id: u64,
        output_hash: String,
        confidence_bps: u32,
        anomaly_score_bps: u32,
    ) -> Result<RequestStatus, AiIntegrationError> {
        Self::require_initialized(&env)?;
        caller.require_auth();

        if confidence_bps > MAX_BPS || anomaly_score_bps > MAX_BPS {
            return Err(AiIntegrationError::InvalidInput);
        }
        Self::validate_non_empty(&output_hash)?;

        let request_key = (REQUEST_KEY, request_id);
        let mut request: AnalysisRequest = env
            .storage()
            .persistent()
            .get(&request_key)
            .ok_or(AiIntegrationError::RequestNotFound)?;

        if request.status != RequestStatus::Pending {
            return Err(AiIntegrationError::InvalidState);
        }

        let provider_key = (PROVIDER_KEY, request.provider_id);
        let provider: AiProvider = env
            .storage()
            .persistent()
            .get(&provider_key)
            .ok_or(AiIntegrationError::ProviderNotFound)?;

        if provider.operator != caller {
            return Err(AiIntegrationError::Unauthorized);
        }
        if provider.status != ProviderStatus::Active {
            return Err(AiIntegrationError::ProviderInactive);
        }

        let result_key = (RESULT_KEY, request_id);
        if env.storage().persistent().has(&result_key) {
            return Err(AiIntegrationError::ResultAlreadyExists);
        }

        let threshold = Self::get_anomaly_threshold(env.clone())?;
        let next_status = if anomaly_score_bps >= threshold {
            RequestStatus::Flagged
        } else {
            RequestStatus::Completed
        };

        let result = AnalysisResult {
            request_id,
            provider_id: request.provider_id,
            output_hash,
            confidence_bps,
            anomaly_score_bps,
            completed_at: env.ledger().timestamp(),
            verification_state: VerificationState::Unverified,
            verification_hash: None,
            verified_at: None,
            verified_by: None,
        };

        request.status = next_status.clone();

        env.storage().persistent().set(&result_key, &result);
        env.storage().persistent().set(&request_key, &request);

        if next_status == RequestStatus::Flagged {
            let mut flagged: Vec<u64> = env
                .storage()
                .persistent()
                .get(&FLAGGED_KEY)
                .unwrap_or(Vec::new(&env));
            flagged.push_back(request_id);
            env.storage().persistent().set(&FLAGGED_KEY, &flagged);
        }

        Ok(next_status)
    }

    pub fn verify_analysis_result(
        env: Env,
        caller: Address,
        request_id: u64,
        accepted: bool,
        verification_hash: String,
    ) -> Result<(), AiIntegrationError> {
        Self::require_initialized(&env)?;
        Self::require_admin(&env, &caller)?;
        Self::validate_non_empty(&verification_hash)?;

        let request_key = (REQUEST_KEY, request_id);
        let mut request: AnalysisRequest = env
            .storage()
            .persistent()
            .get(&request_key)
            .ok_or(AiIntegrationError::RequestNotFound)?;
        if request.status == RequestStatus::Pending {
            return Err(AiIntegrationError::InvalidState);
        }

        let result_key = (RESULT_KEY, request_id);
        let mut result: AnalysisResult = env
            .storage()
            .persistent()
            .get(&result_key)
            .ok_or(AiIntegrationError::ResultNotFound)?;

        result.verification_state = if accepted {
            VerificationState::Verified
        } else {
            VerificationState::Rejected
        };
        result.verified_by = Some(caller);
        result.verified_at = Some(env.ledger().timestamp());
        result.verification_hash = Some(verification_hash);

        if !accepted {
            request.status = RequestStatus::Rejected;
            env.storage().persistent().set(&request_key, &request);
        }

        env.storage().persistent().set(&result_key, &result);
        Ok(())
    }

    pub fn get_analysis_request(
        env: Env,
        request_id: u64,
    ) -> Result<AnalysisRequest, AiIntegrationError> {
        Self::require_initialized(&env)?;

        let request_key = (REQUEST_KEY, request_id);
        env.storage()
            .persistent()
            .get(&request_key)
            .ok_or(AiIntegrationError::RequestNotFound)
    }

    pub fn get_analysis_result(
        env: Env,
        request_id: u64,
    ) -> Result<AnalysisResult, AiIntegrationError> {
        Self::require_initialized(&env)?;

        let result_key = (RESULT_KEY, request_id);
        env.storage()
            .persistent()
            .get(&result_key)
            .ok_or(AiIntegrationError::ResultNotFound)
    }

    pub fn get_flagged_requests(env: Env) -> Result<Vec<u64>, AiIntegrationError> {
        Self::require_initialized(&env)?;
        Ok(env
            .storage()
            .persistent()
            .get(&FLAGGED_KEY)
            .unwrap_or(Vec::new(&env)))
    }

    fn require_initialized(env: &Env) -> Result<(), AiIntegrationError> {
        if env.storage().instance().has(&INITIALIZED) {
            return Ok(());
        }
        Err(AiIntegrationError::NotInitialized)
    }

    fn require_admin(env: &Env, caller: &Address) -> Result<(), AiIntegrationError> {
        caller.require_auth();
        let admin: Address = env
            .storage()
            .instance()
            .get(&ADMIN)
            .ok_or(AiIntegrationError::NotInitialized)?;

        if caller != &admin {
            return Err(AiIntegrationError::Unauthorized);
        }

        Ok(())
    }

    fn ensure_provider_active(env: &Env, provider_id: u32) -> Result<(), AiIntegrationError> {
        let key = (PROVIDER_KEY, provider_id);
        let provider: AiProvider = env
            .storage()
            .persistent()
            .get(&key)
            .ok_or(AiIntegrationError::ProviderNotFound)?;

        if provider.status != ProviderStatus::Active {
            return Err(AiIntegrationError::ProviderInactive);
        }

        Ok(())
    }

    fn validate_non_empty(value: &String) -> Result<(), AiIntegrationError> {
        if value.is_empty() {
            return Err(AiIntegrationError::InvalidInput);
        }

        Ok(())
    }
}
