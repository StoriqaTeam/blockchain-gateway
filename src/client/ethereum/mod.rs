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

    /// Get latest wei balance
    fn get_eth_balance(&self, address: EthereumAddress) -> Box<Future<Item = Amount, Error = Error> + Send>;

    /// Get transactions from blocks starting from `start_block_hash` (or the most recent block if not specified)
    /// and fetch previous blocks. Total number of blocks = `blocks_count`.
    /// `blocks_count` should be greater than 0.
    fn last_eth_transactions(
        &self,
        start_block_hash: Option<String>,
        blocks_count: u64,
    ) -> Box<Stream<Item = BlockchainTransaction, Error = Error> + Send>;

    /// Same as `get_eth_transaction` for stq. Since there could be many stq transfers in one transaction
    /// we return Stream here.
    fn get_stq_transactions(&self, hash: String) -> Box<Stream<Item = BlockchainTransaction, Error = Error> + Send>;

    /// Same as `last_eth_transactions` for stq
    fn last_stq_transactions(
        &self,
        start_block_hash: Option<String>,
        blocks_count: u64,
    ) -> Box<Stream<Item = BlockchainTransaction, Error = Error> + Send>;

    /// Get latest stq-wei balance
    fn get_stq_balance(&self, address: EthereumAddress) -> Box<Future<Item = Amount, Error = Error> + Send>;
}

const ADDRESS_LENGTH: usize = 40;

#[derive(Clone)]
pub struct EthereumClientImpl {
    http_client: Arc<HttpClient>,
    infura_url: String,
    stq_contract_address: String,
    stq_transfer_topic: String,
    stq_approval_topic: String,
    stq_balance_method: String,
}

impl EthereumClientImpl {
    pub fn new(
        http_client: Arc<HttpClient>,
        mode: Mode,
        api_key: String,
        stq_contract_address: String,
        stq_transfer_topic: String,
        stq_approval_topic: String,
        stq_balance_method: String,
    ) -> Self {
        let infura_url = match mode {
            Mode::Production => format!("https://mainnet.infura.io/v3/{}", api_key),
            _ => format!("https://kovan.infura.io/v3/{}", api_key),
        };
        Self {
            http_client,
            infura_url,
            stq_contract_address,
            stq_transfer_topic,
            stq_approval_topic,
            stq_balance_method,
        }
    }
}

impl EthereumClientImpl {
    // Eth

