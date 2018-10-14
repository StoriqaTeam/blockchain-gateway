use diesel::sql_types::VarChar;

/// Hex encoded bitcoin transaction
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, FromSqlRow, AsExpression, Clone)]
#[sql_type = "VarChar"]
pub struct BitcoinTransaction(String);
derive_newtype_sql!(bitcoin_transaction, VarChar, BitcoinTransaction, BitcoinTransaction);

/// Base58 encoded ethereum transaction
#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, FromSqlRow, AsExpression, Clone)]
#[sql_type = "VarChar"]
pub struct EthereumTransaction(String);
derive_newtype_sql!(ethereum_transaction, VarChar, EthereumTransaction, EthereumTransaction);
