//! Recipes (migration 0008): produce and materials become dishes and decor.
//! `craft` takes every input and adds the output in one transaction, all or
//! nothing; `eat` turns a dish into XP. Recipes and dish values come from the
//! database only.

use serde::Serialize;
use sqlx::{PgConnection, PgPool, Row};
use uuid::Uuid;

use crate::error::WorldError;
use crate::params::Params;
use crate::plots::{add_xp, record_event};
use crate::{caps, catalog, inventory};

#[derive(Debug, Clone, Serialize)]
pub struct RecipeInput {
    pub item_id: String,
    pub qty: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct RecipeView {
    pub id: String,
    pub output_item: String,
    pub output_qty: i32,
    pub inputs: Vec<RecipeInput>,
    pub xp: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct CraftResult {
    pub recipe_id: String,
    pub output_item: String,
    pub qty: i32,
    pub xp: i64,
    pub level: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct EatResult {
    pub item_id: String,
    pub xp: i64,
    pub level: i32,
}

fn recipe_from_row(row: &sqlx::postgres::PgRow) -> Result<RecipeView, WorldError> {
    let raw: serde_json::Value = row.try_get("inputs")?;
    let mut inputs = Vec::new();
    if let Some(map) = raw.as_object() {
        for (item_id, qty) in map {
            let qty = qty.as_i64().unwrap_or(0);
            if qty > 0 {
                inputs.push(RecipeInput {
                    item_id: item_id.clone(),
                    qty: i32::try_from(qty).unwrap_or(i32::MAX),
                });
            }
        }
    }
    inputs.sort_by(|a, b| a.item_id.cmp(&b.item_id));
    Ok(RecipeView {
        id: row.try_get("id")?,
        output_item: row.try_get("output_item")?,
        output_qty: row.try_get("output_qty")?,
        inputs,
        xp: row.try_get("xp")?,
    })
}

/// `GET /recipes`: every enabled recipe.
pub async fn list(pool: &PgPool) -> Result<Vec<RecipeView>, WorldError> {
    let rows = sqlx::query(
        "SELECT id, output_item, output_qty, inputs, xp FROM cf.recipes WHERE enabled ORDER BY id",
    )
    .fetch_all(pool)
    .await?;
    rows.iter().map(recipe_from_row).collect()
}

async fn get(conn: &mut PgConnection, id: &str) -> Result<Option<RecipeView>, WorldError> {
    let row = sqlx::query(
        "SELECT id, output_item, output_qty, inputs, xp FROM cf.recipes WHERE id = $1 AND enabled",
    )
    .bind(id)
    .fetch_optional(conn)
    .await?;
    row.as_ref().map(recipe_from_row).transpose()
}

/// `POST /craft { recipe_id, times }`: inputs × times out, output × times in, XP × times.
/// The daily cap counts units: fewer than `times` are made when the day's budget runs low.
pub async fn craft(
    pool: &PgPool,
    user_id: Uuid,
    recipe_id: &str,
    times: i32,
    params: Params,
) -> Result<CraftResult, WorldError> {
    if times < 1 || times > params.shop_max_qty {
        return Err(WorldError::InvalidQty);
    }
    let mut tx = pool.begin().await?;
    let recipe = get(&mut tx, recipe_id).await?.ok_or(WorldError::InvalidItem)?;
    if recipe.inputs.is_empty() {
        return Err(WorldError::InvalidItem);
    }
    let times = caps::consume_up_to(&mut tx, user_id, "craft", times, params.craft_max_per_day).await?;
    if times == 0 {
        return Err(WorldError::DailyCap);
    }
    for input in &recipe.inputs {
        let needed = input.qty.checked_mul(times).ok_or(WorldError::InvalidQty)?;
        inventory::take(&mut tx, user_id, &input.item_id, needed).await?;
    }
    let qty = recipe
        .output_qty
        .checked_mul(times)
        .ok_or(WorldError::InvalidQty)?;
    inventory::add(&mut tx, user_id, &recipe.output_item, qty).await?;
    let island_id: Uuid = sqlx::query_scalar("SELECT id FROM cf.islands WHERE user_id = $1")
        .bind(user_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(WorldError::NotFound)?;
    let (xp, level) = add_xp(
        &mut tx,
        user_id,
        island_id,
        i64::from(recipe.xp) * i64::from(times),
    )
    .await?;
    record_event(
        &mut tx,
        user_id,
        "craft",
        serde_json::json!({ "recipe": recipe.id, "times": times, "output": recipe.output_item, "qty": qty }),
    )
    .await?;
    tx.commit().await?;
    Ok(CraftResult {
        recipe_id: recipe.id,
        output_item: recipe.output_item,
        qty,
        xp,
        level,
    })
}

/// `POST /items/{id}/eat`: one dish → its `meta.xp`.
pub async fn eat(
    pool: &PgPool,
    user_id: Uuid,
    item_id: &str,
    params: Params,
) -> Result<EatResult, WorldError> {
    let mut tx = pool.begin().await?;
    let item = catalog::get(&mut tx, item_id)
        .await?
        .filter(|i| i.enabled && i.kind == "dish")
        .ok_or(WorldError::InvalidItem)?;
    let xp_gain = item
        .meta_i64("xp")
        .filter(|x| *x > 0)
        .ok_or(WorldError::InvalidItem)?;
    caps::consume(&mut tx, user_id, "eat", params.eat_max_per_day).await?;
    inventory::take_one(&mut tx, user_id, &item.id).await?;
    let island_id: Uuid = sqlx::query_scalar("SELECT id FROM cf.islands WHERE user_id = $1")
        .bind(user_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(WorldError::NotFound)?;
    let (xp, level) = add_xp(&mut tx, user_id, island_id, xp_gain).await?;
    record_event(
        &mut tx,
        user_id,
        "eat",
        serde_json::json!({ "item": item.id, "xp": xp_gain }),
    )
    .await?;
    tx.commit().await?;
    Ok(EatResult {
        item_id: item.id,
        xp,
        level,
    })
}
