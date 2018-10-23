mod responses;

use std::sync::Arc;

use hyper::{Body, Request};

use self::responses::*;
use super::error::*;
use super::http_client::HttpClient;
use config::Mode;
use futures::{future, stream};
use models::*;
use prelude::*;
use serde_json;
use utils::read_body;

pub trait BitcoinClient: Send + Sync + 'static {
    fn get_utxos(&self, address: BitcoinAddress) -> Box<Future<Item = Vec<Utxo>, Error = Error> + Send>;
    fn send_raw_tx(&self, tx: RawBitcoinTransaction) -> Box<Future<Item = TxHash, Error = Error> + Send>;
    fn get_transactions(&self, from_block: u64, to_block: u64) -> Box<Future<Item = Vec<BlockchainTransaction>, Error = Error> + Send>;
}

#[derive(Clone)]
pub struct BitcoinClientImpl {
    http_client: Arc<HttpClient>,
    mode: Mode,
    blockcypher_token: String,
}

const BLOCK_TXS_LIMIT: u64 = 50;

impl BitcoinClientImpl {
    pub fn new(http_client: Arc<HttpClient>, blockcypher_token: String, mode: Mode) -> Self {
        Self {
            http_client,
            blockcypher_token,
            mode,
        }
    }

    fn get_transactions_by_hashes(
        &self,
        hash_stream: Box<Stream<Item = String, Error = Error> + Send>,
    ) -> Box<Stream<Item = BlockchainTransaction, Error = Error> + Send> {
        let self_clone = self.clone();
        Box::new(
            hash_stream
                .chunks(BLOCK_TXS_LIMIT as usize)
                .and_then(move |hashes| {
                    let self_clone = self_clone.clone();
                    let fs = hashes.into_iter().map(move |hash| self_clone.get_transaction_by_hash(hash));
                    future::join_all(fs)
                }).map(|x| stream::iter_ok(x))
                .flatten(),
        )
    }

    fn get_transaction_by_hash(&self, hash: String) -> Box<Future<Item = BlockchainTransaction, Error = Error> + Send> {
        let hash_clone = hash.clone();
        let hash_clone2 = hash.clone();
        let http_client = self.http_client.clone();
        let uri_base = match self.mode {
            Mode::Production => "https://blockchain.info",
            _ => "https://testnet.blockchain.info",
        };
        Box::new(
            Request::builder()
                .method("GET")
                .uri(format!("{}/rawtx/{}", uri_base, hash))
                .body(Body::empty())
                .map_err(ectx!(ErrorSource::Hyper, ErrorKind::Internal => hash_clone))
                .into_future()
                .and_then(move |request| http_client.request(request))
                .and_then(|resp| read_body(resp.into_body()).map_err(ectx!(ErrorKind::Internal => hash_clone2)))
                .and_then(|bytes| {
                    let bytes_clone = bytes.clone();
                    String::from_utf8(bytes).map_err(ectx!(ErrorContext::UTF8, ErrorKind::Internal => bytes_clone))
                }).and_then(|string| {
                    serde_json::from_str::<GetTransactionResponse>(&string)
                        .map_err(ectx!(ErrorContext::Json, ErrorKind::Internal => string.clone()))
                }).and_then(BitcoinClientImpl::btc_response_to_tx),
        )
    }

    fn btc_response_to_tx(resp: GetTransactionResponse) -> Result<BlockchainTransaction, Error> {
        let resp_clone = resp.clone();
        let GetTransactionResponse {
            hash,
            inputs,
            out,
            block_height,
        } = resp;
        let from: Vec<_> = inputs
            .into_iter()
            .map(|entry_resp| BlockchainTransactionEntry {
                address: entry_resp.prev_out.addr,
                value: entry_resp.prev_out.value,
            }).collect();
        let to: Vec<_> = out
            .into_iter()
            .map(|entry_resp| BlockchainTransactionEntry {
                address: entry_resp.addr,
                value: entry_resp.value,
            }).collect();

        let from_sum = from.iter().fold(Some(Amount::new(0)), |acc, item| {
            acc.and_then(|acc_val| acc_val.checked_add(item.value))
        });
        let to_sum = to.iter().fold(Some(Amount::new(0)), |acc, item| {
            acc.and_then(|acc_val| acc_val.checked_add(item.value))
        });
        let fee = match (from_sum, to_sum) {
            (Some(fs), Some(ts)) => ts.checked_sub(fs),
            _ => None,
        }.ok_or(ectx!(try err ErrorContext::Overflow, ErrorKind::Internal => resp_clone))?;

        Ok(BlockchainTransaction {
            hash,
            from,
            to,
            block_number: block_height,
            currency: Currency::Btc,
            fee,
            confirmations: 0,
        })
    }

