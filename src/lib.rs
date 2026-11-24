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

/// Configuration for the breaker.
#[derive(Debug, Clone)]
pub struct Config {
    /// Number of consecutive failures that open the circuit.
    pub failure_threshold: u32,
    /// Number of consecutive successes in Half-Open required to close.
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

/// Circuit breaker.
#[derive(Debug)]
pub struct Breaker {
    config: Config,
    state: State,
    failure_count: u32,
    success_count: u32,
    opened_at: Option<Instant>,
}

impl Breaker {
    pub fn new(config: Config) -> Self {
        Self {
            config,
            state: State::Closed,
            failure_count: 0,
            success_count: 0,
            opened_at: None,
        }
    }

    /// Returns `true` if the call is allowed to proceed.
    pub fn allow(&mut self) -> bool {
        self.maybe_transition();
        match self.state {
            State::Closed | State::HalfOpen => true,
            State::Open => false,
        }
    }

    /// Record a successful call.
    pub fn on_success(&mut self) {
        self.maybe_transition();
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
                // should not happen if allow() was checked, but be defensive
            }
        }
    }

    /// Record a failed call.
    pub fn on_failure(&mut self) {
        self.maybe_transition();
        match self.state {
            State::Closed => {
                self.failure_count += 1;
                if self.failure_count >= self.config.failure_threshold {
                    self.transition_to_open();
                }
            }
            State::HalfOpen => {
                // any failure in half-open re-opens the circuit
                self.transition_to_open();
            }
            State::Open => {}
        }
    }

    pub fn state(&self) -> State {
        self.state
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
        let cb = Breaker::new(Config::default());
        assert_eq!(cb.state(), State::Closed);
    }

    #[test]
    fn opens_after_threshold() {
        let mut cb = Breaker::new(Config {
            failure_threshold: 3,
            success_threshold: 1,
            open_duration: Duration::from_secs(60),
        });

        assert!(cb.allow());
        cb.on_failure();
        assert!(cb.allow());
        cb.on_failure();
        assert!(cb.allow());
        cb.on_failure();

        assert_eq!(cb.state(), State::Open);
        assert!(!cb.allow());
    }

    #[test]
    fn success_resets_failure_count() {
        let mut cb = Breaker::new(Config {
            failure_threshold: 3,
            success_threshold: 1,
            open_duration: Duration::from_secs(60),
        });

        cb.on_failure();
        cb.on_failure();
        cb.on_success(); // resets
        cb.on_failure();
        cb.on_failure();
        assert_eq!(cb.state(), State::Closed);
        cb.on_failure();
        assert_eq!(cb.state(), State::Open);
    }

    #[test]
    fn half_open_then_close() {
        let mut cb = Breaker::new(Config {
            failure_threshold: 2,
            success_threshold: 2,
            open_duration: Duration::from_millis(50),
        });

        cb.on_failure();
        cb.on_failure();
        assert_eq!(cb.state(), State::Open);

        thread::sleep(Duration::from_millis(60));
        assert!(cb.allow()); // moves to HalfOpen
        assert_eq!(cb.state(), State::HalfOpen);

        cb.on_success();
        assert_eq!(cb.state(), State::HalfOpen);
        cb.on_success();
        assert_eq!(cb.state(), State::Closed);
    }

    #[test]
    fn half_open_failure_reopens() {
        let mut cb = Breaker::new(Config {
            failure_threshold: 1,
            success_threshold: 2,
            open_duration: Duration::from_millis(30),
        });

        cb.on_failure();
        assert_eq!(cb.state(), State::Open);

        thread::sleep(Duration::from_millis(40));
        assert!(cb.allow());
        assert_eq!(cb.state(), State::HalfOpen);

        cb.on_failure();
        assert_eq!(cb.state(), State::Open);
        assert!(!cb.allow());
    }

    #[test]
    fn reset_works() {
        let mut cb = Breaker::new(Config {
            failure_threshold: 1,
            success_threshold: 1,
            open_duration: Duration::from_secs(60),
        });
        cb.on_failure();
        assert_eq!(cb.state(), State::Open);
        cb.reset();
        assert_eq!(cb.state(), State::Closed);
        assert!(cb.allow());
    }
}
