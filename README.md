<p align="center">
  <img src="banner.svg" alt="breaker" width="100%">
</p>

<p align="center">
  <img src="https://img.shields.io/badge/rust-stable-orange?style=flat-square&logo=rust" alt="rust">
  <img src="https://img.shields.io/badge/license-MIT-blue?style=flat-square" alt="license">
  <img src="https://img.shields.io/badge/deps-zero-brightgreen?style=flat-square" alt="deps">
  <img src="https://img.shields.io/badge/tests-passing-success?style=flat-square" alt="tests">
</p>

**Circuit breaker in pure Rust.**

Closed → Open → Half-Open.  
Configurable failure threshold, cooldown, success threshold, and **probe limit** in Half-Open.

Zero dependencies. Only `std`.

---

## Why

When a downstream service starts failing, you stop hammering it.  
The breaker counts consecutive failures. After the threshold it opens and rejects calls for a cooldown. Then it allows a limited number of probes (half-open). Successes close it again; failures reopen it.

## States

| State     | Behaviour                                              |
|-----------|--------------------------------------------------------|
| Closed    | Calls go through. Failures are counted.                |
| Open      | Calls rejected immediately.                            |
| Half-Open | Only up to `half_open_max_calls` probes are allowed.   |

## Quick start

```toml
[dependencies]
breaker = { git = "https://github.com/vantacorehq/breaker" }
```

### Manual control

```rust
use breaker::{Breaker, Config};
use std::time::Duration;

let mut cb = Breaker::new(Config {
    failure_threshold: 5,
    success_threshold: 2,
    open_duration: Duration::from_secs(30),
    half_open_max_calls: 1,   // only one probe at a time
});

if cb.allow() {
    match do_work() {
        Ok(v) => cb.on_success(),
        Err(_) => cb.on_failure(),
    }
}
```

### Convenient `call` helper

```rust
match cb.call(|| do_work()) {
    None => {
        // circuit open — rejected
    }
    Some(Ok(v)) => {
        // success
    }
    Some(Err(e)) => {
        // failure (already recorded)
    }
}
```

### Multi-threaded usage

`Breaker` itself requires `&mut self` (single-threaded by design).

For concurrent backends use the ready-made wrapper:

```rust
use breaker::SyncBreaker;

let cb = SyncBreaker::with_defaults();   // &self methods, Mutex inside

if cb.allow() {
    // ...
}

// or
let result = cb.call(|| do_work());
```

You can also wrap the plain `Breaker` yourself with `Mutex` / `RwLock`.

## Config

```rust
pub struct Config {
    pub failure_threshold: u32,      // consecutive failures → Open
    pub success_threshold: u32,      // consecutive successes in Half-Open → Closed
    pub open_duration: Duration,     // how long to stay Open
    pub half_open_max_calls: u32,    // max simultaneous probes in Half-Open (default 1)
}
```

## API

```rust
impl Breaker {
    pub fn new(config: Config) -> Self;
    pub fn allow(&mut self) -> bool;
    pub fn on_success(&mut self);
    pub fn on_failure(&mut self);
    pub fn call<T, E>(&mut self, f: impl FnOnce() -> Result<T, E>) -> Option<Result<T, E>>;
    pub fn state(&self) -> State;
    pub fn stats(&self) -> Stats;
    pub fn reset(&mut self);
}
```

`Stats` tracks total calls, successes, failures, times opened, and rejected calls.

## Run tests

```bash
cargo test
```

## License

MIT
