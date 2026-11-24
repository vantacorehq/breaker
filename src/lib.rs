//! breaker — pure Rust circuit breaker.
//! Closed → Open → Half-Open. Zero dependencies.
//!
//! ## Thread safety
//!
//! `Breaker` is intentionally single-threaded (`&mut self`).  
//! For multi-threaded use either:
//! - wrap it in `std::sync::Mutex` / `RwLock`, or
//! - use the provided [`SyncBreaker`] which does that for you.

use std::sync::{Mutex, MutexGuard};
use std::time::{Duration, Instant};

/// Circuit state.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Closed,
    Open,
    HalfOpen,
}

/// Live counters.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Stats {
    pub total_calls: u64,
    pub successes: u64,
    pub failures: u64,
    pub times_opened: u64,
    pub rejected: u64,
}

/// Configuration for the breaker.
#[derive(Debug, Clone)]
pub struct Config {
    /// Consecutive failures that trip the breaker into Open.
    pub failure_threshold: u32,
    /// Consecutive successes in Half-Open required to close.
    pub success_threshold: u32,
    /// How long the circuit stays Open before becoming Half-Open.
    pub open_duration: Duration,
    /// Maximum number of probe calls allowed while in Half-Open.
    /// Prevents a thundering herd when the circuit first opens for probes.
    pub half_open_max_calls: u32,
}

impl Default for Config {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            success_threshold: 2,
            open_duration: Duration::from_secs(30),
            half_open_max_calls: 1,
        }
    }
}

impl Config {
    pub fn new(
        failure_threshold: u32,
        success_threshold: u32,
        open_duration: Duration,
    ) -> Self {
        Self {
            failure_threshold,
            success_threshold,
            open_duration,
            half_open_max_calls: 1,
        }
    }

    pub fn with_half_open_max_calls(mut self, max: u32) -> Self {
        self.half_open_max_calls = max.max(1);
        self
    }
}

/// Circuit breaker (single-threaded).
///
/// Requires exclusive `&mut self` access. For concurrent use see [`SyncBreaker`].
#[derive(Debug)]
pub struct Breaker {
    config: Config,
    state: State,
    failure_count: u32,
    success_count: u32,
    /// Number of calls currently allowed / in-flight while Half-Open.
    half_open_inflight: u32,
    opened_at: Option<Instant>,
    stats: Stats,
}

impl Breaker {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            state: State::Closed,
            failure_count: 0,
            success_count: 0,
            half_open_inflight: 0,
            opened_at: None,
            stats: Stats::default(),
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(Config::default())
    }

    /// Returns `true` if the call is allowed to proceed.
    pub fn allow(&mut self) -> bool {
        self.maybe_transition();

        let allowed = match self.state {
            State::Closed => true,
            State::Open => false,
            State::HalfOpen => {
                if self.half_open_inflight < self.config.half_open_max_calls {
                    self.half_open_inflight += 1;
                    true
                } else {
                    false
                }
            }
        };

        if allowed {
            self.stats.total_calls += 1;
        } else {
            self.stats.rejected += 1;
        }
        allowed
    }

    /// Record a successful call.
    pub fn on_success(&mut self) {
        self.maybe_transition();
        self.stats.successes += 1;

        match self.state {
            State::Closed => {
                self.failure_count = 0;
            }
            State::HalfOpen => {
                self.half_open_inflight = self.half_open_inflight.saturating_sub(1);
                self.success_count += 1;
                if self.success_count >= self.config.success_threshold {
                    self.transition_to_closed();
                }
            }
            State::Open => {}
        }
    }

    /// Record a failed call.
    pub fn on_failure(&mut self) {
        self.maybe_transition();
        self.stats.failures += 1;

        match self.state {
            State::Closed => {
                self.failure_count += 1;
                if self.failure_count >= self.config.failure_threshold {
                    self.transition_to_open();
                }
            }
            State::HalfOpen => {
                self.half_open_inflight = self.half_open_inflight.saturating_sub(1);
                // any failure while probing re-opens the circuit
                self.transition_to_open();
            }
            State::Open => {}
        }
    }

    /// Convenience wrapper: check state, run the closure, update counters.
    ///
    /// Returns `None` when the circuit is open (call was rejected).
    /// Returns `Some(result)` otherwise.
    pub fn call<T, E>(&mut self, f: impl FnOnce() -> Result<T, E>) -> Option<Result<T, E>> {
        if !self.allow() {
            return None;
        }
        match f() {
            Ok(v) => {
                self.on_success();
                Some(Ok(v))
            }
            Err(e) => {
                self.on_failure();
                Some(Err(e))
            }
        }
    }

    pub fn state(&self) -> State {
        self.state
    }

    pub fn stats(&self) -> Stats {
        self.stats
    }

    /// Force the circuit back to Closed.
    pub fn reset(&mut self) {
        self.transition_to_closed();
    }

    fn maybe_transition(&mut self) {
        if self.state == State::Open {
            if let Some(opened) = self.opened_at {
                if opened.elapsed() >= self.config.open_duration {
                    self.transition_to_half_open();
                }
            }
        }
    }

    fn transition_to_open(&mut self) {
        self.state = State::Open;
        self.opened_at = Some(Instant::now());
        self.failure_count = 0;
        self.success_count = 0;
        self.half_open_inflight = 0;
        self.stats.times_opened += 1;
    }

    fn transition_to_half_open(&mut self) {
        self.state = State::HalfOpen;
        self.success_count = 0;
        self.failure_count = 0;
        self.half_open_inflight = 0;
        self.opened_at = None;
    }

    fn transition_to_closed(&mut self) {
        self.state = State::Closed;
        self.failure_count = 0;
        self.success_count = 0;
        self.half_open_inflight = 0;
        self.opened_at = None;
    }
}

