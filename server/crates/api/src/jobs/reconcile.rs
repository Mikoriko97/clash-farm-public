//! Nightly reconciliation (docs/SECURITY.md §5, RUNBOOK alerts): every
//! materialised balance must equal the sum of its ledger rows. A mismatch is
//! an S1 alert — it goes to `audit_log` (admin home page) and the error log.
//! The job never repairs anything: fixes are compensating ledger entries by hand.

use serde_json::json;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use super::{alert, JobError};

/// How many mismatches one run reports in detail.
const REPORT_LIMIT: usize = 50;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Mismatch {
    pub user_id: Uuid,
    pub account: String,
    pub balance: i64,
    pub ledger_sum: i64,
}

pub async fn run(pool: &PgPool) -> Result<String, JobError> {
    let mismatches = ledger_vs_balances(pool).await?;
    if mismatches.is_empty() {
        return Ok("mismatches=0".to_owned());
    }
    let detail: Vec<_> = mismatches
        .iter()
        .take(REPORT_LIMIT)
        .map(|m| json!({ "user_id": m.user_id, "account": m.account, "balance": m.balance, "ledger": m.ledger_sum }))
        .collect();
    alert(
        pool,
        "reconcile",
        "ledger_mismatch",
        None,
        json!({ "count": mismatches.len(), "sample": detail }),
    )
    .await?;
    Ok(format!("mismatches={}", mismatches.len()))
}

/// Balances vs ledger sums per (user, account); either side missing counts as 0.
pub async fn ledger_vs_balances(pool: &PgPool) -> Result<Vec<Mismatch>, JobError> {
    let rows = sqlx::query(
        "WITH sums AS (
             SELECT user_id, account, SUM(delta)::bigint AS total
             FROM cf.ledger WHERE user_id IS NOT NULL GROUP BY user_id, account
         )
         SELECT COALESCE(b.user_id, s.user_id) AS user_id,
                COALESCE(b.account, s.account) AS account,
                COALESCE(b.amount, 0)::bigint AS balance,
                COALESCE(s.total, 0)::bigint AS ledger_sum
         FROM cf.balances b
         FULL OUTER JOIN sums s ON s.user_id = b.user_id AND s.account = b.account
         WHERE COALESCE(b.amount, 0) <> COALESCE(s.total, 0)
         ORDER BY 1, 2",
    )
    .fetch_all(pool)
    .await?;
    rows.iter()
        .map(|r| {
            Ok(Mismatch {
                user_id: r.try_get("user_id")?,
                account: r.try_get("account")?,
                balance: r.try_get("balance")?,
                ledger_sum: r.try_get("ledger_sum")?,
            })
        })
        .collect()
}
