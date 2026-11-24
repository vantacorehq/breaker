//! Demonstration of the circuit breaker with half-open limit and call() helper.

use breaker::{Breaker, Config};
use std::time::Duration;

fn main() {
    let mut cb = Breaker::new(
        Config::new(3, 2, Duration::from_secs(2)).with_half_open_max_calls(1),
    );

    println!("=== breaker demo ===\n");

    for i in 1..=18 {
        let result = cb.call(|| {
            // Fail the first few calls on purpose
            if i <= 4 {
                Err("downstream error")
            } else {
                Ok(i * 10)
            }
        });

        match result {
            None => println!("{:02}  rejected (state = {:?})", i, cb.state()),
            Some(Ok(v)) => println!("{:02}  success  → {}  (state = {:?})", i, v, cb.state()),
            Some(Err(e)) => println!("{:02}  failure  → {}  (state = {:?})", i, e, cb.state()),
        }

        std::thread::sleep(Duration::from_millis(250));
    }

    println!("\nFinal stats: {:?}", cb.stats());
}