    /// Gets NON-ZERO value eth transactions (that means that ERC-20 are not here)
    fn get_eth_transactions_for_block(&self, block: u64) -> impl Stream<Item = PartialBlockchainTransaction, Error = Error> + Send {
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
                        .map(|tx_resp| EthereumClientImpl::eth_response_to_partial_tx(tx_resp.clone())),
                )
            }).flatten()
            .filter(|tx| tx.to[0].value.inner() > 0)
    }

    fn last_eth_transactions_with_current_block(
        &self,
        start_block_hash: Option<String>,
        blocks_count: u64,
        current_block: u64,
    ) -> impl Stream<Item = BlockchainTransaction, Error = Error> + Send {
        let to_block_number_f = match start_block_hash {
            Some(hash) => future::Either::A(self.get_block_number_by_hash(hash)),
            None => future::Either::B(Ok(current_block).into_future()),
        };
        let self_clone = self.clone();
        let self_clone2 = self.clone();
        to_block_number_f
            .into_stream()
            .map(move |to_block| stream::iter_ok::<_, Error>(to_block - blocks_count + 1..=to_block))
            .flatten()
            .map(move |block_number| self_clone.get_eth_transactions_for_block(block_number))
            .flatten()
            .and_then(move |tx| self_clone2.partial_tx_to_tx(&tx, current_block))
    }

    fn get_eth_partial_transaction(&self, hash: String) -> impl Future<Item = PartialBlockchainTransaction, Error = Error> + Send {
        let hash = format!("0x{}", hash);
        let params = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "eth_getTransactionByHash",
            "params": [hash]
        });
        self.get_rpc_response::<TransactionByHashResponse>(&params)
            .and_then(|resp| EthereumClientImpl::eth_response_to_partial_tx(resp.result))
    }

    fn eth_response_to_partial_tx(resp: TransactionResponse) -> Result<PartialBlockchainTransaction, Error> {
        let TransactionResponse {
            block_number,
            hash,
            from,
            to,
            value,
            gas_price,
        } = resp;
        let block_number = EthereumClientImpl::parse_hex(block_number)? as u64;
        let value = Amount::new(EthereumClientImpl::parse_hex(value)?);
        let gas_price = Amount::new(EthereumClientImpl::parse_hex(gas_price)?);
        let from = vec![(&from[2..]).to_string()];
        let to_address = to.map(|t| (&t[2..]).to_string()).unwrap_or("0".to_string());
        let to = vec![BlockchainTransactionEntry {
            address: to_address,
            value,
        }];
        Ok(PartialBlockchainTransaction {
            hash: (&hash[2..]).to_string(),
            from,
            to,
            block_number,
            currency: Currency::Eth,
            gas_price,
            erc20_operation_kind: None,
        })
    }

    // Stq

    fn get_stq_transactions_for_blocks(
        &self,
        from_block: u64,
        to_block: u64,
    ) -> impl Stream<Item = PartialBlockchainTransaction, Error = Error> + Send {
        let self_clone = self.clone();
        let self_clone2 = self.clone();
        let from_block = format!("0x{:x}", from_block);
        let to_block = format!("0x{:x}", to_block);
        let params_approval = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "eth_getLogs",
            "params": [{
                "address": self.stq_contract_address,
                "topics": [self.stq_approval_topic],
                "fromBlock": from_block,
                "toBlock": to_block,
            }]
        });
        let params_transfer = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "eth_getLogs",
            "params": [{
                "address": self.stq_contract_address,
                "topics": [self.stq_transfer_topic],
                "fromBlock": from_block,
                "toBlock": to_block,
            }]
        });
        self.get_rpc_response::<StqResponse>(&params_approval)
            .join(self.get_rpc_response::<StqResponse>(&params_transfer))
            .map(|(approval_resp, transfer_resp)| approval_resp.concat(transfer_resp))
            .into_stream()
            .map(|resp| stream::iter_ok(resp.result.into_iter()))
            .flatten()
            .and_then(move |tx_resp| {
                self_clone
                    .get_eth_partial_transaction(tx_resp.transaction_hash[2..].to_string())
                    .map(|tx| (tx_resp, tx.gas_price))
            }).and_then(move |(tx_resp, gas_price)| self_clone2.stq_response_to_partial_tx(tx_resp, gas_price))
    }

    fn last_stq_transactions_with_current_block(
        &self,
        start_block_hash: Option<String>,
        blocks_count: u64,
        current_block: u64,
    ) -> impl Stream<Item = BlockchainTransaction, Error = Error> + Send {
        let to_block_number_f = match start_block_hash {
            Some(hash) => future::Either::A(self.get_block_number_by_hash(hash)),
            None => future::Either::B(Ok(current_block).into_future()),
        };
        let self_clone = self.clone();
        let self_clone2 = self.clone();
        to_block_number_f
            .into_stream()
            .map(move |to_block| self_clone.get_stq_transactions_for_blocks(to_block - blocks_count + 1, to_block))
            .flatten()
            .and_then(move |tx| self_clone2.partial_tx_to_tx(&tx, current_block))
    }

    fn get_stq_transactions_with_current_block(
        &self,
        hash: String,
        current_block: u64,
    ) -> Box<Stream<Item = BlockchainTransaction, Error = Error> + Send> {
        let self_clone = self.clone();
        let self_clone2 = self.clone();
        let self_clone3 = self.clone();
        let self_clone4 = self.clone();
        let stq_contract_address = self.stq_contract_address.clone();
        let stq_transfer_topic = self.stq_transfer_topic.clone();
        let stq_approval_topic = self.stq_approval_topic.clone();
        let hash_clone = hash.clone();

        Box::new(
            self.get_eth_partial_transaction(hash)
                .and_then(move |partial_eth_tx| {
                    let params_transfer = json!({
                        "jsonrpc": "2.0",
                        "id": 1,
                        "method": "eth_getLogs",
                        "params": [{
                            "address": stq_contract_address,
                            "topics": [stq_transfer_topic],
                            "fromBlock": format!("0x{:x}", partial_eth_tx.block_number),
                            "toBlock": format!("0x{:x}", partial_eth_tx.block_number),
                        }]
                    });
                    let params_approval = json!({
                        "jsonrpc": "2.0",
                        "id": 1,
                        "method": "eth_getLogs",
                        "params": [{
                            "address": stq_contract_address,
                            "topics": [stq_approval_topic],
                            "fromBlock": format!("0x{:x}", partial_eth_tx.block_number),
                            "toBlock": format!("0x{:x}", partial_eth_tx.block_number),
                        }]
                    });
                    self_clone3
                        .get_rpc_response::<StqResponse>(&params_approval)
                        .join(self_clone3.get_rpc_response::<StqResponse>(&params_transfer))
                        .map(|(approval_resp, transfer_resp)| approval_resp.concat(transfer_resp))
                }).into_stream()
                .map(|stq_resp| stream::iter_ok(stq_resp.result.into_iter()))
                .flatten()
                .filter(move |resp_item| resp_item.transaction_hash[2..] == hash_clone[..])
                .and_then(move |tx_resp| {
                    self_clone
                        .get_eth_partial_transaction(tx_resp.transaction_hash[2..].to_string())
                        .map(|tx| (tx_resp, tx.gas_price))
                }).and_then(move |(resp, gas_price)| self_clone4.stq_response_to_partial_tx(resp, gas_price))
                .and_then(move |partial_tx| self_clone2.partial_tx_to_tx(&partial_tx, current_block)),
        )
    }

    fn stq_response_to_partial_tx(&self, log: StqResponseItem, gas_price: Amount) -> Result<PartialBlockchainTransaction, Error> {
        let topic = log
            .topics
            .get(0)
            .ok_or(ectx!(try err ErrorContext::Topics, ErrorKind::Internal))?
            .to_string();
        let erc20_operation_kind = if topic == self.stq_approval_topic {
            Some(Erc20OperationKind::Approve)
        } else if topic == self.stq_transfer_topic {
            Some(Erc20OperationKind::TransferFrom)
        } else {
            None
        };

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
            }).ok_or(ectx!(try err ErrorContext::Topics, ErrorKind::Internal))?;
        let block_number = EthereumClientImpl::parse_hex(log.block_number).map(|x| x as u64)?;
        let value = EthereumClientImpl::parse_hex(log.data).map(Amount::new)?;
        let log_index = EthereumClientImpl::parse_hex(log.transaction_log_index)?;
        // Since there can be many ERC-20 transfers per ETH transaction, we're giving extended hash here
        let hash = format!("{}:{}", log.transaction_hash[2..].to_string(), log_index);
        let from = vec![from];
        let to = vec![BlockchainTransactionEntry { address: to, value }];
        Ok(PartialBlockchainTransaction {
            hash,
            from,
            to,
            block_number,
            currency: Currency::Stq,
            gas_price,
            erc20_operation_kind,
        })
    }

    // Common

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
        let hash = format!("0x{}", hash);
        let params = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "eth_getBlockByHash",
            "params": [hash, false]
        });

        self.get_rpc_response::<ShortBlockResponse>(&params)
            .and_then(|resp| EthereumClientImpl::parse_hex(resp.result.number))
            .map(|x| x as u64)
    }

    fn parse_hex(s: String) -> Result<u128, Error> {
        u128::from_str_radix(&s[2..], 16).map_err(ectx!(ErrorContext::Hex, ErrorKind::Internal => s))
    }

    fn get_rpc_response<T>(&self, params: &::serde_json::Value) -> impl Future<Item = T, Error = Error> + Send
    where
        for<'a> T: Send + 'static + ::serde::Deserialize<'a>,
    {
        let http_client = self.http_client.clone();
        let params_clone = params.clone();
        let params_clone2 = params.clone();
        let infura_url = self.infura_url.clone();
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
            }).and_then(move |string| {
                serde_json::from_str::<T>(&string)
                    .map_err(ectx!(ErrorContext::Json, ErrorKind::Internal => string.clone(), params_clone2, infura_url))
            })
    }

    fn partial_tx_to_tx(
        &self,
        tx: &PartialBlockchainTransaction,
        current_block: u64,
    ) -> impl Future<Item = BlockchainTransaction, Error = Error> {
        let hash = tx.hash.split(":").nth(0).unwrap(); // handle the case of stq transaction hashes, which hash format hash:index
        let hash = format!("0x{}", hash);
        let params = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "eth_getTransactionReceipt",
            "params": [hash]
        });
        let gas_price = tx.gas_price;
        let tx = tx.clone();
        self.get_rpc_response::<TransactionReceiptResponse>(&params).and_then(move |resp| {
            let result = match resp.result {
                Some(res) => res,
                None => return futures::future::Either::A(Err(ErrorKind::NoReceipt.into()).into_future()),
            };
            let resp_clone = resp.clone();
            let gas_used = EthereumClientImpl::parse_hex(result.gas_used).map(Amount::new).into_future();
            let block_number = EthereumClientImpl::parse_hex(result.block_number).into_future();
            futures::future::Either::B(gas_used.join(block_number).and_then(move |(gas_used, block_number)| {
                gas_used
                    .checked_mul(gas_price)
                    .ok_or(ectx!(err ErrorContext::Overflow, ErrorKind::Internal => resp_clone, gas_price))
                    .map(move |fee| {
                        let confirmations = (current_block as usize) - block_number as usize;
                        BlockchainTransaction {
                            hash: tx.hash,
                            from: tx.from,
                            to: tx.to,
                            block_number: tx.block_number,
                            currency: tx.currency,
                            fee,
                            confirmations,
                            erc20_operation_kind: tx.erc20_operation_kind,
                        }
                    })
            }))
        })
    }
}

