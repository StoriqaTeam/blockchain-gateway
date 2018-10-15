use std::sync::Arc;

use super::error::*;
use client::BitcoinClient;
use models::*;
use prelude::*;

pub trait BitcoinService: Send + Sync + 'static {
    fn get_utxos(&self, address: BitcoinAddress) -> Box<Future<Item = Vec<Utxo>, Error = Error> + Send>;
    fn send_raw_tx(&self, tx: BitcoinTransaction) -> Box<Future<Item = TxHash, Error = Error> + Send>;
}

#[derive(Clone)]
pub struct BitcoinServiceImpl {
    client: Arc<BitcoinClient>,
}

impl BitcoinServiceImpl {
    pub fn new(client: Arc<BitcoinClient>) -> Self {
        Self { client }
    }
}

impl BitcoinService for BitcoinServiceImpl {
    fn get_utxos(&self, address: BitcoinAddress) -> Box<Future<Item = Vec<Utxo>, Error = Error> + Send> {
        let address_clone = address.clone();
        Box::new(self.client.get_utxos(address).map_err(ectx!(convert address_clone)))
    }

    fn send_raw_tx(&self, tx: BitcoinTransaction) -> Box<Future<Item = TxHash, Error = Error> + Send> {
        let tx_clone = tx.clone();
        Box::new(self.client.send_raw_tx(tx).map_err(ectx!(convert tx_clone)))
    }
}
