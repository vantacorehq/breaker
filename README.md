# breaker

**Circuit breaker in pure Rust.**

Closed → Open → Half-Open.  
Configurable failure threshold, cooldown, and success count to close again.

Zero dependencies. Only `std`.

---

## Why

When a downstream service starts failing, you want to stop hammering it.  
A circuit breaker tracks consecutive failures and, after a threshold, short-circuits calls for a cooldown period. After the cooldown it lets a few probe requests through (half-open). If they succeed, the circuit closes; if they fail, it opens again.

## States

| State     | Behaviour                                      |
|-----------|------------------------------------------------|
| Closed    | Calls go through. Failures are counted.        |
| Open      | Calls are rejected immediately.                |
| Half-Open | A limited number of probe calls are allowed.   |

## Quick start

```toml
[dependencies]
breaker = { git = "https://github.com/vantacorehq/breaker" }
```

```rust
use breaker::{Breaker, Config};
use std::time::Duration;

fn main() {
    let mut cb = Breaker::new(Config {
        failure_threshold: 5,
        success_threshold: 2,
        open_duration: Duration::from_secs(30),
    });

    // Before every call
    if !cb.allow() {
        // circuit is open — fail fast
        return;
    }

    match do_work() {
        Ok(v) => {
            cb.on_success();
            // use v
        }
        Err(_) => {
            cb.on_failure();
        }
    }
}

fn do_work() -> Result<(), ()> {
    Ok(())
}
```

## Config

```rust
pub struct Config {
    /// Consecutive failures that trip the breaker into Open.
    pub failure_threshold: u32,
    /// Consecutive successes in Half-Open that close the circuit.
    pub success_threshold: u32,
    /// How long the circuit stays Open before moving to Half-Open.
    pub open_duration: Duration,
}
```

## API

```rust
impl Breaker {
    pub fn new(config: Config) -> Self;
    pub fn allow(&mut self) -> bool;   // may I execute the call?
    pub fn on_success(&mut self);
    pub fn on_failure(&mut self);
    pub fn state(&self) -> State;
    pub fn reset(&mut self);           // force Closed
}
```

## Tests

```bash
cargo test
```

## License

MIT
