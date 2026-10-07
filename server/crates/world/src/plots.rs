//! Plot actions. Each one: begin tx → lock the plot through the owner join
//! (`FOR UPDATE`, foreign plot = NotFound) → daily cap → inventory → state
//! change with database time → commit. Nothing numeric comes from the client.
//! The `*_in` variants run inside a caller's transaction so `beds` can apply
//! one action to every plot of a bed atomically.

use sqlx::postgres::PgRow;
use sqlx::{PgConnection, PgPool, Row};
use uuid::Uuid;

use crate::error::WorldError;
use crate::params::Params;
use crate::types::{HarvestResult, PlotView};
use crate::{caps, catalog, inventory};

pub(crate) use crate::xp::{add_xp, add_xp_plain};

const BASIS_POINTS: i64 = 10_000;

pub(crate) struct Locked {
    pub island_id: Uuid,
    pub plot: PlotView,
}

pub(crate) async fn lock_plot(
    conn: &mut PgConnection,
    user_id: Uuid,
    plot_id: Uuid,
) -> Result<Locked, WorldError> {
    // The island row first, then the plot: the order every bed action takes them in
    // (placement::lock_bed), and the order `add_xp` needs later (it updates the island). Plot
    // first here and island first there deadlocked a single harvest against a bed action.
    sqlx::query("SELECT 1 FROM cf.islands WHERE user_id = $1 FOR UPDATE")
        .bind(user_id)
        .fetch_optional(&mut *conn)
        .await?;
    let row = sqlx::query(concat!(
        "SELECT ",
        plot_columns!(),
        ", p.island_id FROM cf.plots p JOIN cf.islands i ON i.id = p.island_id
         WHERE p.id = $1 AND i.user_id = $2 FOR UPDATE OF p"
    ))
    .bind(plot_id)
    .bind(user_id)
    .fetch_optional(conn)
    .await?
    .ok_or(WorldError::NotFound)?;
    Ok(Locked {
        island_id: row.try_get("island_id")?,
        plot: PlotView::from_row(&row)?,
    })
}

pub(crate) async fn reload(conn: &mut PgConnection, plot_id: Uuid) -> Result<PlotView, WorldError> {
    let row: PgRow = sqlx::query(concat!(
        "SELECT ",
        plot_columns!(),
        " FROM cf.plots p WHERE p.id = $1"
    ))
    .bind(plot_id)
    .fetch_one(conn)
    .await?;
    Ok(PlotView::from_row(&row)?)
}

/// Seed from inventory → `growing`, `ready_at = now() + grow_secs` (DB clock).
pub async fn plant(
    pool: &PgPool,
    user_id: Uuid,
    plot_id: Uuid,
    item_id: &str,
    params: Params,
) -> Result<PlotView, WorldError> {
    let mut tx = pool.begin().await?;
    let locked = lock_plot(&mut tx, user_id, plot_id).await?;
    let seed = seed_item(&mut tx, user_id, item_id, params).await?;
    let plot = plant_in(&mut tx, user_id, &locked, &seed, params).await?;
    tx.commit().await?;
    Ok(plot)
}

/// Seed item with its grow time, validated once per request (bed = many plots).
pub(crate) struct Seed {
    pub id: String,
    pub grow_secs: i64,
}

pub(crate) async fn seed_item(
    conn: &mut PgConnection,
    user_id: Uuid,
    item_id: &str,
    params: Params,
) -> Result<Seed, WorldError> {
    let item = catalog::get(conn, item_id)
        .await?
        .filter(|i| i.enabled && i.kind == "seed")
        .ok_or(WorldError::InvalidItem)?;
    let grow_secs = item
        .meta_i64("grow_secs")
        .filter(|s| *s > 0)
        .ok_or(WorldError::InvalidItem)?;
    ensure_exclusive_allowed(conn, user_id, &item, params).await?;
    ensure_level_allowed(conn, user_id, &item).await?;
    Ok(Seed {
        id: item.id,
        grow_secs,
    })
}

/// The tutorial radish (45 s) is outside the plant/harvest daily caps: a newcomer would burn the
/// whole day's cap in minutes. Its income is still bounded by the market's daily credits cap.
fn cap_free(seed_id: &str) -> bool {
    seed_id == crate::island::STARTER_SEED_TUTORIAL
}

