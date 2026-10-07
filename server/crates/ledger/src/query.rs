use serde::Serialize;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::error::LedgerError;

#[derive(Debug, Clone, Serialize)]
pub struct Balance {
    pub account: String,
    pub amount: i64,
}

pub async fn balance(pool: &PgPool, user_id: Uuid, account: &str) -> Result<i64, LedgerError> {
    let amount: Option<i64> =
        sqlx::query_scalar("SELECT amount FROM cf.balances WHERE user_id = $1 AND account = $2")
            .bind(user_id)
            .bind(account)
            .fetch_optional(pool)
            .await?;
    Ok(amount.unwrap_or(0))
}

pub async fn balances(pool: &PgPool, user_id: Uuid) -> Result<Vec<Balance>, LedgerError> {
    let rows = sqlx::query("SELECT account, amount FROM cf.balances WHERE user_id = $1 ORDER BY account")
        .bind(user_id)
        .fetch_all(pool)
        .await?;
    rows.iter()
        .map(|r| {
            Ok(Balance {
                account: r.try_get("account")?,
                amount: r.try_get("amount")?,
            })
        })
        .collect()
}
