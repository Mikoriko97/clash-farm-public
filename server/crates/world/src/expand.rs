//! Island expansion (ADR-015, migration 0010): the n-th strip on a side costs
//! the tier-n permit bought for CLASH. The permit is an ordinary inventory item,
//! so the payment itself goes through the verified order flow untouched; this
//! module only turns a permit into `islands.expand_<side> + 1` under the island lock.

use serde::Serialize;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::error::WorldError;
use crate::inventory;
use crate::plots::record_event;
use crate::types::IslandView;

/// Sides the API accepts; the column is chosen from this list, never from the request text.
pub const SIDES: [&str; 4] = ["n", "e", "s", "w"];
pub const MAX_PER_SIDE: i16 = 4;

/// Catalog id of the permit needed for expansion number `tier` (1-based).
pub fn permit_item(tier: i16) -> String {
    format!("island_expand_{tier}")
}

#[derive(Debug, Clone, Serialize)]
pub struct ExpandResult {
    pub island: IslandView,
    pub side: String,
    /// Expansions this side now has (1..=4).
    pub level: i16,
    pub permit_used: String,
}

fn column_for(side: &str) -> Option<&'static str> {
    match side {
        "n" => Some("expand_n"),
        "e" => Some("expand_e"),
        "s" => Some("expand_s"),
        "w" => Some("expand_w"),
        _ => None,
    }
}

/// `POST /island/expand { side }`.
pub async fn expand(pool: &PgPool, user_id: Uuid, side: &str) -> Result<ExpandResult, WorldError> {
    let column = column_for(side).ok_or(WorldError::InvalidSide)?;
    let mut tx = pool.begin().await?;
    let row = sqlx::query(
        "SELECT id, level, xp, expand_n, expand_e, expand_s, expand_w FROM cf.islands WHERE user_id = $1 FOR UPDATE",
    )
    .bind(user_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(WorldError::NotFound)?;
    let island_id: Uuid = row.try_get("id")?;
    let current: i16 = row.try_get(column)?;
    if current >= MAX_PER_SIDE {
        return Err(WorldError::ExpandMax);
    }
    let tier = current + 1;
    let permit = permit_item(tier);
    // Money buys the strip; the level to use it is earned by playing (0019).
    if let Some(item) = crate::catalog::get(&mut tx, &permit).await? {
        let level: i32 = row.try_get("level")?;
        let min = item.meta_i64("level_min").unwrap_or(0);
        if i64::from(level) < min {
            return Err(WorldError::LevelLocked(i32::try_from(min).unwrap_or(i32::MAX)));
        }
    }
    inventory::take_one(&mut tx, user_id, &permit).await?;
    // The column name comes from `column_for`, a fixed table; the request only picks the side.
    let sql = match column {
        "expand_n" => "UPDATE cf.islands SET expand_n = expand_n + 1 WHERE id = $1 RETURNING level, xp, expand_n, expand_e, expand_s, expand_w",
        "expand_e" => "UPDATE cf.islands SET expand_e = expand_e + 1 WHERE id = $1 RETURNING level, xp, expand_n, expand_e, expand_s, expand_w",
        "expand_s" => "UPDATE cf.islands SET expand_s = expand_s + 1 WHERE id = $1 RETURNING level, xp, expand_n, expand_e, expand_s, expand_w",
        _ => "UPDATE cf.islands SET expand_w = expand_w + 1 WHERE id = $1 RETURNING level, xp, expand_n, expand_e, expand_s, expand_w",
    };
    let updated = sqlx::query(sql).bind(island_id).fetch_one(&mut *tx).await?;
    let island = IslandView {
        id: island_id,
        level: updated.try_get("level")?,
        xp: updated.try_get("xp")?,
        expand_n: updated.try_get("expand_n")?,
        expand_e: updated.try_get("expand_e")?,
        expand_s: updated.try_get("expand_s")?,
        expand_w: updated.try_get("expand_w")?,
    };
    record_event(
        &mut tx,
        user_id,
        "island_expand",
        serde_json::json!({ "side": side, "tier": tier, "permit": permit }),
    )
    .await?;
    tx.commit().await?;
    Ok(ExpandResult {
        island,
        side: side.to_owned(),
        level: tier,
        permit_used: permit,
    })
}
