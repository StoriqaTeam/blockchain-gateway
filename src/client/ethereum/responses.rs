use models::*;

#[derive(Debug, Clone, Deserialize)]
pub struct NonceResponse {
    pub result: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PostTransactionsResponse {
    pub result: TxHash,
}