// ─────────────────────────────────────────────────────────────
// Thread-safe wrapper
// ─────────────────────────────────────────────────────────────

/// Thread-safe circuit breaker.
///
/// Wraps [`Breaker`] in a `Mutex`. All methods take `&self`.
#[derive(Debug)]
pub struct SyncBreaker {
    inner: Mutex<Breaker>,
}

impl SyncBreaker {
    pub fn new(config: Config) -> Self {
        Self {
            inner: Mutex::new(Breaker::new(config)),
        }
    }

    pub fn with_defaults() -> Self {
        Self::new(Config::default())
    }

    fn lock(&self) -> MutexGuard<'_, Breaker> {
        self.inner.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn allow(&self) -> bool {
        self.lock().allow()
    }

    pub fn on_success(&self) {
        self.lock().on_success()
    }

    pub fn on_failure(&self) {
        self.lock().on_failure()
    }

    pub fn call<T, E>(&self, f: impl FnOnce() -> Result<T, E>) -> Option<Result<T, E>> {
        self.lock().call(f)
    }

    pub fn state(&self) -> State {
        self.lock().state()
    }

    pub fn stats(&self) -> Stats {
        self.lock().stats()
    }

    pub fn reset(&self) {
        self.lock().reset()
    }
}

// ─────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use std::thread;

    #[test]
    fn starts_closed() {
        let cb = Breaker::with_defaults();
        assert_eq!(cb.state(), State::Closed);
        assert_eq!(cb.stats().total_calls, 0);
    }

    #[test]
    fn opens_after_threshold() {
        let mut cb = Breaker::new(Config::new(3, 1, Duration::from_secs(60)));

        assert!(cb.allow());
        cb.on_failure();
        assert!(cb.allow());
        cb.on_failure();
        assert!(cb.allow());
        cb.on_failure();

        assert_eq!(cb.state(), State::Open);
        assert!(!cb.allow());
        assert_eq!(cb.stats().times_opened, 1);
        assert_eq!(cb.stats().failures, 3);
        assert!(cb.stats().rejected >= 1);
    }

    #[test]
    fn success_resets_failure_count() {
        let mut cb = Breaker::new(Config::new(3, 1, Duration::from_secs(60)));

        cb.on_failure();
        cb.on_failure();
        cb.on_success();
        cb.on_failure();
        cb.on_failure();
        assert_eq!(cb.state(), State::Closed);
        cb.on_failure();
        assert_eq!(cb.state(), State::Open);
    }

    #[test]
    fn half_open_then_close() {
        let mut cb = Breaker::new(
            Config::new(2, 2, Duration::from_millis(40)).with_half_open_max_calls(2),
        );

        cb.on_failure();
        cb.on_failure();
        assert_eq!(cb.state(), State::Open);

        thread::sleep(Duration::from_millis(50));
        assert!(cb.allow());
        assert_eq!(cb.state(), State::HalfOpen);

        cb.on_success();
        assert_eq!(cb.state(), State::HalfOpen);
        cb.on_success();
        assert_eq!(cb.state(), State::Closed);
    }

    #[test]
    fn half_open_max_calls_respected() {
        let mut cb = Breaker::new(
            Config::new(1, 3, Duration::from_millis(20)).with_half_open_max_calls(1),
        );

        cb.on_failure();
        assert_eq!(cb.state(), State::Open);

        thread::sleep(Duration::from_millis(30));

        // first probe allowed
        assert!(cb.allow());
        assert_eq!(cb.state(), State::HalfOpen);

        // second probe rejected because half_open_max_calls = 1
        assert!(!cb.allow());
        assert_eq!(cb.stats().rejected, 1);
    }

    #[test]
    fn half_open_failure_reopens() {
        let mut cb = Breaker::new(Config::new(1, 2, Duration::from_millis(30)));

        cb.on_failure();
        assert_eq!(cb.state(), State::Open);

        thread::sleep(Duration::from_millis(40));
        assert!(cb.allow());
        assert_eq!(cb.state(), State::HalfOpen);

        cb.on_failure();
        assert_eq!(cb.state(), State::Open);
        assert!(!cb.allow());
        assert_eq!(cb.stats().times_opened, 2);
    }

    #[test]
    fn call_helper_success() {
        let mut cb = Breaker::with_defaults();
        let result = cb.call(|| Ok::<_, ()>(42));
        assert_eq!(result, Some(Ok(42)));
        assert_eq!(cb.stats().successes, 1);
    }

    #[test]
    fn call_helper_failure() {
        let mut cb = Breaker::new(Config::new(1, 1, Duration::from_secs(60)));
        let result = cb.call(|| Err::<(), _>("boom"));
        assert_eq!(result, Some(Err("boom")));
        assert_eq!(cb.state(), State::Open);

        // now rejected
        let rejected = cb.call(|| Ok::<_, ()>(1));
        assert_eq!(rejected, None);
    }

    #[test]
    fn reset_works() {
        let mut cb = Breaker::new(Config::new(1, 1, Duration::from_secs(60)));
        cb.on_failure();
        assert_eq!(cb.state(), State::Open);
        cb.reset();
        assert_eq!(cb.state(), State::Closed);
        assert!(cb.allow());
    }

    #[test]
    fn sync_breaker_basic() {
        let cb = SyncBreaker::new(Config::new(2, 1, Duration::from_secs(10)));
        assert!(cb.allow());
        cb.on_failure();
        assert!(cb.allow());
        cb.on_failure();
        assert_eq!(cb.state(), State::Open);
        assert!(!cb.allow());
    }
}
