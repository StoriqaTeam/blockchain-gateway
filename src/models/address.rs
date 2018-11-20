use std::fmt::{self, Display};
use std::str::FromStr;

/// Base58 encoded bitcoin address
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct BitcoinAddress(String);

impl Display for BitcoinAddress {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for BitcoinAddress {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(BitcoinAddress(s.to_string()))
    }
}

/// Base58 encoded ethereum address
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone)]
pub struct EthereumAddress(String);

impl EthereumAddress {
    pub fn into_inner(self) -> String {
        self.0
    }
}

impl Display for EthereumAddress {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        f.write_str(&self.0)
    }
}

impl FromStr for EthereumAddress {
    type Err = ();
    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(EthereumAddress(s.to_string()))
    }
}
