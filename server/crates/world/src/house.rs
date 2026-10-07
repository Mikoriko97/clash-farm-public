//! The island house (owner backlog 2026-09-05: walk in, sleep, cook). Cooking
//! is the ordinary `/craft`; sleeping is the only house action with an economic
//! effect: a small XP grant once per UTC day (`economy.sleep_max_per_day`,
//! `economy.sleep_xp`). Where the character stands is a client matter and is
//! never trusted here.

use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::caps;
use crate::error::WorldError;
use crate::params::Params;
use crate::plots::{add_xp_plain, record_event};
use crate::types::RestedView;

#[derive(Debug, Clone, Serialize)]
pub struct SleepResult {
    pub xp_gain: i64,
    pub xp: i64,
    pub level: i32,
    /// «Rested» started by this sleep (0026); absent when the status is switched off in settings.
    pub rested: Option<RestedView>,
}

/// `POST /house/sleep`: consumes today's sleep budget, grants the XP (never raised by «Rested») and starts «Rested».
pub async fn sleep(pool: &PgPool, user_id: Uuid, params: Params) -> Result<SleepResult, WorldError> {
    let mut tx = pool.begin().await?;
    let island_id: Uuid = sqlx::query_scalar("SELECT id FROM cf.islands WHERE user_id = $1 FOR UPDATE")
        .bind(user_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(WorldError::NotFound)?;
    caps::consume(&mut tx, user_id, "sleep", params.sleep_max_per_day).await?;
    let xp_gain = i64::from(params.sleep_xp.max(0));
    let (xp, level) = add_xp_plain(&mut tx, user_id, island_id, xp_gain).await?;
    let rested = start_rested(&mut tx, island_id, params).await?;
    record_event(
        &mut tx,
        user_id,
        "sleep",
        serde_json::json!({ "xp": xp_gain, "rested_pct": rested.as_ref().map(|r| r.bonus_pct) }),
    )
    .await?;
    tx.commit().await?;
    Ok(SleepResult {
        xp_gain,
        xp,
        level,
        rested,
    })
}

/// Longest status a setting can ask for (a day) and the largest bonus the column allows.
const MAX_RESTED_MINUTES: i32 = 1_440;
const MAX_RESTED_PCT: i32 = 100;

/// Starts (or restarts, never stacks) «Rested» on the server clock: `rested_minutes` from now, the current
/// `rested_bonus_pct` copied onto the island, the carried remainder reset. `None` when either setting is 0.
async fn start_rested(
    conn: &mut sqlx::PgConnection,
    island_id: Uuid,
    params: Params,
) -> Result<Option<RestedView>, WorldError> {
    let minutes = params.rested_minutes.clamp(0, MAX_RESTED_MINUTES);
    let pct = params.rested_bonus_pct.clamp(0, MAX_RESTED_PCT);
    if minutes == 0 || pct == 0 {
        return Ok(None);
    }
    let bonus_pct = i16::try_from(pct).unwrap_or(0);
    let until: time::OffsetDateTime = sqlx::query_scalar(concat!(
        "UPDATE cf.islands SET rested_until = now() + make_interval(mins => $2), ",
        "rested_bonus_pct = $3, rested_carry = 0 WHERE id = $1 RETURNING rested_until"
    ))
    .bind(island_id)
    .bind(minutes)
    .bind(bonus_pct)
    .fetch_one(&mut *conn)
    .await?;
    Ok(Some(RestedView { until, bonus_pct }))
}
