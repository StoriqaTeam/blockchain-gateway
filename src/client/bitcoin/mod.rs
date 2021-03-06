mod responses;

use std::collections::HashMap;
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

/// Client for working with Bitcoin blockchain
pub trait BitcoinClient: Send + Sync + 'static {
    /// Get available Utxos for bitcoin address
    fn get_utxos(&self, address: BitcoinAddress) -> Box<Future<Item = Vec<Utxo>, Error = Error> + Send>;
    /// Get balance for bitcoin address
    fn get_balance(&self, address: BitcoinAddress) -> Box<Future<Item = Amount, Error = Error> + Send>;

    /// Send raw transaction to blockchain
    fn send_raw_tx(&self, tx: RawBitcoinTransaction) -> Box<Future<Item = TxHash, Error = Error> + Send>;
    /// Get transaction by hash. Since getting block_number from transaction is not yet
    /// supported, you need to provide one in arguments
    fn get_transaction(&self, hash: String, block_number: u64) -> Box<Future<Item = BlockchainTransaction, Error = Error> + Send>;
    /// Get blocks starting from `start_block_hash` (or the most recent block if not specified)
    /// and fetch previous blocks. Total number of blocks = `blocks_count`.
    /// `blocks_count` should be greater than 0.
    fn last_blocks(&self, start_block_hash: Option<String>, blocks_count: u64) -> Box<Stream<Item = Block, Error = Error> + Send>;
    /// Same as `last_blocks`, but returns transactions instead
    fn last_transactions(
        &self,
        start_block_hash: Option<String>,
        blocks_count: u64,
    ) -> Box<Stream<Item = BlockchainTransaction, Error = Error> + Send>;
}

#[derive(Clone)]
pub struct BitcoinClientImpl {
    http_client: Arc<HttpClient>,
    mode: Mode,
    bitcoin_rpc_url: String,
    bitcoin_rpc_user: String,
    bitcoin_rpc_password: String,
}

const BLOCK_TXS_LIMIT: u64 = 10;

impl BitcoinClientImpl {
    pub fn new(
        http_client: Arc<HttpClient>,
        mode: Mode,
        bitcoin_rpc_url: String,
        bitcoin_rpc_user: String,
        bitcoin_rpc_password: String,
    ) -> Self {
        Self {
            http_client,
            mode,
            bitcoin_rpc_url,
            bitcoin_rpc_user,
            bitcoin_rpc_password,
        }
    }

    fn get_rpc_response<T>(&self, params: &::serde_json::Value) -> impl Future<Item = T, Error = Error> + Send
    where
        for<'a> T: Send + 'static + ::serde::Deserialize<'a>,
    {
        let http_client = self.http_client.clone();
        let params_clone = params.clone();
        let basic = ::base64::encode(&format!("{}:{}", self.bitcoin_rpc_user, self.bitcoin_rpc_password));
        let basic = format!("Basic {}", basic);
        serde_json::to_string(params)
            .map_err(ectx!(ErrorContext::Json, ErrorKind::Internal => params))
            .and_then(|body| {
                Request::builder()
                    .method("POST")
                    .header("Authorization", basic)
                    .uri(self.bitcoin_rpc_url.clone())
                    .body(Body::from(body.clone()))
                    .map_err(ectx!(ErrorSource::Hyper, ErrorKind::Internal => body))
            })
            .into_future()
            .and_then(move |request| http_client.request(request))
            .and_then(|resp| read_body(resp.into_body()).map_err(ectx!(ErrorKind::Internal => params_clone)))
            .and_then(|bytes| {
                let bytes_clone = bytes.clone();
                String::from_utf8(bytes).map_err(ectx!(ErrorContext::UTF8, ErrorKind::Internal => bytes_clone))
            })
            .and_then(|string| serde_json::from_str::<T>(&string).map_err(ectx!(ErrorContext::Json, ErrorKind::Internal => string.clone())))
    }

