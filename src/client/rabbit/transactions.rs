use std::io::Error as StdIoError;

use super::error::*;
use super::r2d2::RabbitPool;
use futures::future;
use lapin_futures::channel::Channel;
use models::*;
use prelude::*;
use serde_json;
use tokio::net::tcp::TcpStream;

pub trait TransactionPublisher {
    fn publish(&self, txs: Vec<BlockchainTransaction>) -> Box<Future<Item = (), Error = Error>>;
}

#[derive(Clone)]
pub struct TransactionPublisherImpl {
    pool: RabbitPool,
}

impl TransactionPublisherImpl {
    pub fn new(pool: RabbitPool) -> Self {
        Self { pool }
    }
}

impl TransactionPublisher for TransactionPublisherImpl {
    fn publish(&self, txs: Vec<BlockchainTransaction>) -> Box<Future<Item = (), Error = Error>> {
        let self_clone = self.clone();
        Box::new(
            self.pool
                .get()
                .map_err(ectx!(ErrorKind::Internal))
                .into_future()
                .and_then(move |channel| {
                    self_clone
                        .declare(&channel)
                        .map_err(ectx!(ErrorSource::Lapin, ErrorKind::Internal))
                        .map(move |_| channel)
                }).and_then(move |channel| {
                    let futures = txs.into_iter().map(move |tx| {
                        let routing_key = format!("{}", tx.currency);
                        let payload = serde_json::to_string(&tx).unwrap().into_bytes();
                        channel.clone().basic_publish(
                            "blockchain_transactions",
                            &routing_key,
                            payload,
                            Default::default(),
                            Default::default(),
                        )
                    });
                    future::join_all(futures).map_err(ectx!(ErrorSource::Lapin, ErrorKind::Internal))
                }).map(|_| ()),
        )
    }
}

impl TransactionPublisherImpl {
    fn declare(&self, channel: &Channel<TcpStream>) -> impl Future<Item = (), Error = StdIoError> {
        let f1: Box<Future<Item = (), Error = StdIoError>> =
            Box::new(channel.exchange_declare("blockchain_transactions", "direct", Default::default(), Default::default()));
        let f2: Box<Future<Item = (), Error = StdIoError>> = Box::new(
            channel
                .queue_declare("btc_transactions", Default::default(), Default::default())
                .map(|_| ()),
        );
        let f3: Box<Future<Item = (), Error = StdIoError>> = Box::new(
            channel
                .queue_declare("stq_transactions", Default::default(), Default::default())
                .map(|_| ()),
        );
        let f4: Box<Future<Item = (), Error = StdIoError>> = Box::new(
            channel
                .queue_declare("eth_transactions", Default::default(), Default::default())
                .map(|_| ()),
        );
        let f5: Box<Future<Item = (), Error = StdIoError>> = Box::new(channel.queue_bind(
            "btc_transactions",
            "blockchain_transactions",
            "btc",
            Default::default(),
            Default::default(),
        ));
        let f6 = Box::new(channel.queue_bind(
            "eth_transactions",
            "blockchain_transactions",
            "eth",
            Default::default(),
            Default::default(),
        ));
        let f7 = Box::new(channel.queue_bind(
            "stq_transactions",
            "blockchain_transactions",
            "stq",
            Default::default(),
            Default::default(),
        ));
        future::join_all(vec![f1, f2, f3, f4, f5, f6, f7]).map(|_| ())
    }
}
