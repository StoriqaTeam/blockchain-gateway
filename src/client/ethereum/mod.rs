mod responses;

use std::sync::Arc;

use hyper::{Body, Request};

use self::responses::*;
use super::error::*;
use super::http_client::HttpClient;
use config::Mode;
use models::*;
use prelude::*;
use serde_json;
use utils::read_body;

pub trait EthereumClient: Send + Sync + 'static {
    fn get_nonce(&self, address: EthereumAddress) -> Box<Future<Item = Vec<Utxo>, Error = Error> + Send>;
    fn send_raw_tx(&self, tx: EthereumTransaction) -> Box<Future<Item = TxHash, Error = Error> + Send>;
}

#[derive(Clone)]
pub struct EthereumClientImpl {
    http_client: Arc<HttpClient>,
    infura_url: String,
}

impl EthereumClientImpl {
    pub fn new(http_client: Arc<HttpClient>, blockcypher_token: String, mode: Mode, api_key: String) -> Self {
        let infura_url = match mode {
            Mode::Production => format!("https://mainnet.infura.io/{}", api_key),
            _ => format!("https://kovan.infura.io/{}", api_key),
        };
        Self { http_client, infura_url }
    }
}

impl EthereumClient for EthereumClientImpl {
    fn get_nonce(&self, address: EthereumAddress) -> Box<Future<Item = Vec<Utxo>, Error = Error> + Send> {
        let address_clone = address.clone();
        let address_clone2 = address.clone();
        let http_client = self.http_client.clone();
        let address_raw = format!("{}", address);
        let request = json!({
            "method": "eth_getTransactionCount",
            "params": [address, "latest"]
        })
        Box::new(
            Request::builder()
                .method("GET")
                .uri(self.infura_url)
                .body(Body::empty())
                .map_err(ectx!(ErrorSource::Hyper, ErrorKind::Internal => address_clone2))
                .into_future()
                .and_then(move |request| http_client.request(request))
                .and_then(|resp| read_body(resp.into_body()).map_err(ectx!(ErrorKind::Internal => address)))
                .and_then(|bytes| {
                    let bytes_clone = bytes.clone();
                    String::from_utf8(bytes).map_err(ectx!(ErrorContext::UTF8, ErrorKind::Internal => bytes_clone))
                }).and_then(|string| {
                    serde_json::from_str::<nonceResponse>(&string).map_err(ectx!(ErrorContext::Json, ErrorKind::Internal => string.clone()))
                }).map(|resp| resp.unspent_outputs.into_iter().map(From::from).collect()),
        )
    }

    fn send_raw_tx(&self, tx: EthereumTransaction) -> Box<Future<Item = TxHash, Error = Error> + Send> {
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
