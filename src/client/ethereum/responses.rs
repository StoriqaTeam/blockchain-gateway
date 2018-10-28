use models::*;

#[derive(Debug, Clone, Deserialize)]
pub struct NonceResponse {
    pub result: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BlockNumberResponse {
    pub result: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct BlockByNumberResponse {
    pub result: Option<BlockResponse>,
}

#[derive(Debug, Clone, Deserialize, Default)]
pub struct BlockResponse {
    pub number: String,
    pub transactions: Vec<TransactionResponse>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionResponse {
    pub block_number: String,
    pub hash: String,
    pub from: String,
    pub to: Option<String>,
    pub value: String,
    pub gas: String,
    pub gas_price: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct PostTransactionsResponse {
    pub result: TxHash,
}

#[derive(Deserialize)]
pub struct StqResponse {
    pub result: Option<Vec<StqResponseItem>>,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct StqResponseItem {
    pub address: String,
    pub topics: Vec<String>,
    pub data: String,
    pub block_number: String,
    pub log_index: String,
    pub block_hash: String,
    pub transaction_hash: String,
}

#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ShortBlockResponse {
    pub result: ShortBlock,
}
#[derive(Debug, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct ShortBlock {
    pub number: u64,
}
