use std::sync::Arc;
use std::time::Duration;

use super::error::*;
use client::EthereumClient;
use prelude::*;
use rabbit::TransactionPublisher;
use tokio;
use tokio::timer::Interval;
use utils::log_error;

#[derive(Clone)]
pub struct EthereumPollerService {
    interval: Duration,
    client: Arc<EthereumClient>,
    publisher: Arc<TransactionPublisher>,
    current_block: Option<u64>,
    number_of_tracked_confirmations: usize,
}

impl EthereumPollerService {
    pub fn new(
        interval: Duration,
        client: Arc<EthereumClient>,
        publisher: Arc<TransactionPublisher>,
        number_of_tracked_confirmations: usize,
        start_block: Option<u64>,
    ) -> Self {
        Self {
            interval,
            client,
            publisher,
            current_block: start_block,
            number_of_tracked_confirmations,
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
        let self_clone = self.clone();
        let mut self_clone2 = self.clone();
        let client = self.client.clone();
        let publisher = self.publisher.clone();
        let number_of_tracked_confirmations = self.number_of_tracked_confirmations;
        let f = self
            .client
            .get_current_block()
            .map_err(ectx!(ErrorSource::Client, ErrorKind::Internal))
            .and_then(move |current_block| {
                let from_block = self_clone.current_block.unwrap_or(current_block) - (number_of_tracked_confirmations as u64);
                let to_block = current_block;
                client
                    .get_eth_transactions(from_block as u64, to_block as u64)
                    .map(move |txs| (txs, current_block))
                    .map_err(ectx!(ErrorSource::Client, ErrorKind::Internal))
            }).map(|(mut txs, current_block)| {
                for tx in txs.iter_mut() {
                    tx.confirmations = (current_block - tx.block_number) as usize;
                }
                (txs, current_block)
            }).and_then(move |(txs, current_block)| {
                publisher
                    .publish(txs)
                    .map(move |_| current_block)
                    .map_err(ectx!(ErrorSource::Publisher, ErrorKind::Internal))
            }).map(move |current_block| {
                self_clone2.current_block = Some(current_block);
            }).map_err(|e: Error| {
                log_error(&e);
            });
        tokio::spawn(f);
    }
}
