use std::fmt::{self, Display};

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, Clone, Copy)]
#[serde(rename_all = "lowercase")]
pub enum Currency {
    Eth,
    Stq,
    Btc,
}

impl Display for Currency {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            Currency::Eth => f.write_str("eth"),
            Currency::Stq => f.write_str("stq"),
            Currency::Btc => f.write_str("btc"),
        }
    }
}
