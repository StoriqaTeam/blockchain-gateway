use std::io::Error as StdIoError;

use super::error::*;
use super::r2d2::RabbitConnectionManager;
use super::r2d2::RabbitPool;
use futures::future;
use futures_cpupool::CpuPool;
use lapin_futures::channel::{Channel, ExchangeDeclareOptions, QueueDeclareOptions};
use models::*;
use prelude::*;
use r2d2::PooledConnection;
use serde_json;
use tokio::net::tcp::TcpStream;

pub trait TransactionPublisher: Send + Sync + 'static {
    fn publish(&self, txs: Vec<BlockchainTransaction>) -> Box<Future<Item = (), Error = Error> + Send>;
}

#[derive(Clone)]
pub struct TransactionPublisherImpl {
    rabbit_pool: RabbitPool,
    thread_pool: CpuPool,
}

impl TransactionPublisherImpl {
    pub fn new(rabbit_pool: RabbitPool, thread_pool: CpuPool) -> Self {
        Self { rabbit_pool, thread_pool }
    }

    pub fn init(&self) -> impl Future<Item = (), Error = Error> {
        let self_clone = self.clone();
        self.get_channel().and_then(move |channel| self_clone.declare(&channel))
    }

    fn get_channel(&self) -> impl Future<Item = PooledConnection<RabbitConnectionManager>, Error = Error> {
        let rabbit_pool = self.rabbit_pool.clone();
        self.thread_pool
            .spawn_fn(move || rabbit_pool.get().map_err(ectx!(ErrorSource::Lapin, ErrorKind::Internal)))
    }

    fn declare(&self, channel: &Channel<TcpStream>) -> impl Future<Item = (), Error = Error> {
        let f1: Box<Future<Item = (), Error = StdIoError>> = Box::new(channel.exchange_declare(
            "blockchain_transactions",
            "direct",
            ExchangeDeclareOptions {
                durable: true,
                ..Default::default()
            },
            Default::default(),
        ));
        let f2: Box<Future<Item = (), Error = StdIoError>> = Box::new(
            channel
                .queue_declare(
                    "btc_transactions",
                    QueueDeclareOptions {
                        durable: true,
                        ..Default::default()
                    },
                    Default::default(),
                ).map(|_| ()),
        );
        let f3: Box<Future<Item = (), Error = StdIoError>> = Box::new(
            channel
                .queue_declare(
                    "stq_transactions",
                    QueueDeclareOptions {
                        durable: true,
                        ..Default::default()
                    },
                    Default::default(),
                ).map(|_| ()),
        );
        let f4: Box<Future<Item = (), Error = StdIoError>> = Box::new(
            channel
                .queue_declare(
                    "eth_transactions",
                    QueueDeclareOptions {
                        durable: true,
                        ..Default::default()
                    },
                    Default::default(),
                ).map(|_| ()),
        );
        let f5: Box<Future<Item = (), Error = StdIoError>> = Box::new(channel.queue_bind(
            "btc_transactions",
            "blockchain_transactions",
            "btc_transactions",
            Default::default(),
            Default::default(),
        ));
        let f6 = Box::new(channel.queue_bind(
            "eth_transactions",
            "blockchain_transactions",
            "eth_transactions",
            Default::default(),
            Default::default(),
        ));
        let f7 = Box::new(channel.queue_bind(
            "stq_transactions",
            "blockchain_transactions",
            "stq_transactions",
            Default::default(),
            Default::default(),
        ));
        let f8 = Box::new(
            channel
                .queue_declare("eth_current_block", Default::default(), Default::default())
                .map(|_| ()),
        );
        let f9 = Box::new(
            channel
                .queue_declare("btc_current_block", Default::default(), Default::default())
                .map(|_| ()),
        );
        let f10 = Box::new(channel.queue_bind(
            "eth_current_block",
            "blockchain_transactions",
            "eth_current_block",
            Default::default(),
            Default::default(),
        ));
        let f11 = Box::new(channel.queue_bind(
            "btc_current_block",
            "blockchain_transactions",
            "btc_current_block",
            Default::default(),
            Default::default(),
        ));
        future::join_all(vec![f1, f2, f3, f4, f5, f6, f7, f8, f9, f10, f11])
            .map(|_| ())
            .map_err(ectx!(ErrorSource::Lapin, ErrorKind::Internal))
    }
}

impl TransactionPublisher for TransactionPublisherImpl {
    fn publish(&self, txs: Vec<BlockchainTransaction>) -> Box<Future<Item = (), Error = Error> + Send> {
        let self_clone = self.clone();
        Box::new(
            self.get_channel()
                .and_then(move |channel| {
                    let futures = txs.into_iter().map(move |tx| {
                        let routing_key = format!("{}_transactions", tx.currency);
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

impl TransactionPublisherImpl {}
