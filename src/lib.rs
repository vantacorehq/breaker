//! breaker — pure Rust circuit breaker.
//! Closed → Open → Half-Open. Zero dependencies.

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
}

impl Default for Config {
    fn default() -> Self {
        Self {
            failure_threshold: 5,
            success_threshold: 2,
            open_duration: Duration::from_secs(30),
        }
    }
}

impl Config {
    pub fn new(failure_threshold: u32, success_threshold: u32, open_duration: Duration) -> Self {
        Self {
            failure_threshold,
            success_threshold,
            open_duration,
        }
    }
}

/// Circuit breaker.
#[derive(Debug)]
pub struct Breaker {
    config: Config,
    state: State,
    failure_count: u32,
    success_count: u32,
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
            opened_at: None,
            stats: Stats::default(),
        }
    }

    /// Convenience constructor with common defaults.
    pub fn with_defaults() -> Self {
        Self::new(Config::default())
    }

    /// Returns `true` if the call is allowed to proceed.
    pub fn allow(&mut self) -> bool {
        self.maybe_transition();
        let allowed = matches!(self.state, State::Closed | State::HalfOpen);
        if allowed {
            self.stats.total_calls += 1;
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
                self.success_count += 1;
                if self.success_count >= self.config.success_threshold {
                    self.transition_to_closed();
                }
            }
            State::Open => {
                // should not reach here if allow() was respected
            }
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
                // any failure while probing re-opens the circuit
                self.transition_to_open();
            }
            State::Open => {}
        }
    }

    pub fn state(&self) -> State {
        self.state
    }

    pub fn stats(&self) -> Stats {
        self.stats
    }

    /// Force the circuit back to Closed and clear consecutive counters.
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
        self.stats.times_opened += 1;
    }

    fn transition_to_half_open(&mut self) {
        self.state = State::HalfOpen;
        self.success_count = 0;
        self.failure_count = 0;
        self.opened_at = None;
    }

    fn transition_to_closed(&mut self) {
        self.state = State::Closed;
        self.failure_count = 0;
        self.success_count = 0;
        self.opened_at = None;
    }
}

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
    }

    #[test]
    fn success_resets_failure_count() {
        let mut cb = Breaker::new(Config::new(3, 1, Duration::from_secs(60)));

        cb.on_failure();
        cb.on_failure();
        cb.on_success(); // resets consecutive failures
        cb.on_failure();
        cb.on_failure();
        assert_eq!(cb.state(), State::Closed);
        cb.on_failure();
        assert_eq!(cb.state(), State::Open);
    }

    #[test]
    fn half_open_then_close() {
        let mut cb = Breaker::new(Config::new(2, 2, Duration::from_millis(40)));

        cb.on_failure();
        cb.on_failure();
        assert_eq!(cb.state(), State::Open);

        thread::sleep(Duration::from_millis(50));
        assert!(cb.allow()); // → HalfOpen
        assert_eq!(cb.state(), State::HalfOpen);

        cb.on_success();
        assert_eq!(cb.state(), State::HalfOpen);
        cb.on_success();
        assert_eq!(cb.state(), State::Closed);
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
    fn reset_works() {
        let mut cb = Breaker::new(Config::new(1, 1, Duration::from_secs(60)));
        cb.on_failure();
        assert_eq!(cb.state(), State::Open);
        cb.reset();
        assert_eq!(cb.state(), State::Closed);
        assert!(cb.allow());
    }

    #[test]
    fn stats_accumulate() {
        let mut cb = Breaker::new(Config::new(5, 2, Duration::from_secs(10)));

        for _ in 0..3 {
            assert!(cb.allow());
            cb.on_success();
        }
        assert!(cb.allow());
        cb.on_failure();

        let s = cb.stats();
        assert_eq!(s.total_calls, 4);
        assert_eq!(s.successes, 3);
        assert_eq!(s.failures, 1);
        assert_eq!(s.times_opened, 0);
    }
}
