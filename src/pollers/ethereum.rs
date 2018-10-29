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
    number_of_tracked_confirmations: usize,
}

impl EthereumPollerService {
    pub fn new(
        interval: Duration,
        client: Arc<EthereumClient>,
        publisher: Arc<TransactionPublisher>,
        number_of_tracked_confirmations: usize,
    ) -> Self {
        Self {
            interval,
            client,
            publisher,
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

    pub fn publish_transactions(
        &self,
        start_block_hash: Option<String>,
        blocks_count: u64,
    ) -> impl Future<Item = (), Error = Error> + Send {
        let publisher = self.publisher.clone();
        self.client
            .last_eth_transactions(start_block_hash, blocks_count)
            .map_err(ectx!(ErrorSource::Client, ErrorKind::Internal))
            .and_then(move |tx| {
                publisher
                    .publish(vec![tx])
                    .map_err(ectx!(ErrorSource::Publisher, ErrorKind::Internal))
            }).for_each(|_| Ok(()))
    }

    fn tick(&self) {
        let f = self.publish_transactions(None, self.number_of_tracked_confirmations as u64);
        tokio::spawn(f.map_err(|e| {
            log_error(&e);
        }));
    }
}
