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

pub trait EthereumClient: Send + Sync + 'static {
    /// Get account nonce (needed for creating transactions)
    fn get_nonce(&self, address: EthereumAddress) -> Box<Future<Item = u64, Error = Error> + Send>;
    /// Send raw eth/stq transaction to blockchain
    fn send_raw_tx(&self, tx: RawEthereumTransaction) -> Box<Future<Item = TxHash, Error = Error> + Send>;
    /// Get transaction by hash. Since getting block_number from transaction is not yet
    /// supported, you need to provide one in arguments
    fn get_eth_transaction(&self, hash: String) -> Box<Future<Item = BlockchainTransaction, Error = Error> + Send>;
    /// Get transactions from blocks starting from `start_block_hash` (or the most recent block if not specified)
    /// and fetch previous blocks. Total number of blocks = `prev_blocks_count`.
    /// `prev_blocks_count` should be greater than 0.
    fn last_eth_transactions(
        &self,
        start_block_hash: Option<String>,
        prev_blocks_count: u64,
    ) -> Box<Stream<Item = BlockchainTransaction, Error = Error> + Send>;
    /// Same as `get_eth_transaction` for stq. Since there could be many stq transfers in one transaction
    /// we return Stream here.
    fn get_stq_transactions(&self, hash: String) -> Box<Stream<Item = BlockchainTransaction, Error = Error> + Send>;
    /// Same as `last_eth_transactions` for stq
    fn last_stq_transactions(
        &self,
        start_block_hash: Option<String>,
        prev_blocks_count: u64,
    ) -> Box<Stream<Item = BlockchainTransaction, Error = Error> + Send>;
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
    fn get_rpc_response<T>(&self, params: &::serde_json::Value) -> impl Future<Item = T, Error = Error> + Send
    where
        for<'a> T: Send + 'static + ::serde::Deserialize<'a>,
    {
        let http_client = self.http_client.clone();
        let params_clone = params.clone();
        serde_json::to_string(params)
            .map_err(ectx!(ErrorContext::Json, ErrorKind::Internal => params))
            .and_then(|body| {
                Request::builder()
                .header("Content-Type", "application/json")
                .method("POST")
                .uri(self.infura_url.clone())
                .body(Body::from(body.clone()))
                .map_err(ectx!(ErrorSource::Hyper, ErrorKind::Internal => body))
            }).into_future()
            .and_then(move |request| http_client.request(request))
            .and_then(|resp| read_body(resp.into_body()).map_err(ectx!(ErrorKind::Internal => params_clone)))
            .and_then(|bytes| {
                let bytes_clone = bytes.clone();
                String::from_utf8(bytes).map_err(ectx!(ErrorContext::UTF8, ErrorKind::Internal => bytes_clone))
            }).and_then(|string| {
                serde_json::from_str::<T>(&string).map_err(ectx!(ErrorContext::Json, ErrorKind::Internal => string.clone()))
            })
    }

    fn get_current_block_number(&self) -> impl Future<Item = u64, Error = Error> + Send {
        let params = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "eth_blockNumber",
            "params": []
        });

        self.get_rpc_response::<NonceResponse>(&params).and_then(|resp| {
            u64::from_str_radix(&resp.result[2..], 16).map_err(ectx!(ErrorContext::Hex, ErrorKind::Internal => resp.result))
        })
    }

    fn get_block_number_by_hash(&self, hash: String) -> impl Future<Item = u64, Error = Error> + Send {
        let params = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "eth_getBlockByHash",
            "params": [hash, false]
        });

        self.get_rpc_response::<ShortBlockResponse>(&params).map(|resp| resp.result.number)
    }

    /// Gets NON-ZERO value eth transactions (that means that ERC-20 are not here)
    fn get_eth_transactions_for_block(&self, block: u64) -> impl Stream<Item = BlockchainTransaction, Error = Error> + Send {
        let block = format!("0x{:x}", block);
        let params = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "eth_getBlockByNumber",
            "params": [block, true]
        });
        self.get_rpc_response::<BlockByNumberResponse>(&params)
            .into_stream()
            .map(|resp| {
                stream::iter_result(
                    resp.result
                        .unwrap_or(Default::default())
                        .transactions
                        .into_iter()
                        .map(|tx_resp| EthereumClientImpl::eth_response_to_tx(tx_resp.clone())),
                )
            }).flatten()
            .filter(|tx| tx.to[0].value.inner() > 0)
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
        let from = vec![(&from[2..]).to_string()];
        let to_address = to.map(|t| (&t[2..]).to_string()).unwrap_or("0".to_string());
        let to = vec![BlockchainTransactionEntry {
            address: to_address,
            value,
        }];
        Ok(BlockchainTransaction {
            hash: (&hash[2..]).to_string(),
            from,
            to,
            block_number,
            currency: Currency::Eth,
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
        let log_index = EthereumClientImpl::parse_hex(log.log_index)?;
        // Since there can be many ERC-20 transfers per ETH transaction, we're giving extended hash here
        let hash = format!("{}:{}", log.transaction_hash[2..].to_string(), log_index);
        let from = vec![from];
        let to = vec![BlockchainTransactionEntry { address: to, value }];
        Ok(BlockchainTransaction {
            from,
            to,
            block_number,
            currency: Currency::Stq,
            fee: Amount::new(0),
            confirmations: 0,
            hash,
        })
    }

    fn parse_hex(s: String) -> Result<u128, Error> {
        u128::from_str_radix(&s[2..], 16).map_err(ectx!(ErrorContext::Hex, ErrorKind::Internal => s))
    }
}

