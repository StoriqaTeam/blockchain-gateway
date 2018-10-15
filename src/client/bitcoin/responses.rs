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

impl From<UtxoResponse> for Utxo {
    fn from(u: UtxoResponse) -> Self {
        Utxo {
            tx_hash: u.tx_hash_big_endian,
            index: u.tx_output_n,
            value: u.value,
        }
    }
}
