//! Hitting a mainland node (migration 0012, docs/MAINLAND.md §5): the node is
//! shared, so its state row is locked `FOR UPDATE`; whoever hits gets the item.
//! Daily caps per account (common and rare), XP, an event. The caller has
//! already checked the character stands next to the node (presence).

use serde::Serialize;
use sqlx::postgres::PgRow;
use sqlx::{PgConnection, PgPool, Row};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::error::WorldError;
use crate::expeditions;
use crate::mainland::{self, ChunksView, NodeSpec};
use crate::params::Params;
use crate::plots::{add_xp, record_event};
use crate::tools::{self, ToolUse};
use crate::{caps, inventory};

pub const CAP_COMMON: &str = "mainland_gather";
pub const CAP_RARE: &str = "mainland_gather_rare";

#[derive(Debug, Clone, Serialize)]
pub struct NodeView {
    pub id: String,
    pub kind: String,
    pub x_dm: i32,
    pub z_dm: i32,
    pub hits_left: i32,
    #[serde(with = "time::serde::rfc3339::option")]
    pub respawn_at: Option<OffsetDateTime>,
}

#[derive(Debug, Clone, Serialize)]
pub struct MainlandGather {
    pub node: NodeView,
    pub item_id: String,
    /// Two with the right tool in hand, one without (docs/MAINLAND.md §5 П7).
    pub qty: i32,
    pub xp: i64,
    pub level: i32,
    /// The tool that took the hit, if the player holds one for this kind.
    pub tool: Option<ToolUse>,
}

/// Live state of one node: stock, when it returns and how many times it was used up
/// (`respawns` picks the spot it stands on, see `NodeSpec::at`, migration 0024).
struct NodeState {
    id: String,
    hits: i32,
    respawn_at: Option<OffsetDateTime>,
    respawns: i32,
}

/// Expects columns: id, hits_left, respawn_at, respawning, regrown, respawns.
fn state_from_row(row: &PgRow, full: i32) -> Result<NodeState, sqlx::Error> {
    let id: String = row.try_get("id")?;
    let stored: i32 = row.try_get("hits_left")?;
    let respawning: bool = row.try_get("respawning")?;
    let regrown: bool = row.try_get("regrown")?;
    let respawns: i32 = row.try_get("respawns")?;
    let (hits, respawn_at) = if respawning {
        (0, row.try_get("respawn_at")?)
    } else if regrown || stored == 0 {
        (full, None)
    } else {
        (stored, None)
    };
    Ok(NodeState {
        id,
        hits,
        respawn_at,
        respawns,
    })
}

macro_rules! state_columns {
    () => {
        "n.id, n.kind, n.hits_left, n.respawn_at, n.respawns,
         (n.respawn_at IS NOT NULL AND n.respawn_at > now()) AS respawning,
         (n.respawn_at IS NOT NULL AND n.respawn_at <= now()) AS regrown"
    };
}

/// Lays the live state (hits, respawn, the spot it stands on) over a generated chunk window.
pub async fn overlay(pool: &PgPool, view: &mut ChunksView) -> Result<(), WorldError> {
    let ids: Vec<String> = view
        .chunks
        .iter()
        .flat_map(|c| c.nodes.iter().map(|n| n.id.clone()))
        .collect();
    if ids.is_empty() {
        return Ok(());
    }
    let rows = sqlx::query(concat!(
        "SELECT ",
        state_columns!(),
        " FROM cf.mainland_nodes n WHERE n.id = ANY($1)"
    ))
    .bind(&ids)
    .fetch_all(pool)
    .await?;
    let mut states = std::collections::HashMap::new();
    for row in &rows {
        let kind: String = row.try_get("kind")?;
        let full = mainland::kind_hits(&kind);
        let s = state_from_row(row, full)?;
        states.insert(s.id, (s.hits, s.respawns));
    }
    for chunk in &mut view.chunks {
        for node in &mut chunk.nodes {
            if let Some(&(hits, respawns)) = states.get(&node.id) {
                node.hits_left = hits;
                (node.x_dm, node.z_dm) = node.at(respawns);
            }
        }
    }
    Ok(())
}

/// Where the node stands right now (its home until it was first used up). The gather route
/// measures the character's reach against this, not against the baked home.
pub async fn position_of(pool: &PgPool, spec: &NodeSpec) -> Result<(i32, i32), WorldError> {
    let respawns: Option<i32> = sqlx::query_scalar("SELECT respawns FROM cf.mainland_nodes WHERE id = $1")
        .bind(&spec.id)
        .fetch_optional(pool)
        .await?;
    Ok(spec.at(respawns.unwrap_or(0)))
}

