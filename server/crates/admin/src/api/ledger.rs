//! Ledger explorer: system accounts (mint / sink / burn) as totals and a 30-day series,
//! a parameterised search over rows with the player's wallet, and an on-demand
//! ledger-vs-balances check (an explicit, rate-limited, audited operator action — the
//! nightly `reconcile` job does the same on its own).

use axum::extract::{Query, State};
use axum::http::HeaderMap;
use axum::Json;
use serde::Deserialize;
use serde_json::Value;
use sqlx::Row;
use uuid::Uuid;

use crate::api::{like_pattern, ApiAdmin, ApiError, PageQuery};
use crate::audit;
use crate::error::AdminError;
use crate::state::AdminState;

/// `GET /api/ledger/summary`
pub async fn summary(State(state): State<AdminState>, _admin: ApiAdmin) -> Result<Json<Value>, ApiError> {
    let totals: Value = sqlx::query_scalar(
        "SELECT coalesce(jsonb_agg(to_jsonb(t) ORDER BY account), '[]'::jsonb) FROM (
            SELECT account, currency,
                   -sum(delta) FILTER (WHERE delta < 0) AS given,
                   sum(delta) FILTER (WHERE delta > 0) AS taken,
                   -sum(delta) FILTER (WHERE delta < 0 AND created_at > now() - interval '30 days') AS given_30d,
                   sum(delta) FILTER (WHERE delta > 0 AND created_at > now() - interval '30 days') AS taken_30d,
                   count(*) AS rows
            FROM cf.ledger WHERE user_id IS NULL GROUP BY account, currency) t",
    )
    .fetch_one(&state.pool)
    .await?;
    let days: Value = sqlx::query_scalar(
        "SELECT coalesce(jsonb_agg(to_jsonb(t) ORDER BY day), '[]'::jsonb) FROM (
            SELECT d.day::date AS day,
                   coalesce((SELECT -sum(delta) FROM cf.ledger l WHERE l.account = 'system:mint' AND l.currency = 'CREDITS'
                             AND l.created_at >= d.day AND l.created_at < d.day + interval '1 day'), 0) AS minted,
                   coalesce((SELECT sum(delta) FROM cf.ledger l WHERE l.account = 'system:sink' AND l.currency = 'CREDITS'
                             AND l.created_at >= d.day AND l.created_at < d.day + interval '1 day'), 0) AS spent
            FROM generate_series(date_trunc('day', now()) - interval '29 days', date_trunc('day', now()), interval '1 day') AS d(day)) t",
    )
    .fetch_one(&state.pool)
    .await?;
    let by_type: Value = sqlx::query_scalar(
        "SELECT coalesce(jsonb_agg(to_jsonb(t) ORDER BY rows DESC), '[]'::jsonb) FROM (
            SELECT ref_type, count(*) AS rows, sum(abs(delta)) / 2 AS volume
            FROM cf.ledger WHERE currency = 'CREDITS' AND created_at > now() - interval '30 days' GROUP BY ref_type) t",
    )
    .fetch_one(&state.pool)
    .await?;
    let holders: Value = sqlx::query_scalar(
        "SELECT coalesce(jsonb_agg(to_jsonb(t) ORDER BY amount DESC), '[]'::jsonb) FROM (
            SELECT u.id AS user_id, u.wallet, u.nickname, b.amount
            FROM cf.balances b JOIN cf.users u ON u.id = b.user_id
            WHERE b.account = 'credits' ORDER BY b.amount DESC LIMIT 10) t",
    )
    .fetch_one(&state.pool)
    .await?;
    let in_circulation: i64 = sqlx::query_scalar(
        "SELECT coalesce(sum(amount), 0)::bigint FROM cf.balances WHERE account = 'credits'",
    )
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(serde_json::json!({
        "system": totals, "days": days, "by_type": by_type, "top_holders": holders, "in_circulation": in_circulation,
    })))
}

