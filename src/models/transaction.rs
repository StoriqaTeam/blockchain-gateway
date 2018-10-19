use std::fmt::{self, Display};

use super::amount::Amount;
use super::currency::Currency;

/// Hex encoded bitcoin transaction
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct RawBitcoinTransaction(String);

impl Display for RawBitcoinTransaction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Hex encoded ethereum transaction
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct RawEthereumTransaction(String);

impl Display for RawEthereumTransaction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Hex encoded hash of a transaction
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct TxHash(String);

#[derive(Debug, Serialize, Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct BlockchainTransaction {
    pub hash: String,
    pub from: String,
    pub to: String,
    pub block_number: u64,
    pub currency: Currency,
    pub value: Amount,
    pub fee: Amount,
    pub confirmations: usize,
}