    fn block_transactions(&self, block: Block) -> impl Stream<Item = BlockchainTransaction, Error = Error> {
        let self_clone = self.clone();
        let Block {
            tx: transactions,
            height: block_number,
            ..
        } = block;
        // skipping coinbase transaction
        let hash_stream = stream::iter_ok(transactions.into_iter().skip(1));
        hash_stream
            .chunks(BLOCK_TXS_LIMIT as usize)
            .and_then(move |hashes| {
                let self_clone = self_clone.clone();
                let fs = hashes.into_iter().map(move |hash| self_clone.get_transaction(hash, block_number));
                future::join_all(fs)
            })
            .map(|x| stream::iter_ok(x))
            .flatten()
    }

    fn last_blocks_from_hash(&self, block_hash: String, block_count: u64) -> impl Stream<Item = Block, Error = Error> + Send {
        let self_clone = self.clone();
        stream::unfold((block_count, block_hash), move |(blocks_left, current_hash)| {
            if blocks_left == 0 {
                return None;
            }
            let f = self_clone
                .get_block_by_hash(current_hash)
                .map(move |block| (block.clone(), (blocks_left - 1, block.previousblockhash.clone())));
            Some(f)
        })
    }

    fn get_best_block_hash(&self) -> impl Future<Item = String, Error = Error> + Send {
        let params = json!({
            "jsonrpc": "2",
            "id": "1",
            "method": "getbestblockhash",
            "params": []
        });
        self.get_rpc_response::<RpcBestBlockResponse>(&params).map(|r| r.result)
    }

    pub fn get_block_by_hash(&self, hash: String) -> impl Future<Item = Block, Error = Error> + Send {
        let params = json!({
            "jsonrpc": "2",
            "id": "1",
            "method": "getblock",
            "params": [hash]
        });
        self.get_rpc_response::<RpcBlockResponse>(&params).map(|r| r.result)
    }

    fn rpc_tx_to_tx(
        tx: RpcRawTransaction,
        in_txs: Vec<RpcRawTransactionWithMaybeCoinbaseVins>,
        block_number: u64,
    ) -> Result<BlockchainTransaction, Error> {
        let RpcRawTransaction {
            txid: hash,
            vin: vins,
            vout: vouts,
            confirmations,
        } = tx;
        let hash_clone = hash.clone();
        let hash_clone2 = hash.clone();
        let in_txs_hash: HashMap<String, RpcRawTransactionWithMaybeCoinbaseVins> =
            in_txs.into_iter().map(|tx| (tx.txid.clone(), tx)).collect();
        let from: Result<Vec<BlockchainTransactionEntry>, Error> = vins
            .iter()
            .map(|vin| {
                let Vin { txid: hash, vout: index } = vin;
                let out_tx = in_txs_hash
                    .get(hash)
                    .cloned()
                    .ok_or(ectx!(try err ErrorContext::BitcoinRpcConversion, ErrorKind::Internal => hash_clone.clone()))?;
                let out_out = out_tx
                    .vout
                    .get(*index)
                    .ok_or(ectx!(try err ErrorContext::BitcoinRpcConversion, ErrorKind::Internal => hash_clone.clone()))?;
                let value = out_out.value;
                Ok(BlockchainTransactionEntry {
                    address: out_out.script_pub_key.addresses.get(0).cloned().unwrap_or("0".to_string()),
                    value,
                })
            })
            .collect();
        let from = from?;
        let to: Vec<_> = vouts
            .iter()
            .map(|vout| {
                let Vout { script_pub_key, value } = vout;
                BlockchainTransactionEntry {
                    // TODO - figure out the case with scripthash, so far we say that address is 0 in this case
                    address: script_pub_key.addresses.get(0).cloned().unwrap_or("0".to_string()),
                    value: *value,
                }
            })
            .collect();
        let from_sum = from.iter().fold(Some(Amount::new(0)), |acc, item| {
            acc.and_then(|acc_val| acc_val.checked_add(item.value))
        });
        let to_sum = to.iter().fold(Some(Amount::new(0)), |acc, item| {
            acc.and_then(|acc_val| acc_val.checked_add(item.value))
        });
        let fee = match (from_sum, to_sum) {
            (Some(fs), Some(ts)) => fs.checked_sub(ts),
            _ => None,
        }
        .ok_or(ectx!(try err ErrorContext::Overflow, ErrorKind::Internal => hash_clone2, from_sum, to_sum))?;
        let from: Vec<_> = from
            .into_iter()
            .filter_map(|entry| match entry {
                // these kind of entries are ok for bitcoin, but they are script based, we ignore such entries
                // also note, that they can contain values, therefore we filter it here, rather than above,
                // so that from_sum and to_sum is correct
                ref x if x.address == "0" => None,
                x @ _ => Some(x),
            })
            .map(|from| from.address)
            .collect();
        Ok(BlockchainTransaction {
            hash,
            from,
            to,
            block_number,
            currency: Currency::Btc,
            fee,
            confirmations,
            erc20_operation_kind: None,
        })
    }
}

