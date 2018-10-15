use std::fmt::{self, Display};
use std::str::FromStr;

/// Hex encoded bitcoin transaction
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct BitcoinTransaction(String);

impl Display for BitcoinTransaction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Hex encoded ethereum transaction
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct EthereumTransaction(String);

impl Display for EthereumTransaction {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Hex encoded hash of a transaction
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct TxHash(String);