    fn get_transactions_by_block_offset_and_limit(
        &self,
        block: u64,
        offset: u64,
        limit: u64,
    ) -> Box<Stream<Item = BlockchainTransaction, Error = Error> + Send> {
        unimplemented!()
    }

    fn get_transaction_hashes_by_block(&self, block: u64) -> Box<Future<Item = Vec<String>, Error = Error> + Send> {
        Box::new(
            
        )
        unimplemented!()
    }
}

impl BitcoinClient for BitcoinClientImpl {
    fn get_transactions(&self, from_block: u64, to_block: u64) -> Box<Future<Item = Vec<BlockchainTransaction>, Error = Error> + Send> {
        (from_block..=to_block).iter().map(|block| )
        unimplemented!()
    }

    fn get_utxos(&self, address: BitcoinAddress) -> Box<Future<Item = Vec<Utxo>, Error = Error> + Send> {
        let address_clone = address.clone();
        let address_clone2 = address.clone();
        let http_client = self.http_client.clone();
        let uri_base = match self.mode {
            Mode::Production => "https://blockchain.info",
            _ => "https://testnet.blockchain.info",
        };
        Box::new(
            Request::builder()
                .method("GET")
                .uri(format!("{}/unspent?active={}", uri_base, address))
                .body(Body::empty())
                .map_err(ectx!(ErrorSource::Hyper, ErrorKind::Internal => address_clone2))
                .into_future()
                .and_then(move |request| http_client.request(request))
                .and_then(|resp| read_body(resp.into_body()).map_err(ectx!(ErrorKind::Internal => address)))
                .and_then(|bytes| {
                    let bytes_clone = bytes.clone();
                    String::from_utf8(bytes).map_err(ectx!(ErrorContext::UTF8, ErrorKind::Internal => bytes_clone))
                }).and_then(|string| {
                    serde_json::from_str::<UtxosResponse>(&string).map_err(ectx!(ErrorContext::Json, ErrorKind::Internal => string.clone()))
                }).map(|resp| resp.unspent_outputs.into_iter().map(From::from).collect()),
        )
    }

    fn send_raw_tx(&self, tx: RawBitcoinTransaction) -> Box<Future<Item = TxHash, Error = Error> + Send> {
        let tx_clone = tx.clone();
        let tx_clone2 = tx.clone();
        let http_client = self.http_client.clone();
        let uri_net_name = match self.mode {
            Mode::Production => "main",
            _ => "test3",
        };
        let body = Body::from(format!(r#"{{"tx": {}}}"#, tx));

        Box::new(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "https://api.blockcypher.com/v1/btc/{}/txs/push?token={}",
                    uri_net_name, self.blockcypher_token
                )).body(body)
                .map_err(ectx!(ErrorSource::Hyper, ErrorKind::Internal => tx_clone2))
                .into_future()
                .and_then(move |request| http_client.request(request))
                .and_then(|resp| read_body(resp.into_body()).map_err(ectx!(ErrorKind::Internal => tx)))
                .and_then(|bytes| {
                    let bytes_clone = bytes.clone();
                    String::from_utf8(bytes).map_err(ectx!(ErrorContext::UTF8, ErrorKind::Internal => bytes_clone))
                }).and_then(|string| {
                    serde_json::from_str::<PostTransactionsResponse>(&string)
                        .map_err(ectx!(ErrorContext::Json, ErrorKind::Internal => string.clone()))
                }).map(|resp| resp.hash),
        )
    }
}