/// `POST /mainland/nodes/{id}/gather`: one hit on a shared node.
pub async fn gather(
    pool: &PgPool,
    user_id: Uuid,
    spec: &NodeSpec,
    params: Params,
) -> Result<MainlandGather, WorldError> {
    let info = mainland::kind_info(&spec.kind).ok_or(WorldError::NotFound)?;
    let mut tx = pool.begin().await?;
    let island_id: Uuid = sqlx::query_scalar("SELECT id FROM cf.islands WHERE user_id = $1 FOR UPDATE")
        .bind(user_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(WorldError::NotFound)?;
    // The row appears on the first hit; a lost race here just means a no-op insert.
    sqlx::query(
        "INSERT INTO cf.mainland_nodes (id, kind, hits_left) VALUES ($1, $2, $3) ON CONFLICT (id) DO NOTHING",
    )
    .bind(&spec.id)
    .bind(&spec.kind)
    .bind(info.hits)
    .execute(&mut *tx)
    .await?;
    let row = sqlx::query(concat!(
        "SELECT ",
        state_columns!(),
        " FROM cf.mainland_nodes n WHERE n.id = $1 FOR UPDATE"
    ))
    .bind(&spec.id)
    .fetch_one(&mut *tx)
    .await?;
    let state = state_from_row(&row, info.hits)?;
    let before = state.hits;
    if state.respawn_at.is_some() || before <= 0 {
        return Err(WorldError::Depleted);
    }
    caps::consume(&mut tx, user_id, CAP_COMMON, params.mainland_hits_per_day).await?;
    if info.rare {
        caps::consume(&mut tx, user_id, CAP_RARE, params.mainland_rare_hits_per_day).await?;
    }
    let hits_left = before - 1;
    let respawn_secs = i64::from(if info.rare {
        params.mainland_rare_respawn_secs
    } else {
        params.mainland_respawn_secs
    })
    .max(1);
    sqlx::query(
        "UPDATE cf.mainland_nodes SET hits_left = $2, updated_at = now(),
                respawn_at = CASE WHEN $2 = 0 THEN now() + ($3::bigint * interval '1 second') ELSE NULL END,
                respawns = respawns + CASE WHEN $2 = 0 THEN 1 ELSE 0 END
         WHERE id = $1",
    )
    .bind(&spec.id)
    .bind(hits_left)
    .bind(respawn_secs)
    .execute(&mut *tx)
    .await?;
    let tool = match tools::tool_for_kind(&spec.kind) {
        Some(t) => tools::use_tool(&mut tx, user_id, t, params).await?,
        None => None,
    };
    let qty = if tool.is_some() { 2 } else { 1 };
    inventory::add(&mut tx, user_id, info.item, qty).await?;
    let (xp, level) = add_xp(
        &mut tx,
        user_id,
        island_id,
        i64::from(params.mainland_gather_xp.max(0)),
    )
    .await?;
    record_event(
        &mut tx,
        user_id,
        "mainland_gather",
        serde_json::json!({ "node": spec.id, "kind": spec.kind, "item": info.item, "qty": qty, "tool": tool.as_ref().map(|t| &t.item_id) }),
    )
    .await?;
    expeditions::progress(&mut tx, user_id, "gather", &spec.kind, 1).await?;
    let node = reload(&mut tx, spec, info.hits).await?;
    tx.commit().await?;
    Ok(MainlandGather {
        node,
        item_id: info.item.to_owned(),
        qty,
        xp,
        level,
        tool,
    })
}

async fn reload(conn: &mut PgConnection, spec: &NodeSpec, full: i32) -> Result<NodeView, WorldError> {
    let row = sqlx::query(concat!(
        "SELECT ",
        state_columns!(),
        " FROM cf.mainland_nodes n WHERE n.id = $1"
    ))
    .bind(&spec.id)
    .fetch_one(conn)
    .await?;
    let state = state_from_row(&row, full)?;
    let (x_dm, z_dm) = spec.at(state.respawns);
    Ok(NodeView {
        id: spec.id.clone(),
        kind: spec.kind.clone(),
        x_dm,
        z_dm,
        hits_left: state.hits,
        respawn_at: state.respawn_at,
    })
}
