# Rate Limiting & DoS Protection

This document consolidates rate limiting and DoS protection strategies across the Stellar Teye platform.

## 🛡️ DoS Protection Overview

The Stellar Teye platform implements multiple layers of DoS protection to ensure availability and prevent abuse:

- **Rate Limiting**: Per-user and per-operation limits
- **Circuit Breakers**: Automatic service degradation
- **Resource Quotas**: Computational and storage limits
- **Monitoring**: Real-time threat detection

## 📊 Rate Limiting Architecture

### Sliding Window Algorithm

**Implementation** (from `contracts/common/src/rate_limit.rs`):

```rust
pub struct RateLimiter {
    window_size: u64,      // Time window in seconds
    max_requests: u64,     // Max requests per window
    requests: Vec<(u64, u64)>, // (timestamp, count) pairs
}

impl RateLimiter {
    pub fn check_rate_limit(&mut self, timestamp: u64) -> bool {
        // Remove expired entries
        self.cleanup_expired(timestamp);
        
        // Count current requests
        let current_count = self.requests.iter().map(|(_, count)| count).sum();
        
        if current_count >= self.max_requests {
            return false;
        }
        
        // Add new request
        self.requests.push((timestamp, 1));
        true
    }
}
```

**Properties**:
- **Memory Efficient**: O(k) where k is window size
- **Accurate**: Precise rate limiting within window
- **Flexible**: Configurable windows and limits

### Per-Contract Rate Limits

| Contract | Operation | Window | Limit | Purpose |
|-----------|-------------|----------|--------|
| vision_records | register_patient | 1 hour | 5 per hour |
| vision_records | add_vision_record | 1 minute | 10 per minute |
| vision_records | get_patient_records | 1 minute | 100 per minute |
| governor | create_proposal | 1 day | 10 per day |
| governor | vote | 1 hour | 50 per hour |
| staking | stake | 1 minute | 20 per minute |
| staking | unstake | 1 hour | 10 per hour |
| analytics | query | 1 minute | 200 per minute |

## ⚡ Circuit Breaker Pattern

**Implementation** (from `contracts/vision_records/src/circuit_breaker.rs`):

```rust
pub struct CircuitBreaker {
    failure_threshold: u64,
    success_threshold: u64,
    timeout: u64,
    state: CircuitState,
    failure_count: u64,
    success_count: u64,
    last_failure_time: u64,
}

#[derive(Clone, Debug)]
pub enum CircuitState {
    Closed,     // Normal operation
    Open,       // Failing, reject requests
    HalfOpen,   // Testing recovery
}

impl CircuitBreaker {
    pub fn call<F, R>(&mut self, f: F) -> Result<R, CircuitBreakerError>
    where
        F: FnOnce() -> Result<R, Box<dyn std::error::Error>>,
    {
        match self.state {
            CircuitState::Open => {
                if self.should_attempt_reset() {
                    self.state = CircuitState::HalfOpen;
                } else {
                    return Err(CircuitBreakerError::CircuitOpen);
                }
            }
            _ => {}
        }
        
        match f() {
            Ok(result) => {
                self.on_success();
                Ok(result)
            }
            Err(error) => {
                self.on_failure();
                Err(CircuitBreakerError::CallFailed(error))
            }
        }
    }
}
```

**Circuit Breaker Configuration**:

| Service | Failure Threshold | Success Threshold | Timeout | Purpose |
|----------|------------------|-------------------|----------|---------|
| Database | 5 failures | 3 successes | 60 seconds | Database protection |
| External API | 10 failures | 5 successes | 30 seconds | API protection |
| ZK Verification | 3 failures | 2 successes | 120 seconds | ZK service protection |
| Analytics Engine | 8 failures | 4 successes | 45 seconds | Analytics protection |

## 🔒 Reentrancy Guards

**Implementation** (from `contracts/common/src/reentrancy_guard.rs`):

