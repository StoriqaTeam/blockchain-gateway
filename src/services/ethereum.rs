use std::sync::Arc;

use super::error::*;
use client::EthereumClient;
use models::*;
use prelude::*;

pub trait EthereumService: Send + Sync + 'static {
    fn get_nonce(&self, address: EthereumAddress) -> Box<Future<Item = u64, Error = Error> + Send>;
    fn send_raw_tx(&self, tx: EthereumTransaction) -> Box<Future<Item = TxHash, Error = Error> + Send>;
}

#[derive(Clone)]
pub struct EthereumServiceImpl {
    client: Arc<EthereumClient>,
}

impl EthereumServiceImpl {
    pub fn new(client: Arc<EthereumClient>) -> Self {
        Self { client }
    }
}

impl EthereumService for EthereumServiceImpl {
    fn get_nonce(&self, address: EthereumAddress) -> Box<Future<Item = u64, Error = Error> + Send> {
        let address_clone = address.clone();
        Box::new(self.client.get_nonce(address).map_err(ectx!(convert address_clone)))
    }

    fn send_raw_tx(&self, tx: EthereumTransaction) -> Box<Future<Item = TxHash, Error = Error> + Send> {
        let tx_clone = tx.clone();
        Box::new(self.client.send_raw_tx(tx).map_err(ectx!(convert tx_clone)))
    }
}
