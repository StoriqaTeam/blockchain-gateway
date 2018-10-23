use models::*;

#[derive(Debug, Clone, Deserialize)]
pub struct UtxosResponse {
    pub unspent_outputs: Vec<UtxoResponse>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct UtxoResponse {
    pub tx_hash_big_endian: String,
    pub tx_output_n: usize,
    pub value: Amount,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PostTransactionsResponse {
    pub hash: TxHash,
}

#[derive(Debug, Clone, Deserialize)]
pub struct GetTransactionResponse {
    pub hash: String,
    pub inputs: Vec<TransactionInputResponse>,
    pub out: Vec<TransactionOutputResponse>,
    pub block_height: u64,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TransactionInputResponse {
    pub prev_out: TransactionOutputResponse,
}

#[derive(Debug, Clone, Deserialize)]
pub struct TransactionOutputResponse {
    pub addr: String,
    pub value: Amount,
}

impl From<UtxoResponse> for Utxo {
    fn from(u: UtxoResponse) -> Self {
        Utxo {
            tx_hash: u.tx_hash_big_endian,
            index: u.tx_output_n,
            value: u.value,
        }
    }
}
