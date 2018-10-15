use models::*;

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct NonceResponse {
    pub nonce: u64,
}

#[derive(Debug, Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct TxHashResponse {
    pub tx_hash: TxHash,
}