impl EthereumClient for EthereumClientImpl {
    /// Get latest wei balance
    fn get_eth_balance(&self, address: EthereumAddress) -> Box<Future<Item = Amount, Error = Error> + Send> {
        let address = format!("0x{}", address);
        let params = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "eth_getBalance",
            "params": [address, "latest"]
        });
        Box::new(
            self.get_rpc_response::<BalanceResponse>(&params)
                .and_then(|resp| EthereumClientImpl::parse_hex(resp.result).map(Amount::new)),
        )
    }

    fn last_eth_transactions(
        &self,
        start_block_hash: Option<String>,
        blocks_count: u64,
    ) -> Box<Stream<Item = BlockchainTransaction, Error = Error> + Send> {
        let self_clone = self.clone();
        Box::new(
            self.get_current_block_number()
                .into_stream()
                .map(move |current_block| {
                    let hash = start_block_hash.clone();
                    self_clone.last_eth_transactions_with_current_block(hash, blocks_count, current_block)
                }).flatten(),
        )
    }

    fn get_eth_transaction(&self, hash: String) -> Box<Future<Item = BlockchainTransaction, Error = Error> + Send> {
        let self_clone = self.clone();
        let f1 = self.get_current_block_number();
        let f2 = self.get_eth_partial_transaction(hash);
        Box::new(
            f1.join(f2)
                .and_then(move |(current_block, partial_tx)| self_clone.partial_tx_to_tx(&partial_tx, current_block)),
        )
    }

    /// Get latest stq-wei balance
    fn get_stq_balance(&self, address: EthereumAddress) -> Box<Future<Item = Amount, Error = Error> + Send> {
        let address = match serialize_address(address) {
            Ok(address) => address,
            Err(e) => return Box::new(Err(e).into_future()),
        };
        let data = format!("{}{}", self.stq_balance_method, address);
        let params = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "eth_call",
            "params": [{"to": self.stq_contract_address, "data": data}]
        });
        Box::new(
            self.get_rpc_response::<BalanceResponse>(&params)
                .and_then(|resp| EthereumClientImpl::parse_hex(resp.result).map(Amount::new)),
        )
    }

    fn last_stq_transactions(
        &self,
        start_block_hash: Option<String>,
        blocks_count: u64,
    ) -> Box<Stream<Item = BlockchainTransaction, Error = Error> + Send> {
        let self_clone = self.clone();
        Box::new(
            self.get_current_block_number()
                .into_stream()
                .map(move |current_block| {
                    let hash = start_block_hash.clone();
                    self_clone.last_stq_transactions_with_current_block(hash, blocks_count, current_block)
                }).flatten(),
        )
    }

    fn get_stq_transactions(&self, hash: String) -> Box<Stream<Item = BlockchainTransaction, Error = Error> + Send> {
        let self_clone = self.clone();
        Box::new(
            self.get_current_block_number()
                .into_stream()
                .map(move |current_block| {
                    let hash = hash.clone();
                    self_clone.get_stq_transactions_with_current_block(hash, current_block)
                }).flatten(),
        )
    }

    fn get_nonce(&self, address: EthereumAddress) -> Box<Future<Item = u64, Error = Error> + Send> {
        let address_str = format!("0x{}", address);
        let params = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "eth_getTransactionCount",
            "params": [address_str, "latest"]
        });

        Box::new(self.get_rpc_response::<NonceResponse>(&params).and_then(|resp| {
            u64::from_str_radix(&resp.result[2..], 16).map_err(ectx!(ErrorContext::Hex, ErrorKind::Internal => resp.result))
        }))
    }

    fn send_raw_tx(&self, tx: RawEthereumTransaction) -> Box<Future<Item = TxHash, Error = Error> + Send> {
        let tx_str = format!("0x{}", tx);
        let params = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "eth_sendRawTransaction",
            "params": [tx_str]
        });
        Box::new(
            self.get_rpc_response::<PostTransactionsResponse>(&params)
                .map(|resp| TxHash::new(resp.result[2..].to_string())),
        )
    }
}