#[derive(Debug, Deserialize, Default)]
pub struct SearchQuery {
    /// Wallet or nickname fragment, or a user id.
    pub q: Option<String>,
    pub account: Option<String>,
    pub ref_type: Option<String>,
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

/// `GET /api/ledger?q=&account=&ref_type=&page=` → newest first with the wallet.
pub async fn search(
    State(state): State<AdminState>,
    _admin: ApiAdmin,
    Query(q): Query<SearchQuery>,
) -> Result<Json<Value>, ApiError> {
    let needle: String = q.q.unwrap_or_default().trim().chars().take(64).collect();
    let account: String = q.account.unwrap_or_default().chars().take(40).collect();
    valid_account(&account)?;
    let ref_type: String = q.ref_type.unwrap_or_default().chars().take(40).collect();
    let (limit, offset) = PageQuery {
        page: q.page,
        per_page: q.per_page,
    }
    .bounds();
    let rows = sqlx::query(
        "SELECT l.id, l.user_id, u.wallet, l.account, l.delta, l.currency, l.ref_type, l.ref_id, l.created_at, count(*) OVER () AS total
         FROM cf.ledger l LEFT JOIN cf.users u ON u.id = l.user_id
         WHERE ($1 = '' OR u.wallet ILIKE $2 OR coalesce(u.nickname, '') ILIKE $2 OR l.user_id::text = $1 OR l.ref_id::text = $1)
           AND ($3 = '' OR l.account = $3)
           AND ($4 = '' OR l.ref_type = $4)
         ORDER BY l.id DESC LIMIT $5 OFFSET $6",
    )
    .bind(&needle)
    .bind(like_pattern(&needle))
    .bind(&account)
    .bind(&ref_type)
    .bind(limit)
    .bind(offset)
    .fetch_all(&state.pool)
    .await?;
    let total: i64 = rows.first().map(|r| r.try_get("total")).transpose()?.unwrap_or(0);
    let mut out = Vec::with_capacity(rows.len());
    for r in &rows {
        out.push(serde_json::json!({
            "id": r.try_get::<i64, _>("id")?,
            "user_id": r.try_get::<Option<Uuid>, _>("user_id")?,
            "wallet": r.try_get::<Option<String>, _>("wallet")?,
            "account": r.try_get::<String, _>("account")?,
            "delta": r.try_get::<i64, _>("delta")?,
            "currency": r.try_get::<String, _>("currency")?,
            "ref_type": r.try_get::<String, _>("ref_type")?,
            "ref_id": r.try_get::<Uuid, _>("ref_id")?,
            "created_at": r.try_get::<time::OffsetDateTime, _>("created_at")?.format(&time::format_description::well_known::Rfc3339).unwrap_or_default(),
        }));
    }
    Ok(Json(serde_json::json!({ "total": total, "rows": out })))
}

/// `POST /api/ledger/reconcile`: ledger sums vs materialised balances, right now. Heavy on a
/// big ledger, so it is a deliberate click, under the mutation limit, and in the audit log.
pub async fn reconcile(
    State(state): State<AdminState>,
    admin: ApiAdmin,
    headers: HeaderMap,
) -> Result<Json<Value>, ApiError> {
    admin.guard(&state, &headers).await?;
    admin.heavy(&state).await?;
    let mismatches: Value = sqlx::query_scalar(
        "SELECT coalesce(jsonb_agg(to_jsonb(t)), '[]'::jsonb) FROM (
            SELECT l.user_id, u.wallet, l.account, l.s AS ledger, b.amount AS balance
            FROM (SELECT user_id, account, sum(delta) AS s FROM cf.ledger
                  WHERE user_id IS NOT NULL AND currency = 'CREDITS' GROUP BY user_id, account) l
            JOIN cf.balances b ON b.user_id = l.user_id AND b.account = l.account
            JOIN cf.users u ON u.id = l.user_id
            WHERE l.s <> b.amount LIMIT 50) t",
    )
    .fetch_one(&state.pool)
    .await?;
    let zero_sum: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM (SELECT ref_id, currency FROM cf.ledger GROUP BY ref_id, currency HAVING sum(delta) <> 0) x",
    )
    .fetch_one(&state.pool)
    .await?;
    let count = mismatches.as_array().map(Vec::len).unwrap_or(0);
    let mut conn = state.pool.acquire().await?;
    audit::record(
        &mut conn,
        admin.wallet(),
        "ledger_reconcile",
        None,
        Value::Null,
        serde_json::json!({ "mismatches": count, "unbalanced_refs": zero_sum }),
        None,
    )
    .await?;
    if count > 0 || zero_sum > 0 {
        tracing::error!(actor = %admin.wallet(), mismatches = count, unbalanced_refs = zero_sum, "admin_ledger_reconcile_found");
    }
    Ok(Json(
        serde_json::json!({ "mismatches": mismatches, "unbalanced_refs": zero_sum }),
    ))
}

/// Keeps the ledger routes honest about their inputs: an account name is `credits` or `system:*`.
pub fn valid_account(account: &str) -> Result<(), ApiError> {
    if account.is_empty() || account == "credits" || account.starts_with("system:") {
        Ok(())
    } else {
        Err(ApiError(AdminError::BadRequest("unknown account".to_owned())))
    }
}
