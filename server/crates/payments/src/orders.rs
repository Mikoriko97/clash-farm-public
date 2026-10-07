//! Orders: quote (USD price → CLASH at the oracle rate, fixed for `quote_ttl_sec`)
//! and status lookup. Ownership is always `orders.user_id = session user`.

use cf_world::Params;
use rust_decimal::Decimal;
use serde::Serialize;
use sqlx::{PgPool, Row};
use time::format_description::well_known::Rfc3339;
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

use crate::error::PaymentsError;
use crate::quote;
use crate::PaymentsRuntime;

#[derive(Debug, Clone, Serialize)]
pub struct OrderView {
    pub order_id: Uuid,
    pub item_id: String,
    pub qty: i32,
    pub price_usd_cents: i64,
    pub amount_clash: i64,
    pub burn_amount: i64,
    pub transfer_amount: i64,
    /// CLASH per USD as a decimal string (never a float on the wire).
    pub clash_per_usd: String,
    pub quote_expires_at: String,
    pub memo: String,
    pub mint: String,
    pub decimals: u8,
    pub treasury_token_account: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct OrderStatusView {
    pub order_id: Uuid,
    pub status: String,
    pub reason_code: Option<String>,
    pub amount_clash: i64,
}

pub fn memo_for(order_id: Uuid) -> String {
    format!("cf:{order_id}")
}

/// `POST /orders`: creates a quoted order for a CLASH-priced item.
pub async fn create(
    pool: &PgPool,
    runtime: &PaymentsRuntime,
    user_id: Uuid,
    wallet_tag: &str,
    item_id: &str,
    qty: i32,
    params: Params,
) -> Result<OrderView, PaymentsError> {
    let wallet = wallet_tag
        .strip_prefix("sol:")
        .ok_or(PaymentsError::WalletNotSolana)?;
    if qty < 1 || qty > params.shop_max_qty {
        return Err(PaymentsError::InvalidQty);
    }
    let row = sqlx::query(
        "SELECT price_usd_cents, (meta->>'level_min')::int AS level_min,
                (SELECT level FROM cf.islands WHERE user_id = $2) AS level
         FROM cf.items WHERE id = $1 AND enabled AND price_usd_cents IS NOT NULL",
    )
    .bind(item_id)
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .ok_or(PaymentsError::InvalidItem)?;
    let unit_cents: i32 = row.try_get("price_usd_cents")?;
    // A level-gated item (0019) is not even quoted below its level: nothing to hold unused.
    let level_min: Option<i32> = row.try_get("level_min")?;
    let level: Option<i32> = row.try_get("level")?;
    if let Some(min) = level_min.filter(|m| *m > 1) {
        if level.unwrap_or(1) < min {
            return Err(cf_world::WorldError::LevelLocked(min).into());
        }
    }
    let total_cents = i64::from(unit_cents)
        .checked_mul(i64::from(qty))
        .ok_or(PaymentsError::InvalidQty)?;
    if total_cents < i64::from(params.order_min_usd_cents) {
        return Err(PaymentsError::BelowMinimum);
    }
    let usd_per_clash: Decimal = runtime.oracle.usd_per_clash().await?;
    let amount = quote::amount_clash(total_cents, usd_per_clash, runtime.decimals)
        .ok_or(PaymentsError::OracleUnavailable)?;
    let split = quote::split(amount, params.burn_share_bp).ok_or(PaymentsError::OracleUnavailable)?;
    let rate = quote::clash_per_usd(usd_per_clash).ok_or(PaymentsError::OracleUnavailable)?;
    let expires_at = OffsetDateTime::now_utc() + Duration::seconds(i64::from(params.quote_ttl_sec.max(10)));

    let order_id: Uuid = sqlx::query_scalar(
        "INSERT INTO cf.orders (user_id, item_id, qty, price_usd_cents, clash_per_usd, amount_clash,
                                burn_amount, transfer_amount, wallet, quote_expires_at)
         VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10) RETURNING id",
    )
    .bind(user_id)
    .bind(item_id)
    .bind(qty)
    .bind(i32::try_from(total_cents).map_err(|_| PaymentsError::InvalidQty)?)
    .bind(rate)
    .bind(split.amount_clash)
    .bind(split.burn_amount)
    .bind(split.transfer_amount)
    .bind(wallet)
    .bind(expires_at)
    .fetch_one(pool)
    .await?;
    sqlx::query("INSERT INTO cf.events (user_id, name, props) VALUES ($1, 'order_quoted', $2)")
        .bind(user_id)
        .bind(serde_json::json!({ "order_id": order_id, "item": item_id, "qty": qty, "usd_cents": total_cents, "amount_clash": split.amount_clash }))
        .execute(pool)
        .await?;
    Ok(OrderView {
        order_id,
        item_id: item_id.to_owned(),
        qty,
        price_usd_cents: total_cents,
        amount_clash: split.amount_clash,
        burn_amount: split.burn_amount,
        transfer_amount: split.transfer_amount,
        clash_per_usd: rate.to_string(),
        quote_expires_at: expires_at.format(&Rfc3339).unwrap_or_default(),
        memo: memo_for(order_id),
        mint: runtime.mint.clone(),
        decimals: runtime.decimals,
        treasury_token_account: runtime.treasury_token_account.clone(),
    })
}

/// `GET /orders/{id}` — own orders only; unknown or foreign → NotFound.
pub async fn status(pool: &PgPool, user_id: Uuid, order_id: Uuid) -> Result<OrderStatusView, PaymentsError> {
    let row =
        sqlx::query("SELECT status, reason_code, amount_clash FROM cf.orders WHERE id = $1 AND user_id = $2")
            .bind(order_id)
            .bind(user_id)
            .fetch_optional(pool)
            .await?
            .ok_or(PaymentsError::NotFound)?;
    Ok(OrderStatusView {
        order_id,
        status: row.try_get("status")?,
        reason_code: row.try_get("reason_code")?,
        amount_clash: row.try_get("amount_clash")?,
    })
}