```rust
pub struct ReentrancyGuard {
    entered: bool,
}

impl ReentrancyGuard {
    pub fn enter(&mut self) -> Result<(), ReentrancyError> {
        if self.entered {
            return Err(ReentrancyError::ReentrantCall);
        }
        self.entered = true;
        Ok(())
    }
    
    pub fn exit(&mut self) {
        self.entered = false;
    }
}

// Usage in contracts
#[contract]
pub struct VisionRecordsContract {
    reentrancy_guard: ReentrancyGuard,
}

#[contractimpl]
impl VisionRecordsContract {
    pub fn add_vision_record(&mut self, record: VisionRecord) -> Result<u64, ContractError> {
        self.reentrancy_guard.enter()?;
        
        // Contract logic here
        
        self.reentrancy_guard.exit();
        Ok(record_id)
    }
}
```

## 📈 Resource Quotas

### Computational Limits

| Resource | Limit | Rationale |
|-----------|--------|-----------|
| Gas per Transaction | 5,000,000 | Prevent infinite loops |
| Contract Size | 2 MB | Limit deployment size |
| Storage per Contract | 100 MB | Prevent storage abuse |
| Number of Contracts | 1,000 per user | Prevent spam |
| Total Storage per User | 1 GB | Fair resource allocation |

### Data Transfer Limits

| Operation | Limit | Purpose |
|-----------|--------|---------|
| Record Upload | 10 MB per file | Prevent storage abuse |
| Batch Operations | 100 records per batch | Limit computational load |
| API Response Size | 1 MB | Prevent bandwidth abuse |
| Concurrent Connections | 100 per user | Prevent connection abuse |

## 🚨 Threat Detection

### Anomaly Detection

**Metrics Monitored**:
- **Request Rate**: Sudden spikes in requests
- **Error Rate**: Unusual error patterns
- **Response Time**: Performance degradation
- **Resource Usage**: CPU, memory, storage spikes

**Detection Algorithms**:
```rust
pub struct AnomalyDetector {
    baseline_mean: f64,
    baseline_std: f64,
    threshold_multiplier: f64,
}

impl AnomalyDetector {
    pub fn is_anomaly(&self, current_value: f64) -> bool {
        let z_score = (current_value - self.baseline_mean) / self.baseline_std;
        z_score.abs() > self.threshold_multiplier
    }
}
```

### Attack Pattern Recognition

**Common Attack Patterns**:

1. **Brute Force Attacks**
   - High rate of authentication attempts
   - Repeated failed logins
   - Mitigation: Exponential backoff, account lockout

2. **Resource Exhaustion**
   - Large file uploads
   - Complex query parameters
   - Mitigation: Size limits, complexity limits

3. **Sybil Attacks**
   - Multiple fake accounts
   - Coordinated actions
   - Mitigation: Identity verification, staking requirements

4. **Flash Loan Attacks**
   - Rapid borrowing and repayment
   - Price manipulation
   - Mitigation: Time delays, voting locks

## 🛠️ Implementation Details

### Rate Limiting Storage

```rust
// Storage structure for rate limiting
struct RateLimitStorage {
    user_id: Address,
    operation: String,
    window_start: u64,
    request_count: u64,
}

// Storage key generation
fn rate_limit_key(user: &Address, operation: &str) -> (Symbol, Address, Symbol) {
    (
        symbol_short!("RATE_LIMIT"),
        user.clone(),
        symbol_short!(operation),
    )
}
```

### Circuit Breaker Integration

```rust
// Contract-level circuit breaker
#[contract]
pub struct ProtectedContract {
    circuit_breaker: CircuitBreaker,
    rate_limiter: RateLimiter,
}

#[contractimpl]
impl ProtectedContract {
    pub fn protected_operation(&mut self, user: Address, data: Vec<u8>) -> Result<(), Error> {
        // Check rate limit first
        if !self.rate_limiter.check_rate_limit(env.ledger().timestamp()) {
            return Err(Error::RateLimitExceeded);
        }
        
        // Check circuit breaker
        self.circuit_breaker.call(|| {
            // Execute protected operation
            self.execute_operation(user, data)
        })
    }
}
```

## 📊 Monitoring and Alerting

### Key Metrics

