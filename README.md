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
Configurable failure threshold, cooldown, success threshold, and live stats.

Zero dependencies. Only `std`.

---

## Why

When a downstream service starts failing, you stop hammering it.  
The breaker counts consecutive failures. After the threshold it opens and rejects calls for a cooldown. Then it allows a few probes (half-open). Successes close it again; failures reopen it.

## States

| State     | Behaviour                                    |
|-----------|----------------------------------------------|
| Closed    | Calls go through. Failures are counted.      |
| Open      | Calls rejected immediately.                  |
| Half-Open | Limited probe calls allowed.                 |

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

    for _ in 0..20 {
        if !cb.allow() {
            println!("circuit open — skipping");
            continue;
        }

        match do_work() {
            Ok(_) => cb.on_success(),
            Err(_) => cb.on_failure(),
        }

        println!("state = {:?}, stats = {:?}", cb.state(), cb.stats());
    }
}

fn do_work() -> Result<(), ()> {
    // your call here
    Ok(())
}
```

## Config

```rust
pub struct Config {
    pub failure_threshold: u32,   // consecutive failures → Open
    pub success_threshold: u32,   // consecutive successes in Half-Open → Closed
    pub open_duration: Duration,  // how long to stay Open
}
```

## API

```rust
impl Breaker {
    pub fn new(config: Config) -> Self;
    pub fn allow(&mut self) -> bool;
    pub fn on_success(&mut self);
    pub fn on_failure(&mut self);
    pub fn state(&self) -> State;
    pub fn stats(&self) -> Stats;
    pub fn reset(&mut self);
}
```

`Stats` contains total calls, successes, failures and how many times the circuit opened.

## Run tests

```bash
cargo test
```

## License

MIT
