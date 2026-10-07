//! Gatherable resource nodes (migration 0007): trees give wood, rocks give
//! stone. One `gather` = one hit: owner check through the island, not
//! respawning, daily cap, hit counter, item to the inventory, a little XP.
//! Regrowth is derived from the database clock, never from the client.
//! Since 0017 every node has its own point (dm) and growth start: a used-up node
//! comes back somewhere free (`crate::regrow`, server-chosen), a tree is gatherable
//! only once grown (`stage` 2), rocks are whole at once.

use serde::Serialize;
use sqlx::postgres::PgRow;
use sqlx::{PgConnection, PgPool, Row};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::error::WorldError;
use crate::params::Params;
use crate::plots::{add_xp, record_event};
use crate::{caps, inventory};

pub const KIND_TREE: &str = "tree";
pub const KIND_ROCK: &str = "rock";
pub const ITEM_WOOD: &str = "wood";
pub const ITEM_STONE: &str = "stone";

#[derive(Debug, Clone, Serialize)]
pub struct ResourceView {
    pub idx: i16,
    /// `tree` | `rock`.
    pub kind: String,
    /// Hits until the node is used up; the full count again once it regrew.
    pub hits_left: i32,
    /// Present only while the node is used up: when it comes back (DB clock).
    #[serde(with = "time::serde::rfc3339::option")]
    pub respawn_at: Option<OffsetDateTime>,
    /// Point on the island in dm from the centre (0017); 0,0 before the regrow job placed it.
    pub x_dm: i16,
    pub z_dm: i16,
    /// 0 sapling, 1 young, 2 grown (gatherable); rocks are always 2.
    pub stage: i16,
    /// Which model of the kind the client shows.
    pub variant: i16,
}

/// Footprint radius (dm) a node keeps clear around itself.
pub fn footprint_dm(kind: &str) -> i64 {
    // Crown of a Simple Trees piece ≈ 2 m across, the big mossy stone ≈ 4 m (owner
    // screenshot 2026-09-16: a tree came up inside a rock with the old 18/14).
    if kind == KIND_TREE {
        22
    } else {
        26
    }
}

/// Growth stage from the node's age (seconds) — trees only, rocks are whole at once.
pub fn stage_of(kind: &str, age_secs: i64, params: Params) -> i16 {
    if kind != KIND_TREE {
        return 2;
    }
    let grow = i64::from(params.tree_grow_secs.max(1));
    (age_secs.max(0) / grow).min(2) as i16
}

/// Starter spots of Island.tscn (trees idx 0-2, rocks idx 3-7), dm; same values as migration 0017.
pub const STARTER_SPOTS: [(i16, i16); 8] = [
    (-160, -140),
    (170, 120),
    (150, -160),
    (120, -10),
    (-90, 110),
    (-180, 40),
    (60, 170),
    (-50, -190),
];

#[derive(Debug, Clone, Serialize)]
pub struct GatherResult {
    pub node: ResourceView,
    pub item_id: String,
    pub qty: i32,
    pub xp: i64,
    pub level: i32,
}

/// Hits a fresh node of `kind` takes (settings).
pub fn max_hits(kind: &str, params: Params) -> i32 {
    if kind == KIND_TREE {
        params.tree_hits.max(1)
    } else {
        params.rock_hits.max(1)
    }
}

fn respawn_secs(kind: &str, params: Params) -> i64 {
    i64::from(if kind == KIND_TREE {
        params.tree_respawn_secs
    } else {
        params.rock_respawn_secs
    })
    .max(1)
}

fn item_for(kind: &str) -> &'static str {
    if kind == KIND_TREE {
        ITEM_WOOD
    } else {
        ITEM_STONE
    }
}

/// Expects columns: idx, kind, hits_left, respawn_at, respawning (bool), regrown (bool).
fn view_from_row(row: &PgRow, params: Params) -> Result<ResourceView, sqlx::Error> {
    let kind: String = row.try_get("kind")?;
    let stored: i32 = row.try_get("hits_left")?;
    // A used-up node stays used up until the regrow job has moved it (0017): the
    // client sees it depleted and gather refuses, so nobody harvests a node the job
    // is about to relocate.
    let respawning: bool = row.try_get("respawning")?;
    let hits_left = if respawning {
        0
    } else if stored == 0 {
        max_hits(&kind, params)
    } else {
        stored
    };
    let age: i64 = row.try_get("age_secs")?;
    Ok(ResourceView {
        idx: row.try_get("idx")?,
        respawn_at: if respawning {
            row.try_get("respawn_at")?
        } else {
            None
        },
        x_dm: row.try_get::<Option<i16>, _>("x_dm")?.unwrap_or(0),
        z_dm: row.try_get::<Option<i16>, _>("z_dm")?.unwrap_or(0),
        stage: stage_of(&kind, age, params),
        variant: row.try_get("variant")?,
        kind,
        hits_left,
    })
}

