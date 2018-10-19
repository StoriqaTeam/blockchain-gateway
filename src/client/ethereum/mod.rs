mod responses;

use std::sync::Arc;

use hyper::{Body, Request};

use self::responses::*;
use super::error::*;
use super::http_client::HttpClient;
use config::Mode;
use futures::future;
use models::*;
use prelude::*;
use serde_json;
use utils::read_body;

pub trait EthereumClient: Send + Sync + 'static {
    fn get_nonce(&self, address: EthereumAddress) -> Box<Future<Item = u64, Error = Error> + Send>;
    fn get_current_block(&self) -> Box<Future<Item = u64, Error = Error> + Send>;
    fn get_eth_transactions(&self, from_block: u64, to_block: u64) -> Box<Future<Item = Vec<BlockchainTransaction>, Error = Error> + Send>;
    fn get_stq_transactions(&self, from_block: u64, to_block: u64) -> Box<Future<Item = Vec<BlockchainTransaction>, Error = Error> + Send>;
    fn send_raw_tx(&self, tx: RawEthereumTransaction) -> Box<Future<Item = TxHash, Error = Error> + Send>;
}

const ADDRESS_LENGTH: usize = 40;

#[derive(Clone)]
pub struct EthereumClientImpl {
    http_client: Arc<HttpClient>,
    infura_url: String,
    stq_contract_address: String,
    stq_transfer_topic: String,
}

impl EthereumClientImpl {
    pub fn new(
        http_client: Arc<HttpClient>,
        mode: Mode,
        api_key: String,
        stq_contract_address: String,
        stq_transfer_topic: String,
    ) -> Self {
        let infura_url = match mode {
            Mode::Production => format!("https://mainnet.infura.io/{}", api_key),
            _ => format!("https://kovan.infura.io/{}", api_key),
        };
        Self {
            http_client,
            infura_url,
            stq_contract_address,
            stq_transfer_topic,
        }
    }
}

impl EthereumClientImpl {
    fn get_eth_transactions_for_block(&self, block: u64) -> Box<Future<Item = Vec<BlockchainTransaction>, Error = Error> + Send> {
        let http_client = self.http_client.clone();
        let block = format!("0x{:x}", block);
        let request = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "eth_getBlockByNumber",
            "params": [block, true]
        }).to_string();
        Box::new(
            Request::builder()
                .header("Content-Type", "application/json")
                .method("POST")
                .uri(self.infura_url.clone())
                .body(Body::from(request))
                .map_err(ectx!(ErrorSource::Hyper, ErrorKind::Internal))
                .into_future()
                .and_then(move |request| http_client.request(request))
                .and_then(|resp| read_body(resp.into_body()).map_err(ectx!(ErrorKind::Internal)))
                .and_then(|bytes| {
                    let bytes_clone = bytes.clone();
                    String::from_utf8(bytes).map_err(ectx!(ErrorContext::UTF8, ErrorKind::Internal => bytes_clone))
                }).and_then(|string| {
                    serde_json::from_str::<BlockByNumberResponse>(&string)
                        .map_err(ectx!(ErrorContext::Json, ErrorKind::Internal => string.clone()))
                }).and_then(|resp| {
                    let txs: Result<Vec<BlockchainTransaction>, Error> = resp
                        .result
                        .transactions
                        .iter()
                        .map(|tx_resp| EthereumClientImpl::eth_response_to_tx(tx_resp.clone()))
                        .collect();
                    txs
                }),
        )
    }

    fn eth_response_to_tx(resp: TransactionResponse) -> Result<BlockchainTransaction, Error> {
        let TransactionResponse {
            block_number,
            hash,
            from,
            to,
            value,
            gas,
            gas_price,
        } = resp;
        let block_number = EthereumClientImpl::parse_hex(block_number)? as u64;
        let value = Amount::new(EthereumClientImpl::parse_hex(value)?);
        let gas = EthereumClientImpl::parse_hex(gas)?;
        let gas_price = EthereumClientImpl::parse_hex(gas_price)?;
        let fee = Amount::new(
            gas.checked_mul(gas_price)
                .ok_or(ectx!(try err ErrorContext::Overflow, ErrorKind::Internal))?,
        );
        Ok(BlockchainTransaction {
            hash,
            from,
            to,
            block_number,
            currency: Currency::Eth,
            value,
            fee,
            confirmations: 0,
        })
    }

    fn stq_response_to_tx(log: StqResponseItem) -> Result<BlockchainTransaction, Error> {
        let from = log
            .topics
            .get(1)
            .map(|s| {
                let slice = &s[(s.len() - ADDRESS_LENGTH)..];
                slice.to_string()
            }).ok_or(ectx!(try err ErrorContext::Topics, ErrorKind::Internal))?;
        let to = log
            .topics
            .get(2)
            // remove 0x and leading zeroes
            .map(|s| {
                let slice = &s[(s.len() - ADDRESS_LENGTH)..];
                slice.to_string()
            })
            .ok_or(ectx!(try err ErrorContext::Topics, ErrorKind::Internal))?;
        let block_number = EthereumClientImpl::parse_hex(log.block_number).map(|x| x as u64)?;
        let value = EthereumClientImpl::parse_hex(log.data).map(Amount::new)?;
        Ok(BlockchainTransaction {
            from,
            to,
            block_number,
            currency: Currency::Stq,
            value,
            fee: Amount::new(0),
            confirmations: 0,
            hash: log.transaction_hash[2..].to_string(),
        })
    }

    fn parse_hex(s: String) -> Result<u128, Error> {
        u128::from_str_radix(&s[2..], 16).map_err(ectx!(ErrorContext::Hex, ErrorKind::Internal => s))
    }
}

