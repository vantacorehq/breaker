//! Simple demonstration of the circuit breaker.

use breaker::{Breaker, Config};
use std::time::Duration;

fn main() {
    let mut cb = Breaker::new(Config {
        failure_threshold: 3,
        success_threshold: 2,
        open_duration: Duration::from_secs(2),
    });

    println!("=== breaker demo ===\n");

    // Simulate a flaky dependency
    for i in 1..=15 {
        if !cb.allow() {
            println!("{:02}  rejected (state = {:?})", i, cb.state());
            std::thread::sleep(Duration::from_millis(300));
            continue;
        }

        // Fail the first few calls on purpose
        let result = if i <= 4 { Err(()) } else { Ok(()) };

        match result {
            Ok(()) => {
                cb.on_success();
                println!("{:02}  success   (state = {:?})", i, cb.state());
            }
            Err(()) => {
                cb.on_failure();
                println!("{:02}  failure   (state = {:?})", i, cb.state());
            }
        }

        std::thread::sleep(Duration::from_millis(200));
    }

    println!("\nFinal stats: {:?}", cb.stats());
}