/// Node columns every query selects; a macro keeps the SQL a literal for sqlx's audit.
macro_rules! node_columns {
    () => {
        "r.idx, r.kind, r.hits_left, r.respawn_at, r.x_dm, r.z_dm, r.variant,
         GREATEST(0, EXTRACT(EPOCH FROM (now() - r.grown_at)))::bigint AS age_secs,
         (r.respawn_at IS NOT NULL) AS respawning"
    };
}

/// Starting layout, same order as Island.tscn: trees first, then rocks.
pub async fn seed_starter(
    conn: &mut PgConnection,
    island_id: Uuid,
    params: Params,
) -> Result<(), WorldError> {
    let trees = i32::from(params.starter_trees.max(0));
    let rocks = i32::from(params.starter_rocks.max(0));
    for idx in 0..trees + rocks {
        let kind = if idx < trees { KIND_TREE } else { KIND_ROCK };
        // Starter nodes keep the hand-made spots of Island.tscn and start grown; any
        // extra starter beyond the eight scene spots is placed by the regrow job.
        let spot = STARTER_SPOTS.get(idx as usize).copied();
        sqlx::query(
            "INSERT INTO cf.resource_nodes (island_id, idx, kind, hits_left, x_dm, z_dm, grown_at)
             VALUES ($1, $2, $3, $4, $5, $6, now() - interval '30 days')",
        )
        .bind(island_id)
        .bind(i16::try_from(idx).unwrap_or(i16::MAX))
        .bind(kind)
        .bind(max_hits(kind, params))
        .bind(spot.map(|s| s.0))
        .bind(spot.map(|s| s.1))
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

pub async fn list(
    conn: &mut PgConnection,
    island_id: Uuid,
    params: Params,
) -> Result<Vec<ResourceView>, WorldError> {
    let rows = sqlx::query(concat!(
        "SELECT ",
        node_columns!(),
        " FROM cf.resource_nodes r WHERE r.island_id = $1 ORDER BY r.idx"
    ))
    .bind(island_id)
    .fetch_all(conn)
    .await?;
    Ok(rows
        .iter()
        .map(|r| view_from_row(r, params))
        .collect::<Result<Vec<_>, _>>()?)
}

/// `POST /resources/{idx}/gather`: one hit on the player's node `idx`.
pub async fn gather(
    pool: &PgPool,
    user_id: Uuid,
    idx: i16,
    params: Params,
) -> Result<GatherResult, WorldError> {
    let mut tx = pool.begin().await?;
    let row = sqlx::query(concat!(
        "SELECT r.id, r.island_id, ",
        node_columns!(),
        " FROM cf.resource_nodes r JOIN cf.islands i ON i.id = r.island_id
         WHERE r.idx = $1 AND i.user_id = $2 FOR UPDATE OF r, i"
    ))
    .bind(idx)
    .bind(user_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(WorldError::NotFound)?;
    let node_id: Uuid = row.try_get("id")?;
    let island_id: Uuid = row.try_get("island_id")?;
    let before = view_from_row(&row, params)?;
    if before.respawn_at.is_some() {
        return Err(WorldError::Depleted);
    }
    if before.stage < 2 {
        return Err(WorldError::NotReady);
    }
    caps::consume(&mut tx, user_id, "gather", params.gather_max_per_day).await?;
    let hits_left = before.hits_left - 1;
    sqlx::query(
        "UPDATE cf.resource_nodes SET hits_left = $2,
                respawn_at = CASE WHEN $2 = 0 THEN now() + ($3::bigint * interval '1 second') ELSE NULL END
         WHERE id = $1",
    )
    .bind(node_id)
    .bind(hits_left)
    .bind(respawn_secs(&before.kind, params))
    .execute(&mut *tx)
    .await?;
    let item_id = item_for(&before.kind);
    inventory::add(&mut tx, user_id, item_id, 1).await?;
    let (xp, level) = add_xp(&mut tx, user_id, island_id, i64::from(params.gather_xp.max(0))).await?;
    record_event(
        &mut tx,
        user_id,
        "gather",
        serde_json::json!({ "node": node_id, "idx": idx, "kind": before.kind, "item": item_id }),
    )
    .await?;
    let node = reload(&mut tx, node_id, params).await?;
    tx.commit().await?;
    Ok(GatherResult {
        node,
        item_id: item_id.to_owned(),
        qty: 1,
        xp,
        level,
    })
}

async fn reload(conn: &mut PgConnection, node_id: Uuid, params: Params) -> Result<ResourceView, WorldError> {
    let row = sqlx::query(concat!(
        "SELECT ",
        node_columns!(),
        " FROM cf.resource_nodes r WHERE r.id = $1"
    ))
    .bind(node_id)
    .fetch_one(conn)
    .await?;
    Ok(view_from_row(&row, params)?)
}
