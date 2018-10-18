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
    current_block: Option<u128>,
}

impl EthereumPollerService {
    pub fn new(interval: Duration, client: Arc<EthereumClient>, publisher: Arc<TransactionPublisher>) -> Self {
        Self {
            interval,
            client,
            publisher,
            current_block: None,
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
        let mut self_clone = self.clone();
        // self.client.get_current_block().and_then(move |current_block| {
        //     let current_block = self_clone.current_block.unwrap_or(current_block);

        //     self_clone.current_block = Some(current_block);

        // })
        println!("Tick");
    }
}
