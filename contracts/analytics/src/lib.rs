pub mod aggregation;
pub mod differential_privacy;
pub mod homomorphic;

#[cfg(test)]
mod test;

use soroban_sdk::{contract, contractimpl, contracttype, symbol_short, Address, Env, Symbol, Vec};

use crate::aggregation::Aggregator;
use crate::differential_privacy::DifferentialPrivacy;
use crate::homomorphic::{HomomorphicEngine, PaillierPrivateKey, PaillierPublicKey};

// ── Storage keys ────────────────────────────────────────────────────────────────

const ADMIN: Symbol = symbol_short!("ADMIN");
const AGGREGATOR: Symbol = symbol_short!("AGGR");
const METRIC: Symbol = symbol_short!("METRIC");
const PUB_KEY: Symbol = symbol_short!("PUB_KEY");
const PRIV_KEY: Symbol = symbol_short!("PRIV_KEY");

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
}

#[contracttype]
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TrendPoint {
    pub time_bucket: u64,
    pub value: MetricValue,
}

#[soroban_sdk::contracterror]
#[derive(Copy, Clone, Debug, Eq, PartialEq)]
#[repr(u32)]
pub enum ContractError {
    AlreadyInitialized = 1,
    NotInitialized = 2,
    Unauthorized = 3,
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
        env.storage().instance().set(&ADMIN, &admin);
        env.storage().instance().set(&AGGREGATOR, &aggregator);
        env.storage().instance().set(&PUB_KEY, &pub_key);
        if let Some(pk) = priv_key {
            env.storage().instance().set(&PRIV_KEY, &pk);
        }
        Ok(())
    }

    pub fn get_admin(env: Env) -> Address {
        env.storage().instance().get(&ADMIN).unwrap()
    }

    pub fn get_aggregator(env: Env) -> Address {
        env.storage().instance().get(&AGGREGATOR).unwrap()
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

        let pub_key: PaillierPublicKey = env.storage().instance().get(&PUB_KEY).unwrap();
        let agg_ciphertext = Aggregator::aggregate_sum(&pub_key, ciphertexts.clone());

        // For this demo, we "record" the decrypted value with DP noise
        let priv_key: PaillierPrivateKey = env.storage().instance().get(&PRIV_KEY).unwrap();
        let plaintext_sum = HomomorphicEngine::decrypt(&pub_key, &priv_key, agg_ciphertext);

        let noisy_sum = DifferentialPrivacy::add_laplace_noise(&env, plaintext_sum, 1, 10);
        let count = ciphertexts.len() as i128;

        let key = (METRIC, kind, dims);
        let mut current: MetricValue = env
            .storage()
            .persistent()
            .get(&key)
            .unwrap_or(MetricValue { count: 0, sum: 0 });

        current.count = current.count.saturating_add(count);
        current.sum = current.sum.saturating_add(noisy_sum);

        env.storage().persistent().set(&key, &current);
        Ok(())
    }

    pub fn get_metric(env: Env, kind: Symbol, dims: MetricDimensions) -> MetricValue {
        let key = (METRIC, kind, dims);
        env.storage()
            .persistent()
            .get(&key)
            .unwrap_or(MetricValue { count: 0, sum: 0 })
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
