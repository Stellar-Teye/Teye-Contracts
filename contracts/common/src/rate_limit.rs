//! Simple, host-side rate limiting helpers that mirror the on-chain pattern.
//!
//! This module is intentionally `std`-only and does not depend on `soroban_sdk`
//! so it can be used from off-chain tooling or simulations.

/// Configuration for a fixed-window rate limiter.
#[derive(Clone, Debug)]
pub struct RateLimiterConfig {
    pub max_requests_per_window: u64,
    pub window_duration_seconds: u64,
}

impl RateLimiterConfig {
    pub fn new(max_requests_per_window: u64, window_duration_seconds: u64) -> Self {
        Self {
            max_requests_per_window,
            window_duration_seconds,
        }
    }

    /// Returns `true` if the configuration represents an enabled limiter.
    pub fn is_enabled(&self) -> bool {
        self.max_requests_per_window > 0 && self.window_duration_seconds > 0
    }
}

/// Per-identity rate limiting state.
///
/// This mirrors the `(count, window_start)` tuple that on-chain contracts
/// persist using Soroban storage.
#[derive(Clone, Debug)]
pub struct RateLimiterState {
    /// Number of requests seen in the current window.
    pub count: u64,
    /// Timestamp (in seconds) when the current window started.
    pub window_start: u64,
}

impl RateLimiterState {
    /// Creates an empty state starting at `now`.
    pub fn new(now: u64) -> Self {
        Self {
            count: 0,
            window_start: now,
        }
    }

    /// Records a single hit at `now` using `cfg`.
    ///
    /// Returns `true` if the hit is allowed, or `false` if it exceeds the
    /// configured limit for the current window.
    pub fn record_hit(&mut self, now: u64, cfg: &RateLimiterConfig) -> bool {
        if !cfg.is_enabled() {
            return true;
        }

        // Reset the window if it has fully elapsed.
        let window_end = self
            .window_start
            .saturating_add(cfg.window_duration_seconds);
        if now >= window_end {
            self.window_start = now;
            self.count = 0;
        }

        let next = self.count.saturating_add(1);
        if next > cfg.max_requests_per_window {
            return false;
        }

        self.count = next;
        true
    }
}
