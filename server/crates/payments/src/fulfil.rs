//! Fulfilment of a verified order (rules/server-payments.md): one database
//! transaction writes the burn row (UNIQUE signature = no double fulfilment), the
//! double-entry ledger lines, the inventory, reputation, the terminal status and
//! the analytics event. Called by `submit` and by the manual review queue.

use cf_chain::Commitment;
use cf_ledger::{Currency, Entry, SYSTEM_BURN_CLASH};
use cf_world::inventory;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::PaymentsError;
use crate::submit::{Claimed, ACCOUNT_CLASH_PAID, SYSTEM_TREASURY_CLASH};

const REPUTATION_PER_ORDER: i32 = 10;
/// One DB transaction: burns → ledger → inventory → reputation → status (rules/server-payments.md).
pub(crate) async fn fulfil(
    pool: &PgPool,
    user_id: Uuid,
    order_id: Uuid,
    order: &Claimed,
    signature: &str,
    verified: &cf_chain::Verified,
    commitment: Commitment,
) -> Result<(), PaymentsError> {
    let mut tx = pool.begin().await?;
    let status: String = sqlx::query_scalar("SELECT status FROM cf.orders WHERE id = $1 FOR UPDATE")
        .bind(order_id)
        .fetch_one(&mut *tx)
        .await?;
    if status != "submitted" {
        return Err(PaymentsError::WrongStatus);
    }
    let burn =
        i64::try_from(verified.burn).map_err(|_| PaymentsError::NeedsReview("burn overflow".into()))?;
    let transfer = i64::try_from(verified.transfer)
        .map_err(|_| PaymentsError::NeedsReview("transfer overflow".into()))?;
    insert_burn(&mut tx, order_id, signature, verified, commitment, burn, transfer).await?;
    let total = burn
        .checked_add(transfer)
        .ok_or_else(|| PaymentsError::NeedsReview("total overflow".into()))?;
    let mut entries = vec![Entry::user(user_id, ACCOUNT_CLASH_PAID, Currency::Clash, total)];
    if burn > 0 {
        entries.push(Entry::system(SYSTEM_BURN_CLASH, Currency::Clash, -burn));
    }
    if transfer > 0 {
        entries.push(Entry::system(SYSTEM_TREASURY_CLASH, Currency::Clash, -transfer));
    }
    cf_ledger::post(&mut tx, "order", order_id, &entries).await?;
    deliver(&mut tx, user_id, order_id, order, burn, transfer).await?;
    tx.commit().await?;
    tracing::info!(%order_id, burn, transfer, "order_fulfilled");
    Ok(())
}

/// `UNIQUE(signature)` is the last line against double fulfilment: 23505 → `SignatureReused`.
async fn insert_burn(
    tx: &mut sqlx::PgConnection,
    order_id: Uuid,
    signature: &str,
    verified: &cf_chain::Verified,
    commitment: Commitment,
    burn: i64,
    transfer: i64,
) -> Result<(), PaymentsError> {
    let inserted = sqlx::query(
        "INSERT INTO cf.burns (order_id, signature, slot, block_time, amount, treasury_amount, commitment, raw_summary)
         VALUES ($1, $2, $3, to_timestamp($4), $5, $6, $7, $8)",
    )
    .bind(order_id)
    .bind(signature)
    .bind(i64::try_from(verified.slot).unwrap_or(i64::MAX))
    .bind(verified.block_time)
    .bind(burn)
    .bind(transfer)
    .bind(match commitment {
        Commitment::Confirmed => "confirmed",
        Commitment::Finalized => "finalized",
    })
    .bind(serde_json::json!({ "slot": verified.slot, "burn": verified.burn, "transfer": verified.transfer }))
    .execute(&mut *tx)
    .await;
    match inserted {
        Ok(_) => Ok(()),
        Err(sqlx::Error::Database(db)) if db.code().as_deref() == Some("23505") => {
            Err(PaymentsError::SignatureReused)
        }
        Err(err) => Err(err.into()),
    }
}

/// Inventory, reputation, terminal status and the analytics event, inside the caller's transaction.
async fn deliver(
    tx: &mut sqlx::PgConnection,
    user_id: Uuid,
    order_id: Uuid,
    order: &Claimed,
    burn: i64,
    transfer: i64,
) -> Result<(), PaymentsError> {
    // A pack item (items.meta.pack, 0019) delivers that many units per ordered unit.
    let pack: Option<i64> = sqlx::query_scalar("SELECT (meta->>'pack')::bigint FROM cf.items WHERE id = $1")
        .bind(&order.item_id)
        .fetch_optional(&mut *tx)
        .await?
        .flatten();
    let units = i32::try_from(i64::from(order.qty).saturating_mul(pack.unwrap_or(1).clamp(1, 1_000)))
        .unwrap_or(i32::MAX);
    inventory::add(tx, user_id, &order.item_id, units).await?;
    sqlx::query("UPDATE cf.users SET reputation = reputation + $2 WHERE id = $1")
        .bind(user_id)
        .bind(REPUTATION_PER_ORDER)
        .execute(&mut *tx)
        .await?;
    sqlx::query(
        "UPDATE cf.orders SET status = 'fulfilled', reason_code = NULL, updated_at = now() WHERE id = $1",
    )
    .bind(order_id)
    .execute(&mut *tx)
    .await?;
    sqlx::query("INSERT INTO cf.events (user_id, name, props) VALUES ($1, 'order_fulfilled', $2)")
        .bind(user_id)
        .bind(serde_json::json!({ "order_id": order_id, "item": order.item_id, "qty": order.qty, "burn": burn, "transfer": transfer }))
        .execute(&mut *tx)
        .await?;
    Ok(())
}

// `to_timestamp($4)` above receives a bigint; the seconds→timestamptz cast happens in SQL.
