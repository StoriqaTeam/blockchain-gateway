/// Hex encoded bitcoin transaction
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct BitcoinTransaction(String);

/// Hex encoded ethereum transaction
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct EthereumTransaction(String);

/// Hex encoded hash of a transaction
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct TxHash(String);