pub(crate) async fn plant_in(
    conn: &mut PgConnection,
    user_id: Uuid,
    locked: &Locked,
    seed: &Seed,
    params: Params,
) -> Result<PlotView, WorldError> {
    if locked.plot.state != "empty" {
        return Err(WorldError::WrongState);
    }
    if !cap_free(&seed.id) {
        caps::consume(conn, user_id, "plant", params.plant_max_per_day).await?;
    }
    inventory::take_one(conn, user_id, &seed.id).await?;
    sqlx::query(
        "UPDATE cf.plots SET state = 'growing', crop_id = $2, planted_at = now(),
                ready_at = now() + ($3::bigint * interval '1 second'), fertilizer_id = NULL
         WHERE id = $1",
    )
    .bind(locked.plot.id)
    .bind(&seed.id)
    .bind(seed.grow_secs)
    .execute(&mut *conn)
    .await?;
    if params.plant_xp > 0 {
        add_xp(conn, user_id, locked.island_id, i64::from(params.plant_xp)).await?;
    }
    record_event(
        conn,
        user_id,
        "plant",
        serde_json::json!({ "plot": locked.plot.id, "item": seed.id }),
    )
    .await?;
    reload(conn, locked.plot.id).await
}

/// Fertilizer from inventory removes `speedup_bp` of the remaining time; once per crop.
pub async fn fertilize(
    pool: &PgPool,
    user_id: Uuid,
    plot_id: Uuid,
    item_id: &str,
    params: Params,
) -> Result<PlotView, WorldError> {
    let mut tx = pool.begin().await?;
    let locked = lock_plot(&mut tx, user_id, plot_id).await?;
    let fert = fertilizer_item(&mut tx, user_id, item_id, params).await?;
    let plot = fertilize_in(&mut tx, user_id, &locked, &fert, params).await?;
    tx.commit().await?;
    Ok(plot)
}

pub(crate) struct Fertilizer {
    pub id: String,
    /// Share of the remaining time that stays, in basis points.
    pub remaining_bp: i32,
    /// `meta.premium`: the paid fertilizer with its own daily cap (0019).
    pub premium: bool,
}

pub(crate) async fn fertilizer_item(
    conn: &mut PgConnection,
    user_id: Uuid,
    item_id: &str,
    params: Params,
) -> Result<Fertilizer, WorldError> {
    let item = catalog::get(conn, item_id)
        .await?
        .filter(|i| i.enabled && i.kind == "fertilizer")
        .ok_or(WorldError::InvalidItem)?;
    let speedup = item
        .meta_i64("speedup_bp")
        .filter(|bp| (1..=BASIS_POINTS).contains(bp))
        .ok_or(WorldError::InvalidItem)?;
    ensure_exclusive_allowed(conn, user_id, &item, params).await?;
    let remaining_bp = i32::try_from(BASIS_POINTS - speedup).unwrap_or(BASIS_POINTS as i32);
    Ok(Fertilizer {
        id: item.id,
        remaining_bp,
        premium: item
            .meta
            .get("premium")
            .and_then(serde_json::Value::as_bool)
            .unwrap_or(false),
    })
}

pub(crate) async fn fertilize_in(
    conn: &mut PgConnection,
    user_id: Uuid,
    locked: &Locked,
    fert: &Fertilizer,
    params: Params,
) -> Result<PlotView, WorldError> {
    if locked.plot.state != "growing" {
        return Err(WorldError::WrongState);
    }
    if locked.plot.fertilizer_id.is_some() {
        return Err(WorldError::AlreadyFertilized);
    }
    // Fertilizing shares the planting cap: it is the same "action per day" budget.
    caps::consume(conn, user_id, "fertilize", params.plant_max_per_day).await?;
    // The paid one is a convenience, not a ceiling: its own cap keeps the XP gain bounded (0019).
    if fert.premium {
        caps::consume(conn, user_id, "fertilize_premium", params.premium_fert_per_day).await?;
    }
    inventory::take_one(conn, user_id, &fert.id).await?;
    sqlx::query(
        "UPDATE cf.plots SET ready_at = now() + ((ready_at - now()) * $2) / 10000, fertilizer_id = $3 WHERE id = $1",
    )
    .bind(locked.plot.id)
    .bind(fert.remaining_bp)
    .bind(&fert.id)
    .execute(&mut *conn)
    .await?;
    record_event(
        conn,
        user_id,
        "fertilize",
        serde_json::json!({ "plot": locked.plot.id, "item": fert.id }),
    )
    .await?;
    reload(conn, locked.plot.id).await
}

/// Ready crop → `empty`; XP to the island; credits booked from 0003 on.
pub async fn harvest(
    pool: &PgPool,
    user_id: Uuid,
    plot_id: Uuid,
    params: Params,
) -> Result<HarvestResult, WorldError> {
    let mut tx = pool.begin().await?;
    let locked = lock_plot(&mut tx, user_id, plot_id).await?;
    let result = harvest_in(&mut tx, user_id, &locked, params).await?;
    tx.commit().await?;
    Ok(result)
}

