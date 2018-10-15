mod error;
mod responses;

use std::sync::Arc;

use hyper::{Body, Request};

use self::error::*;
use self::responses::UtxosResponse;
use super::HttpClient;
use models::*;
use prelude::*;
use serde_json;
use utils::read_body;

pub trait BitcoinClient {
    fn get_utxos(&self, address: BitcoinAddress) -> Box<Future<Item = Vec<Utxo>, Error = Error>>;
}

#[derive(Clone)]
struct BitcoinClientImpl {
    http_client: Arc<HttpClient>,
    blockcypher_token: String,
}

impl BitcoinClient for BitcoinClientImpl {
    fn get_utxos(&self, address: BitcoinAddress) -> Box<Future<Item = Vec<Utxo>, Error = Error>> {
        let address_clone = address.clone();
        let address_clone2 = address.clone();
        let http_client = self.http_client.clone();
        Box::new(
            Request::builder()
                .method("GET")
                .uri(format!("https://blockchain.info/unspent?active={}", address))
                .body(Body::empty())
                .map_err(ectx!(ErrorSource::Hyper, ErrorKind::Internal => address_clone2))
                .into_future()
                .and_then(move |request| http_client.request(request).map_err(ectx!(ErrorKind::Internal => address_clone)))
                .and_then(|resp| read_body(resp.into_body()).map_err(ectx!(ErrorKind::Internal => address)))
                .and_then(|bytes| {
                    let bytes_clone = bytes.clone();
                    String::from_utf8(bytes).map_err(ectx!(ErrorContext::UTF8, ErrorKind::Internal => bytes_clone))
                }).and_then(|string| {
                    serde_json::from_str::<UtxosResponse>(&string).map_err(ectx!(ErrorContext::Json, ErrorKind::Internal => string.clone()))
                }).map(|resp| resp.unspent_outputs.into_iter().map(From::from).collect()),
        )
    }
}
