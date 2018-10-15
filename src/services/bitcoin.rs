use std::sync::Arc;

use super::error::*;
use client::BitcoinClient;
use models::*;
use prelude::*;

pub trait BitcoinService: Send + Sync + 'static {
    fn get_utxos(&self, address: BitcoinAddress) -> Box<Future<Item = Vec<Utxo>, Error = Error> + Send>;
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
        Box::new(self.client.get_utxos(address).map_err(ectx!(ErrorKind::Internal)))
    }
}
