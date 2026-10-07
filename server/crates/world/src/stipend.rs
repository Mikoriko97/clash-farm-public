//! Seed stipend (0019, docs/ECONOMY_LAB.md §4.7): an island with no seeds, nothing
//! growing and no credits has no way back into the loop, so once a UTC day it gets a
//! few wheat seeds. Checked when the island snapshot is requested; the grant is an
//! event and an inventory row, never credits.

use sqlx::PgPool;
use uuid::Uuid;

use crate::error::WorldError;
use crate::params::Params;
use crate::plots::record_event;
use crate::{caps, inventory};

pub const ITEM: &str = "seed_wheat";

/// Grants today's stipend when the island is stuck; returns the seeds given (0 = not needed).
pub async fn grant_if_stuck(pool: &PgPool, user_id: Uuid, params: Params) -> Result<i32, WorldError> {
    if params.stipend_wheat <= 0 {
        return Ok(0);
    }
    let mut tx = pool.begin().await?;
    // Broke = cannot afford the cheapest seed at today's shop price.
    let broke_below = crate::catalog::price_credits(&mut tx, ITEM).await?.unwrap_or(5);
    let stuck: bool = sqlx::query_scalar(
        "SELECT NOT EXISTS (SELECT 1 FROM cf.inventory inv JOIN cf.items it ON it.id = inv.item_id
                            WHERE inv.user_id = $1 AND inv.qty > 0 AND it.kind = 'seed')
            AND NOT EXISTS (SELECT 1 FROM cf.plots p JOIN cf.islands i ON i.id = p.island_id
                            WHERE i.user_id = $1 AND p.state <> 'empty')
            AND COALESCE((SELECT amount FROM cf.balances WHERE user_id = $1 AND account = 'credits'), 0) < $2",
    )
    .bind(user_id)
    .bind(broke_below)
    .fetch_one(&mut *tx)
    .await?;
    if !stuck {
        return Ok(0);
    }
    match caps::consume(&mut tx, user_id, "stipend", 1).await {
        Ok(_) => {}
        Err(WorldError::DailyCap) => return Ok(0), // already given today; the player must plant it
        Err(e) => return Err(e),
    }
    inventory::add(&mut tx, user_id, ITEM, params.stipend_wheat).await?;
    record_event(
        &mut tx,
        user_id,
        "stipend",
        serde_json::json!({ "item": ITEM, "qty": params.stipend_wheat }),
    )
    .await?;
    tx.commit().await?;
    Ok(params.stipend_wheat)
}
