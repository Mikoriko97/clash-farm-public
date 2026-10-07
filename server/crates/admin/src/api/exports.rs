//! CSV exports for the spreadsheet people: players, orders and ledger rows, newest first,
//! capped at 10 000 rows per file. Same session rule as the rest of the API; the browser
//! downloads them by navigation, so no CSRF is involved (read-only).

use axum::extract::{Path, State};
use axum::http::header::{CACHE_CONTROL, CONTENT_DISPOSITION, CONTENT_TYPE};
use axum::response::{IntoResponse, Response};
use serde_json::Value;

use crate::api::{ApiAdmin, ApiError};
use crate::error::AdminError;
use crate::state::AdminState;

const MAX_ROWS: i64 = 10_000;

/// `GET /api/export/{players|orders|ledger}.csv`
pub async fn get(
    State(state): State<AdminState>,
    admin: ApiAdmin,
    Path(file): Path<String>,
) -> Result<Response, ApiError> {
    let (name, sql) = match file.as_str() {
        "players.csv" => ("players", PLAYERS),
        "orders.csv" => ("orders", ORDERS),
        "ledger.csv" => ("ledger", LEDGER),
        _ => return Err(ApiError(AdminError::NotFound)),
    };
    admin.heavy(&state).await?;
    let rows: Value = sqlx::query_scalar(sql)
        .bind(MAX_ROWS)
        .fetch_one(&state.pool)
        .await?;
    let csv = to_csv(&rows);
    tracing::info!(actor = %admin.wallet(), export = name, "admin_export");
    Ok((
        [
            (CONTENT_TYPE, "text/csv; charset=utf-8".to_owned()),
            (
                CONTENT_DISPOSITION,
                format!("attachment; filename=\"clash-farm-{name}.csv\""),
            ),
            (CACHE_CONTROL, "no-store".to_owned()),
        ],
        csv,
    )
        .into_response())
}

const PLAYERS: &str = "SELECT coalesce(jsonb_agg(to_jsonb(t)), '[]'::jsonb) FROM (
    SELECT u.id, u.wallet, u.nickname, coalesce(i.level, 1) AS level, coalesce(i.xp, 0) AS xp,
           coalesce(b.amount, 0) AS credits, u.reputation, u.created_at, u.last_seen_at, u.banned_at,
           (SELECT count(*) FROM cf.orders o WHERE o.user_id = u.id AND o.status = 'fulfilled') AS orders
    FROM cf.users u LEFT JOIN cf.islands i ON i.user_id = u.id
    LEFT JOIN cf.balances b ON b.user_id = u.id AND b.account = 'credits'
    ORDER BY u.created_at DESC LIMIT $1) t";

const ORDERS: &str = "SELECT coalesce(jsonb_agg(to_jsonb(t)), '[]'::jsonb) FROM (
    SELECT o.id, u.wallet, o.item_id, o.qty, o.price_usd_cents, o.amount_clash, o.burn_amount, o.transfer_amount,
           o.status, o.reason_code, o.created_at, o.updated_at
    FROM cf.orders o JOIN cf.users u ON u.id = o.user_id ORDER BY o.created_at DESC LIMIT $1) t";

const LEDGER: &str = "SELECT coalesce(jsonb_agg(to_jsonb(t)), '[]'::jsonb) FROM (
    SELECT l.id, l.created_at, u.wallet, l.account, l.currency, l.delta, l.ref_type, l.ref_id
    FROM cf.ledger l LEFT JOIN cf.users u ON u.id = l.user_id ORDER BY l.id DESC LIMIT $1) t";

/// Rows of flat JSON objects → CSV with the first row's keys as the header; every cell is
/// quoted, quotes doubled, and a leading `=+-@` is escaped so spreadsheets never run it.
pub fn to_csv(rows: &Value) -> String {
    let Some(list) = rows.as_array() else {
        return String::new();
    };
    let Some(first) = list.first().and_then(Value::as_object) else {
        return String::new();
    };
    let keys: Vec<&str> = first.keys().map(String::as_str).collect();
    let mut out = String::new();
    out.push_str(&keys.iter().map(|k| cell(k)).collect::<Vec<_>>().join(","));
    out.push('\n');
    for row in list {
        let line: Vec<String> = keys
            .iter()
            .map(|k| match row.get(*k) {
                None | Some(Value::Null) => String::new(),
                Some(Value::String(s)) => cell(s),
                Some(Value::Number(n)) => n.to_string(),
                Some(Value::Bool(b)) => b.to_string(),
                Some(other) => cell(&other.to_string()),
            })
            .collect();
        out.push_str(&line.join(","));
        out.push('\n');
    }
    out
}

fn cell(s: &str) -> String {
    let guarded = if s.starts_with(['=', '+', '-', '@', '\t', '\r']) {
        format!("'{s}")
    } else {
        s.to_owned()
    };
    format!("\"{}\"", guarded.replace('"', "\"\""))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn csv_quotes_and_neutralises_formulas() {
        let rows =
            serde_json::json!([{ "a": "x\"y", "b": 5, "c": "=SUM(1)" }, { "a": null, "b": -1, "c": "@x" }]);
        let csv = to_csv(&rows);
        assert_eq!(
            csv, "\"a\",\"b\",\"c\"\n\"x\"\"y\",5,\"'=SUM(1)\"\n,-1,\"'@x\"\n",
            "numbers bare (a negative delta is not a formula), text guarded"
        );
        assert_eq!(to_csv(&serde_json::json!([{ "t": "\tx" }])), "\"t\"\n\"'\tx\"\n");
        assert_eq!(to_csv(&serde_json::json!([])), "");
    }
}
