//! Cleared wild bushes (migration 0023, owner request 2026-09-30: «кущі немає чим прибирати»).
//! The island's wild flora is scattered by the client from a fixed seed, so the server knows
//! no individual bush. Clearing is a free, tool-less action whose only effect is a remembered
//! spot: every client keeps the wild flora away from it. Nothing in the economy, ledger or
//! inventory reads it — the checks are only what keeps the table small and honest: the spot
//! must lie on the player's own island, sit on a 0.5 m grid (a repeat click is the same row)
//! and the island holds at most `MAX_CLEARINGS` of them.

use sqlx::{PgConnection, PgPool, Row};
use uuid::Uuid;

use crate::decor::inside_island_dm;
use crate::error::WorldError;
use crate::plots::record_event;

/// Grid the spots are rounded to (dm): 0.5 m.
pub const GRID_DM: i64 = 5;
/// Cleared spots one island may hold.
pub const MAX_CLEARINGS: i64 = 400;

/// Rounds a coordinate to the nearest grid line (ties away from zero, like `f64::round`).
pub fn snap(v: i64) -> i64 {
    let half = GRID_DM / 2;
    let r = v.rem_euclid(GRID_DM);
    let base = v - r;
    if r > half {
        base + GRID_DM
    } else {
        base
    }
}

/// The island's cleared spots as `[x_dm, z_dm]`, oldest first.
pub async fn list(conn: &mut PgConnection, island_id: Uuid) -> Result<Vec<[i16; 2]>, WorldError> {
    let rows = sqlx::query(
        "SELECT x_dm, z_dm FROM cf.island_clearings WHERE island_id = $1 ORDER BY cleared_at, x_dm, z_dm",
    )
    .bind(island_id)
    .fetch_all(conn)
    .await?;
    rows.iter()
        .map(|r| Ok([r.try_get("x_dm")?, r.try_get("z_dm")?]))
        .collect()
}

/// `POST /island/clear { x_dm, z_dm }`: remembers the spot (idempotent) and returns the full
/// list. The island row is locked like every other island mutation, so the cap cannot be
/// raced past by parallel requests.
pub async fn clear(pool: &PgPool, user_id: Uuid, x_dm: i16, z_dm: i16) -> Result<Vec<[i16; 2]>, WorldError> {
    let (x, z) = (snap(i64::from(x_dm)), snap(i64::from(z_dm)));
    let mut tx = pool.begin().await?;
    let island = sqlx::query(
        "SELECT id, expand_n, expand_e, expand_s, expand_w FROM cf.islands WHERE user_id = $1 FOR UPDATE",
    )
    .bind(user_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(WorldError::NotFound)?;
    let island_id: Uuid = island.try_get("id")?;
    let expand: [i16; 4] = [
        island.try_get("expand_n")?,
        island.try_get("expand_e")?,
        island.try_get("expand_s")?,
        island.try_get("expand_w")?,
    ];
    if !inside_island_dm(x, z, 0, expand) {
        return Err(WorldError::OutOfBounds);
    }
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM cf.island_clearings WHERE island_id = $1")
        .bind(island_id)
        .fetch_one(&mut *tx)
        .await?;
    let (sx, sz) = (
        i16::try_from(x).map_err(|_| WorldError::OutOfBounds)?,
        i16::try_from(z).map_err(|_| WorldError::OutOfBounds)?,
    );
    let inserted = sqlx::query(
        "INSERT INTO cf.island_clearings (island_id, x_dm, z_dm)
         SELECT $1, $2, $3 WHERE $4 < $5
         ON CONFLICT DO NOTHING",
    )
    .bind(island_id)
    .bind(sx)
    .bind(sz)
    .bind(count)
    .bind(MAX_CLEARINGS)
    .execute(&mut *tx)
    .await?
    .rows_affected();
    if inserted == 0 {
        let known: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM cf.island_clearings WHERE island_id = $1 AND x_dm = $2 AND z_dm = $3)",
        )
        .bind(island_id)
        .bind(sx)
        .bind(sz)
        .fetch_one(&mut *tx)
        .await?;
        if !known {
            return Err(WorldError::ClearingLimit);
        }
    } else {
        record_event(
            &mut tx,
            user_id,
            "island_clear",
            serde_json::json!({ "x_dm": sx, "z_dm": sz }),
        )
        .await?;
    }
    let all = list(&mut tx, island_id).await?;
    tx.commit().await?;
    Ok(all)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spots_snap_to_half_metres() {
        assert_eq!(snap(0), 0);
        assert_eq!(snap(2), 0);
        assert_eq!(snap(3), 5);
        assert_eq!(snap(7), 5);
        assert_eq!(snap(8), 10);
        assert_eq!(snap(-2), 0);
        assert_eq!(snap(-3), -5);
        assert_eq!(snap(-8), -10);
        assert_eq!(snap(237), 235);
        assert_eq!(snap(238), 240);
        for v in -400..=400 {
            assert_eq!(snap(v) % GRID_DM, 0, "{v}");
            assert!((snap(v) - v).abs() <= 2, "{v}");
        }
    }
}
