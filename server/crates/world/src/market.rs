//! Selling produce and dishes for credits (migration 0008): the only way credits
//! are minted after a harvest. Same daily credits cap as before, taken from
//! `rate_events` atomically; at most the affordable whole units are sold and the
//! rest stays in the inventory. Ledger: system:mint -> player credits.

use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::WorldError;
use crate::params::Params;
use crate::plots::record_event;
use crate::{caps, catalog, inventory};

#[derive(Debug, Clone, Serialize)]
pub struct SaleResult {
    pub item_id: String,
    /// Units actually sold (the daily cap may cut the request short).
    pub qty_sold: i32,
    pub credits_earned: i64,
    /// Balance right after the sale, from the server.
    pub credits: i64,
}

/// `POST /market/sell { item_id, qty }`.
pub async fn sell(
    pool: &PgPool,
    user_id: Uuid,
    item_id: &str,
    qty: i32,
    params: Params,
) -> Result<SaleResult, WorldError> {
    if qty < 1 || qty > params.shop_max_qty {
        return Err(WorldError::InvalidQty);
    }
    let mut tx = pool.begin().await?;
    let item = catalog::get(&mut tx, item_id)
        .await?
        .filter(|i| i.enabled)
        .ok_or(WorldError::InvalidItem)?;
    let price = item
        .meta_i64("sell_credits")
        .filter(|p| *p > 0)
        .ok_or(WorldError::InvalidItem)?;
    // The cap grants credits, not units: sell the whole units the grant covers and hand
    // the rest of the grant back, so the day's count equals what was minted.
    // The day's cap grows with the island level (0019), never above the ceiling.
    let cap = params.credits_cap(crate::plots::island_level(&mut tx, user_id).await?);
    let wanted = i32::try_from(price.saturating_mul(i64::from(qty)))
        .unwrap_or(i32::MAX)
        .min(cap);
    let granted = i64::from(caps::consume_up_to(&mut tx, user_id, "credits", wanted, cap).await?);
    let qty_sold = i32::try_from(granted / price).unwrap_or(0).min(qty);
    if qty_sold == 0 {
        return Err(WorldError::DailyCap);
    }
    let credits_earned = price * i64::from(qty_sold);
    let unused = i32::try_from(granted - credits_earned).unwrap_or(0);
    caps::release(&mut tx, user_id, "credits", unused).await?;
    inventory::take(&mut tx, user_id, &item.id, qty_sold).await?;
    let ref_id = Uuid::new_v4();
    cf_ledger::post(
        &mut tx,
        "sell",
        ref_id,
        &[
            cf_ledger::Entry::system(
                cf_ledger::SYSTEM_MINT,
                cf_ledger::Currency::Credits,
                -credits_earned,
            ),
            cf_ledger::Entry::user(
                user_id,
                cf_ledger::ACCOUNT_CREDITS,
                cf_ledger::Currency::Credits,
                credits_earned,
            ),
        ],
    )
    .await?;
    let credits: i64 =
        sqlx::query_scalar("SELECT amount FROM cf.balances WHERE user_id = $1 AND account = $2")
            .bind(user_id)
            .bind(cf_ledger::ACCOUNT_CREDITS)
            .fetch_optional(&mut *tx)
            .await?
            .unwrap_or(0);
    record_event(
        &mut tx,
        user_id,
        "sell",
        serde_json::json!({ "item": item.id, "qty": qty_sold, "credits": credits_earned, "ref_id": ref_id }),
    )
    .await?;
    tx.commit().await?;
    Ok(SaleResult {
        item_id: item.id,
        qty_sold,
        credits_earned,
        credits,
    })
}
