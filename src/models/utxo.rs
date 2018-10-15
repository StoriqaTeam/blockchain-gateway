use super::Amount;

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Utxo {
    tx_hash: String,
    index: usize,
    value: Amount,
}
