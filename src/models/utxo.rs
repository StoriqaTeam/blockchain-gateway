use diesel::sql_types::VarChar;

use super::Amount;

#[derive(Debug, Serialize, Deserialize, PartialEq, Eq, FromSqlRow, AsExpression, Clone)]
#[sql_type = "VarChar"]
#[serde(rename_all = "camelCase")]
pub struct Utxo {
    tx_hash: String,
    index: usize,
    value: Amount,
}
