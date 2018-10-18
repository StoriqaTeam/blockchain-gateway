use std::sync::Arc;
use std::time::Duration;

use client::EthereumClient;
use client::TransactionPublisher;
use prelude::*;
use tokio;
use tokio::timer::Interval;

#[derive(Clone)]
pub struct EthereumPollerService {
    interval: Duration,
    client: Arc<EthereumClient>,
    publisher: Arc<TransactionPublisher>,
}

impl EthereumPollerService {
    pub fn new(interval: Duration, client: Arc<EthereumClient>, publisher: Arc<TransactionPublisher>) -> Self {
        Self {
            interval,
            client,
            publisher,
        }
    }

    pub fn start(&self) {
        let self_clone = self.clone();
        let interval = Interval::new_interval(self.interval).for_each(move |_| {
            self_clone.tick();
            Ok(())
        });
        tokio::spawn(interval.map_err(|_| ()));
    }

    fn tick(&self) {
        println!("Tick");
    }
}