pub fn bytes_to_hex(bytes: &[u8]) -> String {
    let mut res = String::with_capacity(bytes.len() * 2);
    for byte in bytes.iter() {
        res.push_str(&format!("{:02x}", byte));
    }
    res
}

pub fn hex_to_bytes(hex: String) -> Result<Vec<u8>, Error> {
    let chars: Vec<char> = hex.clone().chars().collect();
    chars
        .chunks(2)
        .map(|chunk| {
            let hex = hex.clone();
            if chunk.len() < 2 {
                let e: Error = ErrorKind::BadRequest.into();
                return Err(ectx!(err e, ErrorKind::BadRequest => hex));
            }
            let string = format!("{}{}", chunk[0], chunk[1]);
            u8::from_str_radix(&string, 16).map_err(ectx!(ErrorKind::BadRequest => hex))
        }).collect()
}

fn serialize_address(address: EthereumAddress) -> Result<String, Error> {
    hex_to_bytes(address.into_inner())
        .map(|data| to_padded_32_bytes(&data))
        .map(|bytes| bytes_to_hex(&bytes))
}

fn to_padded_32_bytes(data: &[u8]) -> Vec<u8> {
    let zeros_len = 32 - data.len();
    let mut res = Vec::with_capacity(32);
    for _ in 0..zeros_len {
        res.push(0);
    }
    res.extend(data.iter());
    res
}
