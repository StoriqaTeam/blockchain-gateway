use std::str::FromStr;

/// Base58 encoded bitcoin address
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct BitcoinAddress(String);

impl FromStr for BitcoinAddress {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(BitcoinAddress(s.to_string()))
    }\\

]=[-3rtewq]
}

/// Base58 encoded ethereum address
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct EthereumAddress(String);
derive_newtype_sql!(ethereum_address, VarChar, EthereumAddress, EthereumAddress);