impl EthereumClient for EthereumClientImpl {
    fn get_stq_transactions(&self, from_block: u64, to_block: u64) -> Box<Future<Item = Vec<BlockchainTransaction>, Error = Error> + Send> {
        let http_client = self.http_client.clone();
        let from_block = format!("0x{:x}", from_block);
        let to_block = format!("0x{:x}", to_block);
        let request = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "eth_getLogs",
            "params": [{
                "address": self.stq_contract_address,
                "topics": [self.stq_transfer_topic],
                "fromBlock": from_block,
                "toBlock": to_block
            }]
        }).to_string();
        Box::new(
            Request::builder()
                .header("Content-Type", "application/json")
                .method("POST")
                .uri(self.infura_url.clone())
                .body(Body::from(request))
                .map_err(ectx!(ErrorSource::Hyper, ErrorKind::Internal))
                .into_future()
                .and_then(move |request| http_client.request(request))
                .and_then(|resp| read_body(resp.into_body()).map_err(ectx!(ErrorKind::Internal)))
                .and_then(|bytes| {
                    let bytes_clone = bytes.clone();
                    String::from_utf8(bytes).map_err(ectx!(ErrorContext::UTF8, ErrorKind::Internal => bytes_clone))
                }).and_then(|string| {
                    serde_json::from_str::<StqResponse>(&string).map_err(ectx!(ErrorContext::Json, ErrorKind::Internal => string.clone()))
                }).and_then(|resp| {
                    let res: Result<Vec<BlockchainTransaction>, Error> =
                        resp.result.into_iter().map(EthereumClientImpl::stq_response_to_tx).collect();
                    res
                }),
        )
    }

    fn get_eth_transactions(&self, from_block: u64, to_block: u64) -> Box<Future<Item = Vec<BlockchainTransaction>, Error = Error> + Send> {
        let self_clone = self.clone();
        let fs = (from_block..=to_block).map(move |block| self_clone.get_eth_transactions_for_block(block));
        Box::new(future::join_all(fs).map(|nested_txs| nested_txs.iter().flat_map(|x| x.iter()).cloned().collect()))
    }

    fn get_current_block(&self) -> Box<Future<Item = u64, Error = Error> + Send> {
        let http_client = self.http_client.clone();
        let request = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "eth_blockNumber",
            "params": []
        }).to_string();
        Box::new(
            Request::builder()
                .header("Content-Type", "application/json")
                .method("POST")
                .uri(self.infura_url.clone())
                .body(Body::from(request))
                .map_err(ectx!(ErrorSource::Hyper, ErrorKind::Internal))
                .into_future()
                .and_then(move |request| http_client.request(request))
                .and_then(|resp| read_body(resp.into_body()).map_err(ectx!(ErrorKind::Internal)))
                .and_then(|bytes| {
                    let bytes_clone = bytes.clone();
                    String::from_utf8(bytes).map_err(ectx!(ErrorContext::UTF8, ErrorKind::Internal => bytes_clone))
                }).and_then(|string| {
                    serde_json::from_str::<NonceResponse>(&string).map_err(ectx!(ErrorContext::Json, ErrorKind::Internal => string.clone()))
                }).and_then(|resp| {
                    u64::from_str_radix(&resp.result[2..], 16).map_err(ectx!(ErrorContext::Hex, ErrorKind::Internal => resp.result))
                }),
        )
    }
    fn get_nonce(&self, address: EthereumAddress) -> Box<Future<Item = u64, Error = Error> + Send> {
        let address_clone = address.clone();
        let address_clone2 = address.clone();
        let http_client = self.http_client.clone();
        let address_str = format!("0x{}", address);
        let request = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "eth_getTransactionCount",
            "params": [address_str, "latest"]
        }).to_string();
        Box::new(
            Request::builder()
                .header("Content-Type", "application/json")
                .method("POST")
                .uri(self.infura_url.clone())
                .body(Body::from(request))
                .map_err(ectx!(ErrorSource::Hyper, ErrorKind::Internal => address_clone2))
                .into_future()
                .and_then(move |request| http_client.request(request))
                .and_then(|resp| read_body(resp.into_body()).map_err(ectx!(ErrorKind::Internal => address)))
                .and_then(|bytes| {
                    let bytes_clone = bytes.clone();
                    String::from_utf8(bytes).map_err(ectx!(ErrorContext::UTF8, ErrorKind::Internal => bytes_clone))
                }).and_then(|string| {
                    serde_json::from_str::<NonceResponse>(&string).map_err(ectx!(ErrorContext::Json, ErrorKind::Internal => string.clone()))
                }).and_then(|resp| {
                    u64::from_str_radix(&resp.result[2..], 16).map_err(ectx!(ErrorContext::Hex, ErrorKind::Internal => resp.result))
                }),
        )
    }

    fn send_raw_tx(&self, tx: RawEthereumTransaction) -> Box<Future<Item = TxHash, Error = Error> + Send> {
        let tx_clone = tx.clone();
        let tx_clone2 = tx.clone();
        let http_client = self.http_client.clone();
        let tx_str = format!("0x{}", tx);
        let request = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "eth_sendRawTransaction",
            "params": [tx_str]
        }).to_string();
        Box::new(
            Request::builder()
                .method("POST")
                .uri(self.infura_url.clone())
                .body(Body::from(request))
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
                }).map(|resp| resp.result),
        )
    }
}
