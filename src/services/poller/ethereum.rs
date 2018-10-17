use std::time::Duration;

use prelude::*;
use tokio;
use tokio::timer::Interval;

#[derive(Clone)]
pub struct EthereumPollerService {
    interval: Duration,
}

impl EthereumPollerService {
    pub fn new(interval: Duration) -> Self {
        Self { interval }
    }

    pub fn start(&self) {
        let self_clone = self.clone();
        let interval = Interval::new_interval(self.interval).for_each(move |_| {
            self_clone.tick();
            Ok(())
        });
        tokio::run(interval.map_err(|_| ()));
    }

    fn tick(&self) {
        println!("Tick");
    }
}
