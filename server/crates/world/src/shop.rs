//! Purchases for credits (docs/API.md `/shop/buy-credits`): ledger debit and
//! inventory credit in one transaction; the balance CHECK is the last barrier.

use cf_ledger::{Currency, Entry, ACCOUNT_CREDITS, SYSTEM_SINK};
use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::WorldError;
use crate::params::Params;
use crate::{catalog, inventory};

#[derive(Debug, Clone, Serialize)]
pub struct Purchase {
    pub item_id: String,
    pub qty: i32,
    pub spent_credits: i64,
    pub credits_left: i64,
}

pub const PLOT_ITEM: &str = "plot";

/// Locks the island row (parallel purchases count each other's plots) and returns its level and
/// the plots it owns: placed ones plus those still in the bag.
async fn lock_plots_owned(conn: &mut sqlx::PgConnection, user_id: Uuid) -> Result<(i32, i64), WorldError> {
    let level: i32 = sqlx::query_scalar("SELECT level FROM cf.islands WHERE user_id = $1 FOR UPDATE")
        .bind(user_id)
        .fetch_optional(&mut *conn)
        .await?
        .ok_or(WorldError::NotFound)?;
    let owned: i64 = sqlx::query_scalar(
        "SELECT (SELECT count(*) FROM cf.plots p JOIN cf.islands i ON i.id = p.island_id WHERE i.user_id = $1)
              + COALESCE((SELECT qty FROM cf.inventory WHERE user_id = $1 AND item_id = $2), 0)",
    )
    .bind(user_id)
    .bind(PLOT_ITEM)
    .fetch_one(conn)
    .await?;
    Ok((level, owned))
}

/// Plots beyond the starters cost `plot_price_step` more each (0019): the n-th extra plot
/// (placed or still in the bag) costs `base + step × (n - 1)`; `qty` of them sum the series.
pub async fn plot_price_total(
    conn: &mut sqlx::PgConnection,
    user_id: Uuid,
    base: i64,
    qty: i32,
    params: Params,
) -> Result<i64, WorldError> {
    let (_, owned) = lock_plots_owned(conn, user_id).await?;
    plot_series(owned, base, qty, params)
}

fn plot_series(owned: i64, base: i64, qty: i32, params: Params) -> Result<i64, WorldError> {
    let extra = (owned - i64::from(params.plots_start)).max(0);
    let step = i64::from(params.plot_price_step);
    let n = i64::from(qty);
    // Σ_{k=0}^{n-1} (base + step × (extra + k)) = n × base + step × (n × extra + n(n-1)/2)
    n.checked_mul(base)
        .and_then(|b| {
            step.checked_mul(n * extra + n * (n - 1) / 2)
                .and_then(|s| b.checked_add(s))
        })
        .ok_or(WorldError::InvalidQty)
}

pub async fn buy_credits(
    pool: &PgPool,
    user_id: Uuid,
    item_id: &str,
    qty: i32,
    params: Params,
) -> Result<Purchase, WorldError> {
    if qty < 1 || qty > params.shop_max_qty {
        return Err(WorldError::InvalidQty);
    }
    let mut tx = pool.begin().await?;
    let item = catalog::get(&mut tx, item_id)
        .await?
        .filter(|i| i.enabled)
        .ok_or(WorldError::InvalidItem)?;
    let price = catalog::price_credits(&mut tx, &item.id)
        .await?
        .ok_or(WorldError::InvalidItem)?;
    if item.exclusive {
        crate::plots::ensure_exclusive_allowed(&mut tx, user_id, &item, params).await?;
    }
    crate::plots::ensure_level_allowed(&mut tx, user_id, &item).await?;
    let total = if item.id == PLOT_ITEM {
        // A plot past the level's limit could never be placed: refuse it instead of selling it.
        let (level, owned) = lock_plots_owned(&mut tx, user_id).await?;
        if owned + i64::from(qty) > crate::placement::plot_limit(level, params) {
            return Err(WorldError::PlotLimit);
        }
        plot_series(owned, price, qty, params)?
    } else {
        price.checked_mul(i64::from(qty)).ok_or(WorldError::InvalidQty)?
    };
    let ref_id = Uuid::new_v4();
    if total > 0 {
        cf_ledger::post(
            &mut tx,
            "shop_buy",
            ref_id,
            &[
                Entry::user(user_id, ACCOUNT_CREDITS, Currency::Credits, -total),
                Entry::system(SYSTEM_SINK, Currency::Credits, total),
            ],
        )
        .await?;
    }
    inventory::add(&mut tx, user_id, &item.id, qty).await?;
    sqlx::query("INSERT INTO cf.events (user_id, name, props) VALUES ($1, 'shop_buy', $2)")
        .bind(user_id)
        .bind(serde_json::json!({ "item": item.id, "qty": qty, "credits": total, "ref_id": ref_id }))
        .execute(&mut *tx)
        .await?;
    let credits_left: i64 =
        sqlx::query_scalar("SELECT amount FROM cf.balances WHERE user_id = $1 AND account = $2")
            .bind(user_id)
            .bind(ACCOUNT_CREDITS)
            .fetch_optional(&mut *tx)
            .await?
            .unwrap_or(0);
    tx.commit().await?;
    Ok(Purchase {
        item_id: item.id,
        qty,
        spent_credits: total,
        credits_left,
    })
}
