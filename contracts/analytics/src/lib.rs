pub mod aggregation;
pub mod differential_privacy;
pub mod homomorphic;
pub mod events;

#[cfg(test)]
mod test;

use soroban_sdk::{
    contract, contractclient, contractimpl, contracttype, symbol_short, Address, Env, Symbol, Vec,
};

use crate::aggregation::Aggregator;
use crate::differential_privacy::DifferentialPrivacy;
use crate::homomorphic::{HomomorphicEngine, PaillierPrivateKey, PaillierPublicKey};

// ── Storage keys ────────────────────────────────────────────────────────────────

const ADMIN: Symbol = symbol_short!("ADMIN");
const AGGREGATOR: Symbol = symbol_short!("AGGR");
const METRIC: Symbol = symbol_short!("METRIC");
const PUB_KEY: Symbol = symbol_short!("PUB_KEY");
const PRIV_KEY: Symbol = symbol_short!("PRIV_KEY");
const DEP_VER: Symbol = symbol_short!("DEP_VER");

// ── Types ──────────────────────────────────────────────────────────────────────

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MetricDimensions {
    pub region: Option<Symbol>,
    pub age_band: Option<Symbol>,
    pub condition: Option<Symbol>,
    pub time_bucket: u64,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MetricValue {
    pub count: i128,
    pub sum: i128,
    pub version: u32,
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrendPoint {
    pub time_bucket: u64,
    pub value: MetricValue,
}

// Note: Events moved to events.rs

#[soroban_sdk::contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum ContractError {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    Unauthorized = 3,
    ExternalCallFailed = 4,
    TimelockNotMet = 5,
    SubmissionExpired = 6,
    InvalidInput = 7,
}

#[contractclient(name = "MetricSourceClient")]
trait MetricSourceInterface {
    fn read_metric(env: Env, kind: Symbol, dims: MetricDimensions) -> MetricValue;
}

// ── Contract ───────────────────────────────────────────────────────────────────

#[contract]
pub struct AnalyticsContract;

#[contractimpl]
impl AnalyticsContract {
    pub fn initialize(
        env: Env,
        admin: Address,
        aggregator: Address,
        pub_key: PaillierPublicKey,
        priv_key: Option<PaillierPrivateKey>,
    ) -> Result<(), ContractError> {
        if env.storage().instance().has(&ADMIN) {
            return Err(ContractError::AlreadyInitialized);
        }
        if pub_key.n <= 0 || pub_key.nn <= 0 || pub_key.g <= 0 {
            return Err(ContractError::InvalidInput);
        }
        if let Some(ref pk) = priv_key {
            if pk.lambda <= 0 || pk.mu <= 0 {
                return Err(ContractError::InvalidInput);
            }
        }
        env.storage().instance().set(&ADMIN, &admin);
        env.storage().instance().set(&AGGREGATOR, &aggregator);
        env.storage().instance().set(&PUB_KEY, &pub_key);
        if let Some(pk) = priv_key {
            env.storage().instance().set(&PRIV_KEY, &pk);
        }
        env.storage().instance().set(&DEP_VER, &1u32);

        env.events().publish(
            (symbol_short!("INIT"), admin.clone(), aggregator.clone()),
            events::InitializedEvent { admin, aggregator },
        );
        Ok(())
    }

    pub fn get_dep_ver(env: Env) -> u32 {
        env.storage().instance().get(&DEP_VER).unwrap_or(0)
    }

    pub fn set_aggregator(env: Env, new_aggregator: Address) -> Result<(), ContractError> {
        let admin: Address = env.storage().instance().get(&ADMIN).ok_or(ContractError::NotInitialized)?;
        admin.require_auth();

        env.storage().instance().set(&AGGREGATOR, &new_aggregator);
        
        let new_ver = Self::get_dep_ver(env.clone()) + 1;
        env.storage().instance().set(&DEP_VER, &new_ver);

        env.events().publish(
            (symbol_short!("DEP_UP"), symbol_short!("AGGR"), admin.clone()),
            events::DependencyUpdatedEvent {
                admin,
                effect: symbol_short!("AGGR"),
                new_version: new_ver,
            },
        );
        Ok(())
    }

    pub fn set_paillier_keys(
        env: Env,
        pub_key: PaillierPublicKey,
        priv_key: Option<PaillierPrivateKey>,
    ) -> Result<(), ContractError> {
        let admin: Address = env.storage().instance().get(&ADMIN).ok_or(ContractError::NotInitialized)?;
        admin.require_auth();

        if pub_key.n <= 0 || pub_key.nn <= 0 || pub_key.g <= 0 {
            return Err(ContractError::InvalidInput);
        }
        if let Some(ref pk) = priv_key {
            if pk.lambda <= 0 || pk.mu <= 0 {
                return Err(ContractError::InvalidInput);
            }
        }

        env.storage().instance().set(&PUB_KEY, &pub_key);
        if let Some(pk) = priv_key {
            env.storage().instance().set(&PRIV_KEY, &pk);
        } else {
            env.storage().instance().remove(&PRIV_KEY);
        }

        let new_ver = Self::get_dep_ver(env.clone()) + 1;
        env.storage().instance().set(&DEP_VER, &new_ver);

        env.events().publish(
            (symbol_short!("DEP_UP"), symbol_short!("KEYS"), admin.clone()),
            events::DependencyUpdatedEvent {
                admin,
                effect: symbol_short!("KEYS"),
                new_version: new_ver,
            },
        );
        Ok(())
    }

    pub fn get_admin(env: Env) -> Address {
        env.storage().instance().get(&ADMIN).unwrap()
    }

    pub fn get_aggregator(env: Env) -> Address {
        env.storage().instance().get(&AGGREGATOR).unwrap()
    }

    pub fn import_metric_from_source(
        env: Env,
        caller: Address,
        source: Address,
        kind: Symbol,
        dims: MetricDimensions,
    ) -> Result<MetricValue, ContractError> {
        caller.require_auth();
        let aggregator = Self::get_aggregator(env.clone());
        if caller != aggregator {
            return Err(ContractError::Unauthorized);
        }

        let mut imported = match MetricSourceClient::new(&env, &source).try_read_metric(&kind, &dims) {
            Ok(Ok(value)) => value,
            _ => return Err(ContractError::ExternalCallFailed),
        };

        imported.version = Self::get_dep_ver(env.clone());

        let key = (METRIC, kind.clone(), dims.clone());
        env.storage().persistent().set(&key, &imported);
        env.events().publish(
            (symbol_short!("M_IMPORT"), kind, caller.clone()),
            events::MetricImportedEvent {
                caller,
                source,
                kind: key.1.clone(),
                dims,
                value: imported.clone(),
            },
        );

        Ok(imported)
    }

    // ── Homomorphic Operations ────────────────────────────────────────────────

    pub fn encrypt(env: Env, m: i128) -> i128 {
        let pub_key: PaillierPublicKey = env.storage().instance().get(&PUB_KEY).unwrap();
        HomomorphicEngine::encrypt(&env, &pub_key, m)
    }

    pub fn add_ciphertexts(env: Env, c1: i128, c2: i128) -> i128 {
        let pub_key: PaillierPublicKey = env.storage().instance().get(&PUB_KEY).unwrap();
        HomomorphicEngine::add_ciphertexts(&pub_key, c1, c2)
    }

    pub fn decrypt(env: Env, caller: Address, c: i128) -> Result<i128, ContractError> {
        caller.require_auth();
        let aggregator: Address = env.storage().instance().get(&AGGREGATOR).unwrap();
        if caller != aggregator {
            return Err(ContractError::Unauthorized);
        }
        let pub_key: PaillierPublicKey = env.storage().instance().get(&PUB_KEY).unwrap();
        let priv_key: PaillierPrivateKey = env
            .storage()
            .instance()
            .get(&PRIV_KEY)
            .ok_or(ContractError::Unauthorized)?;
        Ok(HomomorphicEngine::decrypt(&pub_key, &priv_key, c))
    }

    // ── Aggregation ──────────────────────────────────────────────────────────

    pub fn aggregate_records(
        env: Env,
        caller: Address,
        kind: Symbol,
        dims: MetricDimensions,
        ciphertexts: Vec<i128>,
    ) -> Result<(), ContractError> {
        caller.require_auth();
        let aggregator = Self::get_aggregator(env.clone());
        if caller != aggregator {
            return Err(ContractError::Unauthorized);
        }
        if ciphertexts.is_empty() {
            return Err(ContractError::InvalidInput);
        }

        let pub_key: PaillierPublicKey = env.storage().instance().get(&PUB_KEY).unwrap();
        let agg_ciphertext = Aggregator::aggregate_sum(&pub_key, ciphertexts.clone());

        // For this demo, we "record" the decrypted value with DP noise
        let priv_key: PaillierPrivateKey = env.storage().instance().get(&PRIV_KEY).unwrap();
        let plaintext_sum = HomomorphicEngine::decrypt(&pub_key, &priv_key, agg_ciphertext);

        let noisy_sum = DifferentialPrivacy::add_laplace_noise(&env, plaintext_sum, 1, 10);
        let count = ciphertexts.len() as i128;

        let key = (METRIC, kind.clone(), dims.clone());
        let mut current: MetricValue = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or(MetricValue { count: 0, sum: 0, version: 0 });

        let dep_ver = Self::get_dep_ver(env.clone());
        if current.version < dep_ver {
             env.events().publish(
                (symbol_short!("M_INVALID"), kind.clone(), caller.clone()),
                events::DataInvalidatedEvent {
                    kind: kind.clone(),
                    dims: dims.clone(),
                    old_version: current.version,
                    current_version: dep_ver,
                },
            );
            current.count = 0;
            current.sum = 0;
        }

        current.count = current.count.saturating_add(count);
        current.sum = current.sum.saturating_add(noisy_sum);
        current.version = dep_ver;

        env.storage().persistent().set(&key, &current);
        env.events().publish(
            (symbol_short!("M_AGG"), kind, caller.clone()),
            events::MetricAggregatedEvent {
                caller,
                kind: key.1.clone(),
                dims,
                value: current.clone(),
            },
        );
        Ok(())
    }

    pub fn aggregate_records_in_window(
        env: Env,
        caller: Address,
        kind: Symbol,
        dims: MetricDimensions,
        ciphertexts: Vec<i128>,
        not_before: u64,
        expires_at: u64,
    ) -> Result<(), ContractError> {
        let now = env.ledger().timestamp();
        if now < not_before {
            return Err(ContractError::TimelockNotMet);
        }
        if now > expires_at {
            return Err(ContractError::SubmissionExpired);
        }

        Self::aggregate_records(env, caller, kind, dims, ciphertexts)
    }

    pub fn get_metric(env: Env, kind: Symbol, dims: MetricDimensions) -> MetricValue {
        let key = (METRIC, kind, dims);
        let stored: MetricValue = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or(MetricValue { count: 0, sum: 0, version: 0 });
        
        if stored.version < Self::get_dep_ver(env.clone()) {
            return MetricValue { count: 0, sum: 0, version: 0 };
        }
        stored
    }

    pub fn get_trend(
        env: Env,
        kind: Symbol,
        region: Option<Symbol>,
        age_band: Option<Symbol>,
        condition: Option<Symbol>,
        start_bucket: u64,
        end_bucket: u64,
    ) -> Vec<TrendPoint> {
        let mut out = Vec::new(&env);
        for bucket in start_bucket..=end_bucket {
            let dims = MetricDimensions {
                region: region.clone(),
                age_band: age_band.clone(),
                condition: condition.clone(),
                time_bucket: bucket,
            };
            out.push_back(TrendPoint {
                time_bucket: bucket,
                value: Self::get_metric(env.clone(), kind.clone(), dims),
            });
        }
        out
    }
}
