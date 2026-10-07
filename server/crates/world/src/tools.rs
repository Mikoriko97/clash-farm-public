//! Tools with durability (migration 0013, docs/MAINLAND.md §5 П7). A tool is a
//! stackable item; `tool_wear` counts the hits of the unit in use. The right
//! tool doubles a mainland hit; at `tool_durability` hits one unit breaks and
//! wear resets. Repair takes the materials listed in `items.meta.repair`.

use serde::Serialize;
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use crate::error::WorldError;
use crate::inventory;
use crate::params::Params;
use crate::plots::record_event;

pub const AXE: &str = "tool_axe";
pub const PICKAXE: &str = "tool_pickaxe";
pub const TOOLS: [&str; 2] = [AXE, PICKAXE];

/// Which tool a mainland node kind wants; plants need none.
pub fn tool_for_kind(kind: &str) -> Option<&'static str> {
    if kind == "tree_fallen" {
        None
    } else if kind.starts_with("tree") {
        Some(AXE)
    } else if kind == "rock" || kind.starts_with("ore") || kind == "crystal" {
        Some(PICKAXE)
    } else {
        None
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ToolView {
    pub item_id: String,
    /// Units in the inventory (0 = no tool; the wear row may still exist).
    pub qty: i32,
    pub wear: i32,
    pub durability: i32,
    pub repair: Vec<RepairCost>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RepairCost {
    pub item_id: String,
    pub qty: i32,
}

/// What one hit did to the tool in hand.
#[derive(Debug, Clone, Serialize)]
pub struct ToolUse {
    pub item_id: String,
    pub wear: i32,
    pub durability: i32,
    /// The unit reached its durability on this hit and left the inventory.
    pub broke: bool,
}

fn durability(params: Params) -> i32 {
    params.tool_durability.max(1)
}

/// Repair materials from `items.meta.repair` (`{item_id: qty}`).
async fn repair_cost(conn: &mut PgConnection, item_id: &str) -> Result<Vec<RepairCost>, WorldError> {
    let meta: Option<serde_json::Value> =
        sqlx::query_scalar("SELECT meta FROM cf.items WHERE id = $1 AND kind = 'tool'")
            .bind(item_id)
            .fetch_optional(&mut *conn)
            .await?;
    let meta = meta.ok_or(WorldError::NotFound)?;
    let mut cost: Vec<RepairCost> = meta
        .get("repair")
        .and_then(|r| r.as_object())
        .map(|m| {
            m.iter()
                .filter_map(|(k, v)| {
                    let qty = i32::try_from(v.as_i64()?).ok()?;
                    (qty > 0).then(|| RepairCost {
                        item_id: k.clone(),
                        qty,
                    })
                })
                .collect()
        })
        .unwrap_or_default();
    cost.sort_by(|a, b| a.item_id.cmp(&b.item_id));
    Ok(cost)
}

async fn qty_of(conn: &mut PgConnection, user_id: Uuid, item_id: &str) -> Result<i32, WorldError> {
    let qty: Option<i32> =
        sqlx::query_scalar("SELECT qty FROM cf.inventory WHERE user_id = $1 AND item_id = $2")
            .bind(user_id)
            .bind(item_id)
            .fetch_optional(conn)
            .await?;
    Ok(qty.unwrap_or(0).max(0))
}

async fn wear_of(conn: &mut PgConnection, user_id: Uuid, item_id: &str) -> Result<i32, WorldError> {
    let wear: Option<i32> =
        sqlx::query_scalar("SELECT wear FROM cf.tool_wear WHERE user_id = $1 AND item_id = $2")
            .bind(user_id)
            .bind(item_id)
            .fetch_optional(conn)
            .await?;
    Ok(wear.unwrap_or(0))
}

/// `GET /tools`: both tools, owned or not.
pub async fn list(pool: &PgPool, user_id: Uuid, params: Params) -> Result<Vec<ToolView>, WorldError> {
    let mut conn = pool.acquire().await?;
    let mut out = Vec::with_capacity(TOOLS.len());
    for tool in TOOLS {
        out.push(ToolView {
            item_id: tool.to_owned(),
            qty: qty_of(&mut conn, user_id, tool).await?,
            wear: wear_of(&mut conn, user_id, tool).await?,
            durability: durability(params),
            repair: repair_cost(&mut conn, tool).await?,
        });
    }
    Ok(out)
}

/// One hit with `tool`, inside the caller's transaction: `None` when the player
/// has no such tool. Wear grows; at durability the unit breaks and wear resets.
pub async fn use_tool(
    conn: &mut PgConnection,
    user_id: Uuid,
    tool: &str,
    params: Params,
) -> Result<Option<ToolUse>, WorldError> {
    if qty_of(conn, user_id, tool).await? <= 0 {
        return Ok(None);
    }
    let max = durability(params);
    let wear: i32 = sqlx::query_scalar(
        "INSERT INTO cf.tool_wear (user_id, item_id, wear) VALUES ($1, $2, 1)
         ON CONFLICT (user_id, item_id) DO UPDATE SET wear = cf.tool_wear.wear + 1, updated_at = now()
         RETURNING wear",
    )
    .bind(user_id)
    .bind(tool)
    .fetch_one(&mut *conn)
    .await?;
    if wear < max {
        return Ok(Some(ToolUse {
            item_id: tool.to_owned(),
            wear,
            durability: max,
            broke: false,
        }));
    }
    inventory::take_one(conn, user_id, tool).await?;
    sqlx::query("UPDATE cf.tool_wear SET wear = 0, updated_at = now() WHERE user_id = $1 AND item_id = $2")
        .bind(user_id)
        .bind(tool)
        .execute(&mut *conn)
        .await?;
    record_event(conn, user_id, "tool_broke", serde_json::json!({ "tool": tool })).await?;
    Ok(Some(ToolUse {
        item_id: tool.to_owned(),
        wear: 0,
        durability: max,
        broke: true,
    }))
}

/// `POST /tools/{id}/repair`: materials from `meta.repair` reset the wear.
pub async fn repair(
    pool: &PgPool,
    user_id: Uuid,
    tool: &str,
    params: Params,
) -> Result<ToolView, WorldError> {
    if !TOOLS.contains(&tool) {
        return Err(WorldError::NotFound);
    }
    let mut tx = pool.begin().await?;
    // Serialise on the player's island row like every other inventory action.
    sqlx::query_scalar::<_, Uuid>("SELECT id FROM cf.islands WHERE user_id = $1 FOR UPDATE")
        .bind(user_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(WorldError::NotFound)?;
    if qty_of(&mut tx, user_id, tool).await? <= 0 {
        return Err(WorldError::NotFound);
    }
    if wear_of(&mut tx, user_id, tool).await? <= 0 {
        return Err(WorldError::WrongState);
    }
    let cost = repair_cost(&mut tx, tool).await?;
    // A tool whose price is missing or malformed in the catalog is not repaired for free.
    if cost.is_empty() {
        return Err(WorldError::InvalidItem);
    }
    for part in &cost {
        inventory::take(&mut tx, user_id, &part.item_id, part.qty).await?;
    }
    sqlx::query("UPDATE cf.tool_wear SET wear = 0, updated_at = now() WHERE user_id = $1 AND item_id = $2")
        .bind(user_id)
        .bind(tool)
        .execute(&mut *tx)
        .await?;
    record_event(
        &mut tx,
        user_id,
        "tool_repair",
        serde_json::json!({ "tool": tool }),
    )
    .await?;
    let view = ToolView {
        item_id: tool.to_owned(),
        qty: qty_of(&mut tx, user_id, tool).await?,
        wear: 0,
        durability: durability(params),
        repair: cost,
    };
    tx.commit().await?;
    Ok(view)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tools_match_node_kinds() {
        assert_eq!(tool_for_kind("tree_pine"), Some(AXE));
        assert_eq!(tool_for_kind("tree_oak"), Some(AXE));
        assert_eq!(tool_for_kind("rock"), Some(PICKAXE));
        assert_eq!(tool_for_kind("ore_iron"), Some(PICKAXE));
        assert_eq!(tool_for_kind("crystal"), Some(PICKAXE));
        assert_eq!(tool_for_kind("tree_fallen"), None);
        assert_eq!(tool_for_kind("herb"), None);
        assert_eq!(tool_for_kind("mushroom_rare"), None);
    }
}
