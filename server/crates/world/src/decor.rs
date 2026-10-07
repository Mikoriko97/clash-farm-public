//! Decor placed anywhere on the island (migration 0009): a point in decimetres
//! from the island centre and a yaw in 15° steps. The server owns the rules:
//! inside the island top (with ADR-015 expansions), no overlap with other decor
//! or plots by footprint, at most `decor_slots` pieces. Items move between the
//! inventory and `decor_placements` atomically under the island row lock.

use sqlx::{PgConnection, PgPool, Row};
use uuid::Uuid;

use crate::error::WorldError;
use crate::params::Params;
use crate::placement::{CELL_M, EXPAND_STRIP_M, ISLAND_HALF_M};
use crate::types::DecorView;
use crate::{catalog, inventory};

/// Rotation granularity (degrees).
pub const YAW_STEP: i16 = 15;
/// Footprint radius when the catalog does not say (dm), and the range any catalog value is clamped to.
const DEFAULT_FOOTPRINT_DM: i64 = 10;
const FOOTPRINT_RANGE_DM: std::ops::RangeInclusive<i64> = 1..=100;
/// Plots are squares of half this size (dm); decor may not sit on them.
const PLOT_HALF_DM: i64 = (CELL_M as i64) * 5;
/// Walkable rim kept clear at the island edge (dm).
const EDGE_MARGIN_DM: i64 = 8;

struct IslandLock {
    id: Uuid,
    expand: [i16; 4], // n, e, s, w
}

async fn lock_island(conn: &mut PgConnection, user_id: Uuid) -> Result<IslandLock, WorldError> {
    let row = sqlx::query(
        "SELECT id, expand_n, expand_e, expand_s, expand_w FROM cf.islands WHERE user_id = $1 FOR UPDATE",
    )
    .bind(user_id)
    .fetch_optional(conn)
    .await?
    .ok_or(WorldError::NotFound)?;
    Ok(IslandLock {
        id: row.try_get("id")?,
        expand: [
            row.try_get("expand_n")?,
            row.try_get("expand_e")?,
            row.try_get("expand_s")?,
            row.try_get("expand_w")?,
        ],
    })
}

/// Whether a piece of radius `r_dm` centred on (x, z) dm stays on the island top.
pub fn inside_island_dm(x: i64, z: i64, r_dm: i64, expand: [i16; 4]) -> bool {
    let [n, e, s, w] = expand.map(|x| i64::from(x) * i64::from(EXPAND_STRIP_M) * 10);
    let half = i64::from(ISLAND_HALF_M) * 10;
    let r = r_dm + EDGE_MARGIN_DM;
    x - r >= -(half + w) && x + r <= half + e && z - r >= -(half + n) && z + r <= half + s
}

fn overlaps_circle(x: i64, z: i64, r: i64, ox: i64, oz: i64, or: i64) -> bool {
    let dx = x - ox;
    let dz = z - oz;
    let reach = r + or;
    dx * dx + dz * dz < reach * reach
}

fn overlaps_plot(x: i64, z: i64, r: i64, col: i64, row: i64) -> bool {
    let cx = col * PLOT_HALF_DM * 2;
    let cz = row * PLOT_HALF_DM * 2;
    (x - cx).abs() < PLOT_HALF_DM + r && (z - cz).abs() < PLOT_HALF_DM + r
}

fn clamp_footprint(raw: Option<i64>) -> i64 {
    raw.unwrap_or(DEFAULT_FOOTPRINT_DM)
        .clamp(*FOOTPRINT_RANGE_DM.start(), *FOOTPRINT_RANGE_DM.end())
}

/// `Overlap` when a piece of radius `r` at (x, z) touches other decor or a plot of the island.
async fn ensure_clear(
    conn: &mut PgConnection,
    island_id: Uuid,
    x: i64,
    z: i64,
    r: i64,
) -> Result<(), WorldError> {
    let others = sqlx::query(
        "SELECT p.x_dm, p.z_dm, (i.meta->>'footprint_dm')::bigint AS footprint
         FROM cf.decor_placements p JOIN cf.items i ON i.id = p.item_id WHERE p.island_id = $1",
    )
    .bind(island_id)
    .fetch_all(&mut *conn)
    .await?;
    for other in &others {
        let ox: i16 = other.try_get("x_dm")?;
        let oz: i16 = other.try_get("z_dm")?;
        let or = clamp_footprint(other.try_get::<Option<i64>, _>("footprint")?);
        if overlaps_circle(x, z, r, i64::from(ox), i64::from(oz), or) {
            return Err(WorldError::Overlap);
        }
    }
    let plots = sqlx::query("SELECT col, row FROM cf.plots WHERE island_id = $1")
        .bind(island_id)
        .fetch_all(conn)
        .await?;
    for plot in &plots {
        let col: i16 = plot.try_get("col")?;
        let row: i16 = plot.try_get("row")?;
        if overlaps_plot(x, z, r, i64::from(col), i64::from(row)) {
            return Err(WorldError::Overlap);
        }
    }
    Ok(())
}