impl BitcoinClient for BitcoinClientImpl {
    fn get_balance(&self, address: BitcoinAddress) -> Box<Future<Item = Amount, Error = Error> + Send> {
        Box::new(self.get_utxos(address).and_then(|utxos| {
            utxos
                .into_iter()
                .fold(Some(Amount::new(0)), |acc, elem| acc.and_then(|acc| acc.checked_add(elem.value)))
                .ok_or(ectx!(err ErrorContext::Overflow, ErrorKind::Internal))
        }))
    }
    fn get_transaction(&self, hash: String, block_number: u64) -> Box<Future<Item = BlockchainTransaction, Error = Error> + Send> {
        let params = json!({
            "jsonrpc": "2",
            "id": "1",
            "method": "getrawtransaction",
            "params": [hash, true]
        });
        let self_clone = self.clone();
        Box::new(
            self.get_rpc_response::<RpcRawTransactionResponse>(&params)
                .and_then(move |resp| {
                    let self_clone = self_clone.clone();
                    let in_transaction_fs: Vec<_> = resp
                        .result
                        .vin
                        .iter()
                        .map(move |vin| {
                            let params = json!({
                                "jsonrpc": "2",
                                "id": "1",
                                "method": "getrawtransaction",
                                "params": [vin.txid, true]
                            });
                            self_clone
                                .get_rpc_response::<RpcRawTransactionMaybeCoinbaseVinsResponse>(&params)
                                .map(|r| r.result)
                        })
                        .collect();
                    future::join_all(in_transaction_fs).map(move |in_transactions| (resp, in_transactions))
                })
                .and_then(move |(tx_resp, in_txs_resp)| BitcoinClientImpl::rpc_tx_to_tx(tx_resp.result, in_txs_resp, block_number)),
        )
    }

    fn last_blocks(&self, start_block_hash: Option<String>, blocks_count: u64) -> Box<Stream<Item = Block, Error = Error> + Send> {
        let self_clone = self.clone();
        let start_hash_f = match start_block_hash {
            Some(hash) => future::Either::A(Ok(hash).into_future()),
            None => future::Either::B(self.get_best_block_hash()),
        };
        Box::new(
            start_hash_f
                .into_stream()
                .map(move |block_hash| self_clone.last_blocks_from_hash(block_hash, blocks_count))
                .flatten(),
        )
    }

    fn last_transactions(
        &self,
        start_block_hash: Option<String>,
        blocks_count: u64,
    ) -> Box<Stream<Item = BlockchainTransaction, Error = Error> + Send> {
        let self_clone = self.clone();
        Box::new(
            self.last_blocks(start_block_hash, blocks_count)
                .map(move |block| self_clone.block_transactions(block))
                .flatten(),
        )
    }

    fn get_utxos(&self, address: BitcoinAddress) -> Box<Future<Item = Vec<Utxo>, Error = Error> + Send> {
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
                })
                .and_then(|string| {
                    serde_json::from_str::<UtxosResponse>(&string).map_err(ectx!(ErrorContext::Json, ErrorKind::Internal => string.clone()))
                })
                .map(|resp| resp.unspent_outputs.into_iter().map(From::from).collect()),
        )
    }

    fn send_raw_tx(&self, tx: RawBitcoinTransaction) -> Box<Future<Item = TxHash, Error = Error> + Send> {
        let tx = format!("{}", tx);
        let params = json!({
            "jsonrpc": "2",
            "id": "1",
            "method": "sendrawtransaction",
            "params": [tx]
        });
        Box::new(
            self.get_rpc_response::<RpcSendTransactionsResponse>(&params)
                .map(|resp| resp.result),
        )
    }
}