pub(crate) async fn harvest_in(
    conn: &mut PgConnection,
    user_id: Uuid,
    locked: &Locked,
    params: Params,
) -> Result<HarvestResult, WorldError> {
    match locked.plot.state.as_str() {
        "ready" => {}
        "growing" => return Err(WorldError::NotReady),
        _ => return Err(WorldError::WrongState),
    }
    let plot_id = locked.plot.id;
    let crop_id = locked.plot.crop_id.clone().ok_or(WorldError::WrongState)?;
    let crop = catalog::get(conn, &crop_id)
        .await?
        .ok_or(WorldError::InvalidItem)?;
    // XP keeps following the crop's credit value; the credits themselves come from selling (0008).
    let crop_credits = crop.meta_i64("credits").unwrap_or(0).max(0);
    // Seeds name their produce; a seed added without it still yields `crop_<crop>`.
    let produce = crop
        .meta_str("produce")
        .filter(|id| !id.is_empty())
        .or_else(|| crop.meta_str("crop").map(|c| format!("crop_{c}")))
        .ok_or(WorldError::InvalidItem)?;
    if !cap_free(&crop_id) {
        caps::consume(conn, user_id, "harvest", params.harvest_max_per_day).await?;
    }
    sqlx::query(
        "UPDATE cf.plots SET state = 'empty', crop_id = NULL, planted_at = NULL, ready_at = NULL, fertilizer_id = NULL WHERE id = $1",
    )
    .bind(plot_id)
    .execute(&mut *conn)
    .await?;
    inventory::add(conn, user_id, &produce, 1).await?;
    let (xp, level) = add_xp(conn, user_id, locked.island_id, crop_credits / 10).await?;
    record_event(
        conn,
        user_id,
        "harvest",
        serde_json::json!({ "plot": plot_id, "crop": crop_id, "item": produce, "qty": 1 }),
    )
    .await?;
    let plot = reload(conn, plot_id).await?;
    Ok(HarvestResult {
        plot,
        xp,
        level,
        item_id: produce,
        qty: 1,
    })
}

/// Exclusive items (ECONOMY §3) need `users.reputation >= exclusive_min_reputation`.
pub(crate) async fn ensure_exclusive_allowed(
    conn: &mut PgConnection,
    user_id: Uuid,
    item: &catalog::Item,
    params: Params,
) -> Result<(), WorldError> {
    if !item.exclusive {
        return Ok(());
    }
    // ECONOMY §3: reputation >= threshold OR at least one fulfilled CLASH order.
    let allowed: bool = sqlx::query_scalar(
        "SELECT (u.reputation >= $2) OR EXISTS (SELECT 1 FROM cf.orders o WHERE o.user_id = u.id AND o.status = 'fulfilled')
         FROM cf.users u WHERE u.id = $1",
    )
    .bind(user_id)
    .bind(params.exclusive_min_reputation)
    .fetch_optional(conn)
    .await?
    .ok_or(WorldError::NotFound)?;
    if !allowed {
        return Err(WorldError::ExclusiveLocked);
    }
    Ok(())
}

/// `items.meta.level_min` (0019): the island must have reached it.
pub(crate) async fn ensure_level_allowed(
    conn: &mut PgConnection,
    user_id: Uuid,
    item: &catalog::Item,
) -> Result<(), WorldError> {
    let Some(min) = item.meta_i64("level_min").filter(|m| *m > 1) else {
        return Ok(());
    };
    let min = i32::try_from(min).unwrap_or(i32::MAX);
    if island_level(conn, user_id).await? < min {
        return Err(WorldError::LevelLocked(min));
    }
    Ok(())
}

/// Level of the user's island (1 when it does not exist yet).
pub(crate) async fn island_level(conn: &mut PgConnection, user_id: Uuid) -> Result<i32, WorldError> {
    let level: Option<i32> = sqlx::query_scalar("SELECT level FROM cf.islands WHERE user_id = $1")
        .bind(user_id)
        .fetch_optional(conn)
        .await?;
    Ok(level.unwrap_or(1))
}

pub(crate) async fn record_event(
    conn: &mut PgConnection,
    user_id: Uuid,
    name: &str,
    props: serde_json::Value,
) -> Result<(), WorldError> {
    sqlx::query("INSERT INTO cf.events (user_id, name, props) VALUES ($1, $2, $3)")
        .bind(user_id)
        .bind(name)
        .bind(props)
        .execute(conn)
        .await?;
    Ok(())
}