/// `POST /decor/place { item_id, x_dm, z_dm, yaw_deg }`.
pub async fn place(
    pool: &PgPool,
    user_id: Uuid,
    item_id: &str,
    x_dm: i16,
    z_dm: i16,
    yaw_deg: i16,
    params: Params,
) -> Result<DecorView, WorldError> {
    if !(0..360).contains(&yaw_deg) || yaw_deg % YAW_STEP != 0 {
        return Err(WorldError::InvalidRotation);
    }
    let mut tx = pool.begin().await?;
    let island = lock_island(&mut tx, user_id).await?;
    let item = catalog::get(&mut tx, item_id)
        .await?
        .filter(|i| i.enabled && i.kind == "decor")
        .ok_or(WorldError::InvalidItem)?;
    let radius = clamp_footprint(item.meta_i64("footprint_dm"));
    let (x, z) = (i64::from(x_dm), i64::from(z_dm));
    if !inside_island_dm(x, z, radius, island.expand) {
        return Err(WorldError::OutOfBounds);
    }
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM cf.decor_placements WHERE island_id = $1")
        .bind(island.id)
        .fetch_one(&mut *tx)
        .await?;
    if count >= i64::from(params.decor_slots) {
        return Err(WorldError::DecorLimit);
    }
    ensure_clear(&mut tx, island.id, x, z, radius).await?;
    inventory::take_one(&mut tx, user_id, &item.id).await?;
    let id: Uuid = sqlx::query_scalar(
        "INSERT INTO cf.decor_placements (island_id, item_id, x_dm, z_dm, yaw_deg) VALUES ($1, $2, $3, $4, $5) RETURNING id",
    )
    .bind(island.id)
    .bind(&item.id)
    .bind(x_dm)
    .bind(z_dm)
    .bind(yaw_deg)
    .fetch_one(&mut *tx)
    .await?;
    sqlx::query("INSERT INTO cf.events (user_id, name, props) VALUES ($1, 'decor_placed', $2)")
        .bind(user_id)
        .bind(
            serde_json::json!({ "item": item.id, "id": id, "x_dm": x_dm, "z_dm": z_dm, "yaw_deg": yaw_deg }),
        )
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(DecorView {
        id,
        item_id: item.id,
        x_dm,
        z_dm,
        yaw_deg,
    })
}

/// Returns the item to the inventory; a foreign or unknown id is `NotFound`.
pub async fn remove(pool: &PgPool, user_id: Uuid, id: Uuid) -> Result<DecorView, WorldError> {
    let mut tx = pool.begin().await?;
    let island = lock_island(&mut tx, user_id).await?;
    let row = sqlx::query(
        "DELETE FROM cf.decor_placements WHERE id = $1 AND island_id = $2 RETURNING item_id, x_dm, z_dm, yaw_deg",
    )
    .bind(id)
    .bind(island.id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(WorldError::NotFound)?;
    let view = DecorView {
        id,
        item_id: row.try_get("item_id")?,
        x_dm: row.try_get("x_dm")?,
        z_dm: row.try_get("z_dm")?,
        yaw_deg: row.try_get("yaw_deg")?,
    };
    inventory::add(&mut tx, user_id, &view.item_id, 1).await?;
    sqlx::query("INSERT INTO cf.events (user_id, name, props) VALUES ($1, 'decor_removed', $2)")
        .bind(user_id)
        .bind(serde_json::json!({ "item": view.item_id, "id": id }))
        .execute(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(view)
}

pub async fn list(conn: &mut PgConnection, island_id: Uuid) -> Result<Vec<DecorView>, WorldError> {
    let rows = sqlx::query(
        "SELECT id, item_id, x_dm, z_dm, yaw_deg FROM cf.decor_placements WHERE island_id = $1 ORDER BY placed_at, id",
    )
    .bind(island_id)
    .fetch_all(conn)
    .await?;
    rows.iter()
        .map(|r| {
            Ok(DecorView {
                id: r.try_get("id")?,
                item_id: r.try_get("item_id")?,
                x_dm: r.try_get("x_dm")?,
                z_dm: r.try_get("z_dm")?,
                yaw_deg: r.try_get("yaw_deg")?,
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn footprints_are_clamped() {
        assert_eq!(clamp_footprint(None), 10);
        assert_eq!(clamp_footprint(Some(0)), 1);
        assert_eq!(clamp_footprint(Some(100_000)), 100);
    }

    #[test]
    fn bounds_and_overlaps() {
        assert!(inside_island_dm(0, 0, 10, [0, 0, 0, 0]));
        assert!(inside_island_dm(220, 0, 10, [0, 0, 0, 0]));
        assert!(!inside_island_dm(225, 0, 10, [0, 0, 0, 0]));
        assert!(inside_island_dm(300, 0, 10, [0, 1, 0, 0]), "one paid strip east");
        assert!(overlaps_circle(0, 0, 10, 15, 0, 10));
        assert!(!overlaps_circle(0, 0, 10, 20, 0, 10));
        assert!(overlaps_plot(20, 0, 10, 1, 0), "plot cell 1 spans 15..45 dm");
        assert!(!overlaps_plot(0, 0, 10, 1, 0));
    }
}
