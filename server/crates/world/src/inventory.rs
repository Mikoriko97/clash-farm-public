//! Player inventory: quantities per item, never negative (`CHECK (qty >= 0)`).

use sqlx::{PgConnection, PgPool, Row};
use uuid::Uuid;

use crate::error::WorldError;
use crate::types::InventoryEntry;

pub async fn list(pool: &PgPool, user_id: Uuid) -> Result<Vec<InventoryEntry>, WorldError> {
    let rows =
        sqlx::query("SELECT item_id, qty FROM cf.inventory WHERE user_id = $1 AND qty > 0 ORDER BY item_id")
            .bind(user_id)
            .fetch_all(pool)
            .await?;
    rows.iter()
        .map(|r| {
            Ok(InventoryEntry {
                item_id: r.try_get("item_id")?,
                qty: r.try_get("qty")?,
            })
        })
        .collect()
}

pub async fn add(conn: &mut PgConnection, user_id: Uuid, item_id: &str, qty: i32) -> Result<(), WorldError> {
    sqlx::query(
        "INSERT INTO cf.inventory (user_id, item_id, qty) VALUES ($1, $2, $3)
         ON CONFLICT (user_id, item_id) DO UPDATE SET qty = cf.inventory.qty + EXCLUDED.qty",
    )
    .bind(user_id)
    .bind(item_id)
    .bind(qty)
    .execute(conn)
    .await?;
    Ok(())
}

/// Removes one unit; `Err(InsufficientItems)` when the player has none.
pub async fn take_one(conn: &mut PgConnection, user_id: Uuid, item_id: &str) -> Result<(), WorldError> {
    take(conn, user_id, item_id, 1).await
}

/// Removes `qty` units atomically; `Err(InsufficientItems)` when fewer are owned.
pub async fn take(conn: &mut PgConnection, user_id: Uuid, item_id: &str, qty: i32) -> Result<(), WorldError> {
    if qty < 1 {
        return Err(WorldError::InvalidQty);
    }
    let updated = sqlx::query(
        "UPDATE cf.inventory SET qty = qty - $3 WHERE user_id = $1 AND item_id = $2 AND qty >= $3",
    )
    .bind(user_id)
    .bind(item_id)
    .bind(qty)
    .execute(conn)
    .await?;
    if updated.rows_affected() == 0 {
        return Err(WorldError::InsufficientItems);
    }
    Ok(())
}