| Metric | Threshold | Alert Level | Action |
|---------|------------|--------------|--------|
| Request Rate | > 1000 req/min | High | Scale up resources |
| Error Rate | > 5% | Medium | Investigate errors |
| Response Time | > 5 seconds | Medium | Optimize code |
| Circuit Breaker | Open | Critical | Manual intervention |
| Resource Usage | > 80% | High | Scale resources |

### Alert Configuration

```rust
pub struct AlertConfig {
    pub metric_name: String,
    pub threshold: f64,
    pub comparison: Comparison,
    pub severity: AlertSeverity,
    pub action: AlertAction,
}

pub enum Comparison {
    GreaterThan,
    LessThan,
    Equals,
}

pub enum AlertSeverity {
    Info,
    Warning,
    Error,
    Critical,
}
```

## 🔧 Configuration Management

### Environment-Specific Limits

| Environment | Rate Limits | Circuit Breaker | Monitoring |
|-------------|---------------|------------------|------------|
| Development | Relaxed | Disabled | Basic |
| Testing | Standard | Enabled | Full |
| Staging | Production-like | Enabled | Full |
| Production | Strict | Enabled | Full |

### Dynamic Configuration

```rust
pub struct RateLimitConfig {
    pub window_size: u64,
    pub max_requests: u64,
    pub penalty_duration: u64,
}

// Admin can update configuration
pub fn update_rate_limit_config(
    &mut self,
    admin: Address,
    config: RateLimitConfig,
) -> Result<(), Error> {
    self.require_admin(admin)?;
    self.rate_limit_config = config;
    Ok(())
}
```

## 🧪 Testing DoS Protection

### Load Testing

```rust
#[cfg(test)]
mod dos_tests {
    use super::*;
    
    #[test]
    fn test_rate_limiting() {
        let mut rate_limiter = RateLimiter::new(60, 10); // 10 per minute
        
        let timestamp = 1000;
        for i in 0..15 {
            let allowed = rate_limiter.check_rate_limit(timestamp + i);
            if i < 10 {
                assert!(allowed);
            } else {
                assert!(!allowed);
            }
        }
    }
    
    #[test]
    fn test_circuit_breaker() {
        let mut circuit_breaker = CircuitBreaker::new(3, 2, 60);
        
        // Trigger failures
        for _ in 0..3 {
            let result = circuit_breaker.call(|| Err("error".into()));
            assert!(result.is_err());
        }
        
        // Circuit should be open
        let result = circuit_breaker.call(|| Ok("success"));
        assert!(matches!(result, Err(CircuitBreakerError::CircuitOpen)));
    }
}
```

### Fuzz Testing

```rust
fuzz_target!(|operations: Vec<DosOperation>| {
    let mut contract = setup_test_contract();
    
    for operation in operations {
        let result = contract.handle_operation(operation);
        
        // Should never panic or crash
        assert!(!result.is_panic());
        
        // Should handle gracefully
        assert!(result.is_ok() || is_expected_error(result.err()));
    }
});
```

## 📋 Best Practices

### Development Guidelines

1. **Always Check Rate Limits**: Before processing any request
2. **Use Circuit Breakers**: For external dependencies
3. **Implement Reentrancy Guards**: Prevent recursive calls
4. **Set Resource Limits**: Prevent resource exhaustion
5. **Monitor Everything**: Comprehensive logging and metrics

### Operational Guidelines

1. **Regular Review**: Adjust limits based on usage patterns
2. **Incident Response**: Clear procedures for DoS attacks
3. **Capacity Planning**: Scale resources proactively
4. **Security Updates**: Regular security patches
5. **Testing**: Regular DoS testing and drills

## 📚 References

- [Rate Limiting Best Practices](https://stripe.com/blog/rate-limiters)
- [Circuit Breaker Pattern](https://martinfowler.com/bliki/CircuitBreaker.html)
- [DoS Attack Mitigation](https://owasp.org/www-project-ddos-protection/)
- [Reentrancy Attacks](https://consensys.github.io/smart-contract-best-practices/attacks/reentrancy/)

---

**Last Updated**: 2025-02-25  
**Next Review**: 2025-03-25  
**Version**: 1.0