impl EthereumClient for EthereumClientImpl {
    fn last_stq_transactions(
        &self,
        start_block_hash: Option<String>,
        prev_blocks_count: u64,
    ) -> Box<Stream<Item = BlockchainTransaction, Error = Error> + Send> {
        let self_clone = self.clone();
        let to_block_number_f = match start_block_hash {
            Some(hash) => future::Either::A(self.get_block_number_by_hash(hash)),
            None => future::Either::B(self.get_current_block_number()),
        };

        Box::new(
            to_block_number_f
                .and_then(move |to_block_number| {
                    let to_block = format!("0x{:x}", to_block_number);
                    let from_block = format!("0x{:x}", to_block_number - prev_blocks_count + 1);
                    let params = json!({
                        "jsonrpc": "2.0",
                        "id": 1,
                        "method": "eth_getLogs",
                        "params": [{
                            "address": self_clone.stq_contract_address,
                            "topics": [self_clone.stq_transfer_topic],
                            "fromBlock": from_block,
                            "toBlock": to_block,
                        }]
                    });
                    self_clone.get_rpc_response::<StqResponse>(&params)
                }).into_stream()
                .map(|resp| stream::iter_ok(resp.result.unwrap_or(vec![]).into_iter()))
                .flatten()
                .and_then(EthereumClientImpl::stq_response_to_tx),
        )
    }

    fn last_eth_transactions(
        &self,
        start_block_hash: Option<String>,
        prev_blocks_count: u64,
    ) -> Box<Stream<Item = BlockchainTransaction, Error = Error> + Send> {
        let self_clone = self.clone();
        let to_block_number_f = match start_block_hash {
            Some(hash) => future::Either::A(self.get_block_number_by_hash(hash)),
            None => future::Either::B(self.get_current_block_number()),
        };
        Box::new(
            to_block_number_f
                .into_stream()
                .map(move |to_block_number| stream::iter_ok::<_, Error>(to_block_number - prev_blocks_count + 1..=to_block_number))
                .flatten()
                .map(move |block_number| self_clone.get_eth_transactions_for_block(block_number))
                .flatten(),
        )
    }

    fn get_eth_transaction(&self, hash: String) -> Box<Future<Item = BlockchainTransaction, Error = Error> + Send> {
        let params = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "eth_getTransactionByHash",
            "params": [hash]
        });
        Box::new(
            self.get_rpc_response::<TransactionResponse>(&params)
                .and_then(|resp| EthereumClientImpl::eth_response_to_tx(resp)),
        )
    }

    fn get_stq_transactions(&self, hash: String) -> Box<Stream<Item = BlockchainTransaction, Error = Error> + Send> {
        let params = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "eth_getLogs",
            "params": [{
                "address": self.stq_contract_address,
                "topics": [self.stq_transfer_topic],
                "transactionHash": hash,
            }]
        });

        Box::new(
            self.get_rpc_response::<StqResponse>(&params)
                .into_stream()
                .map(|resp| stream::iter_ok(resp.result.unwrap_or(vec![]).into_iter()))
                .flatten()
                .and_then(EthereumClientImpl::stq_response_to_tx),
        )
    }

    fn get_nonce(&self, address: EthereumAddress) -> Box<Future<Item = u64, Error = Error> + Send> {
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
