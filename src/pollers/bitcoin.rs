use std::sync::Arc;
use std::time::Duration;

use super::error::*;
use client::BitcoinClient;
use prelude::*;
use rabbit::TransactionPublisher;
use tokio;
use tokio::timer::Interval;
use utils::log_error;

#[derive(Clone)]
pub struct BitcoinPollerService {
    interval: Duration,
    client: Arc<BitcoinClient>,
    publisher: Arc<TransactionPublisher>,
    number_of_tracked_confirmations: usize,
}

impl BitcoinPollerService {
    pub fn new(
        interval: Duration,
        client: Arc<BitcoinClient>,
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

    fn tick(&self) {
        let publisher = self.publisher.clone();
        let f = self
            .client
            .last_transactions(self.number_of_tracked_confirmations as u64)
            .map_err(ectx!(ErrorSource::Client, ErrorKind::Internal))
            .and_then(move |tx| {
                publisher
                    .publish(vec![tx])
                    .map_err(ectx!(ErrorSource::Publisher, ErrorKind::Internal))
            }).for_each(|_| Ok(()))
            .map_err(|e: Error| {
                log_error(&e);
            });
        tokio::spawn(f);
    }
}
