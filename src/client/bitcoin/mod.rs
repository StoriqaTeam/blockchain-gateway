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
    bitcoin_rpc_url: String,
    bitcoin_rpc_user: String,
    bitcoin_rpc_password: String,
}

const BLOCK_TXS_LIMIT: u64 = 10;

impl BitcoinClientImpl {
    pub fn new(
        http_client: Arc<HttpClient>,
        blockcypher_token: String,
        mode: Mode,
        bitcoin_rpc_url: String,
        bitcoin_rpc_user: String,
        bitcoin_rpc_password: String,
    ) -> Self {
        Self {
            http_client,
            blockcypher_token,
            mode,
            bitcoin_rpc_url,
            bitcoin_rpc_user,
            bitcoin_rpc_password,
        }
    }

    // fn get_transactions_by_hashes(
    //     &self,
    //     hash_stream: impl Stream<Item = String, Error = Error> + Send,
    // ) -> Box<Stream<Item = BlockchainTransaction, Error = Error> + Send> {
    //     let self_clone = self.clone();
    //     Box::new(
    //         hash_stream
    //             .chunks(BLOCK_TXS_LIMIT as usize)
    //             .and_then(move |hashes| {
    //                 let self_clone = self_clone.clone();
    //                 let fs = hashes.into_iter().map(move |hash| self_clone.get_transaction_by_hash(hash));
    //                 future::join_all(fs)
    //             }).map(|x| stream::iter_ok(x))
    //             .flatten(),
    //     )
    // }

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

    pub fn last_transactions(&self, block_count: u64) -> impl Stream<Item = BlockchainTransaction, Error = Error> + Send {
        let self_clone = self.clone();
        self.last_blocks(block_count)
            .map(move |block| self_clone.block_transactions(block))
            .flatten()
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
                let fs = hashes
                    .into_iter()
                    .map(move |hash| self_clone.get_transaction_by_hash(hash, block_number));
                future::join_all(fs)
            }).map(|x| stream::iter_ok(x))
            .flatten()
    }

    pub fn last_blocks(&self, block_count: u64) -> impl Stream<Item = Block, Error = Error> + Send {
        let self_clone = self.clone();
        self.get_best_block_hash()
            .into_stream()
            .map(move |block_hash| self_clone.last_blocks_from_hash(block_hash, block_count))
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

    pub fn get_transaction_by_hash(
        &self,
        hash: String,
        block_number: u64,
    ) -> impl Future<Item = BlockchainTransaction, Error = Error> + Send {
        let params = json!({
            "jsonrpc": "2",
            "id": "1",
            "method": "getrawtransaction",
            "params": [hash, true]
        });
        let self_clone = self.clone();
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
                        self_clone.get_rpc_response::<RpcRawTransactionResponse>(&params).map(|r| r.result)
                    }).collect();
                future::join_all(in_transaction_fs).map(move |in_transactions| (resp, in_transactions))
            }).and_then(move |(tx_resp, in_txs_resp)| BitcoinClientImpl::rpc_tx_to_tx(tx_resp.result, in_txs_resp, block_number))
    }

    fn rpc_tx_to_tx(tx: RpcRawTransaction, in_txs: Vec<RpcRawTransaction>, block_number: u64) -> Result<BlockchainTransaction, Error> {
        let RpcRawTransaction {
            txid: hash,
            vin: vins,
            vout: vouts,
            confirmations,
        } = tx;
        let hash_clone = hash.clone();
        let hash_clone2 = hash.clone();
        let in_txs_hash: HashMap<String, RpcRawTransaction> = in_txs.into_iter().map(|tx| (tx.txid.clone(), tx)).collect();
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
                    // TODO - figure out the case with scripthash, so far we say that address is 0 in this case
                    address: out_out.script_pub_key.addresses.get(0).cloned().unwrap_or("0x0".to_string()),
                    value,
                })
            }).collect();
        let from = from?;
        let to: Vec<_> = vouts
            .iter()
            .map(|vout| {
                let Vout { script_pub_key, value } = vout;
                BlockchainTransactionEntry {
                    // TODO - figure out the case with scripthash, so far we say that address is 0 in this case
                    address: script_pub_key.addresses.get(0).cloned().unwrap_or("0x0".to_string()),
                    value: *value,
                }
            }).collect();
        let from_sum = from.iter().fold(Some(Amount::new(0)), |acc, item| {
            acc.and_then(|acc_val| acc_val.checked_add(item.value))
        });
        let to_sum = to.iter().fold(Some(Amount::new(0)), |acc, item| {
            acc.and_then(|acc_val| acc_val.checked_add(item.value))
        });
        let fee = match (from_sum, to_sum) {
            (Some(fs), Some(ts)) => fs.checked_sub(ts),
            _ => None,
        }.ok_or(ectx!(try err ErrorContext::Overflow, ErrorKind::Internal => hash_clone2, from_sum, to_sum))?;
        let from: Vec<_> = from.into_iter().map(|from| from.address).collect();
        Ok(BlockchainTransaction {
            hash,
            from,
            to,
            block_number,
            currency: Currency::Btc,
            fee,
            confirmations,
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
        unimplemented!()
    }
}

impl BitcoinClient for BitcoinClientImpl {
    fn get_transactions(&self, from_block: u64, to_block: u64) -> Box<Future<Item = Vec<BlockchainTransaction>, Error = Error> + Send> {
        // (from_block..=to_block).iter().map(|block| )
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
