use models::*;
use serde::{Deserialize, Deserializer};

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
pub struct RpcSendTransactionsResponse {
    pub result: TxHash,
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

#[derive(Debug, Clone, Deserialize)]
pub struct RpcBlockResponse {
    pub result: Block,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Block {
    pub hash: String,
    pub previousblockhash: String,
    pub tx: Vec<String>,
    pub height: u64,
    pub confirmations: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RpcRawTransactionResponse {
    pub result: RpcRawTransaction,
}

// Every raw transaction is converted to BlockchainTransaction
// Coinbase tx however doesn't have any inputs, so it's hard to convert it
// Basically it misses txid and vout fields
// But when we convert a blockchain transactions for each vin we need to fetch a transactions that it refers to
// These transactions maybe coinbase. But we don't need their vins, so we could ignore it
// For that case we use this thing
#[derive(Debug, Clone, Deserialize)]
pub struct RpcRawTransactionMaybeCoinbaseVinsResponse {
    pub result: RpcRawTransactionWithMaybeCoinbaseVins,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RpcBalanceResponse {
    #[serde(deserialize_with = "de_bitcoin_decimal")]
    pub result: Amount,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RpcBestBlockResponse {
    pub result: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RpcRawTransaction {
    pub txid: String,
    pub vin: Vec<Vin>,
    pub vout: Vec<Vout>,
    pub confirmations: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct RpcRawTransactionWithMaybeCoinbaseVins {
    pub txid: String,
    pub vin: Vec<MaybeCoinbaseVin>,
    pub vout: Vec<Vout>,
    pub confirmations: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Vin {
    // utxo transaction
    pub txid: String,
    // utxo index
    pub vout: usize,
}

#[derive(Debug, Clone, Deserialize)]
pub struct MaybeCoinbaseVin {
    // utxo transaction
    pub txid: Option<String>,
    // utxo index
    pub vout: Option<usize>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Vout {
    pub script_pub_key: ScriptPubKey,
    #[serde(deserialize_with = "de_bitcoin_decimal")]
    pub value: Amount,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ScriptPubKey {
    #[serde(default)]
    pub addresses: Vec<String>,
    #[serde(rename = "type")]
    pub typ: String,
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

fn de_bitcoin_decimal<'de, D>(deserializer: D) -> Result<Amount, D::Error>
where
    D: Deserializer<'de>,
{
    let num: ::serde_json::Number = Deserialize::deserialize(deserializer)?;
    let s = num.to_string();
    decimal_string_to_satoshis(&s).ok_or(::serde::de::Error::custom("Failed to parse bitcoin rpc amount"))
}

fn decimal_string_to_satoshis(s: &str) -> Option<Amount> {
    let parts: Vec<&str> = s.split(".").collect();
    let int = parts.get(0)?;
    let float = parts.get(1)?;
    let mut s = float.to_string();
    // making sure we have at least 8 numbers
    for _ in s.len()..8 {
        s.push('0');
    }
    if s.len() != 8 {
        return None;
    }
    let satoshisstr = format!("{}{}", int, s);
    let val: u128 = u128::from_str_radix(&satoshisstr, 10).ok()?;
    Some(Amount::new(val))
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn test_decimal_string_to_satoshis() {
        let cases = [
            ("0.123", Some(12300000)),
            ("0.12300000", Some(12300000)),
            ("10.123", Some(1012300000)),
            ("10456789.123", Some(1045678912300000)),
            ("0.12345678", Some(12345678)),
            ("0.123456789", None),
            ("1.12345670", Some(112345670)),
        ];

        for case in cases.iter() {
            let case = case.clone();
            assert_eq!(decimal_string_to_satoshis(case.0), case.1.map(Amount::new));
        }
    }
}
