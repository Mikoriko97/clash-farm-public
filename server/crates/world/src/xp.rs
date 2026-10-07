//! XP gains (ECONOMY §3): one place adds XP to the island, applies the «Rested» bonus (0026) and re-derives the level.
//! Other modules reach it through `crate::plots::{add_xp, add_xp_plain}` (re-exported there).

use sqlx::PgConnection;
use uuid::Uuid;

use crate::error::WorldError;
use crate::levels::level_for_xp;
use crate::plots::record_event;

/// Adds XP to the island and re-derives the level; returns `(xp, level)`.
pub(crate) async fn add_xp(
    conn: &mut PgConnection,
    user_id: Uuid,
    island_id: Uuid,
    xp_gain: i64,
) -> Result<(i64, i32), WorldError> {
    add_xp_with(conn, user_id, island_id, xp_gain, true).await
}

/// XP from the house itself (sleep) is never raised by «Rested» (0026), whatever status a previous sleep left.
pub(crate) async fn add_xp_plain(
    conn: &mut PgConnection,
    user_id: Uuid,
    island_id: Uuid,
    xp_gain: i64,
) -> Result<(i64, i32), WorldError> {
    add_xp_with(conn, user_id, island_id, xp_gain, false).await
}

/// One statement adds the gain and, while the island is «Rested» (`rested_until` in the future, server clock),
/// `rested_bonus_pct` percent on top. The percent is applied exactly on average: the hundredths that do not make a
/// whole XP stay in `rested_carry` and ride on the next gain (all integers, no floats).
async fn add_xp_with(
    conn: &mut PgConnection,
    user_id: Uuid,
    island_id: Uuid,
    xp_gain: i64,
    boosted: bool,
) -> Result<(i64, i32), WorldError> {
    let xp: i64 = sqlx::query_scalar(concat!(
        "UPDATE cf.islands SET ",
        "xp = xp + $2 + CASE WHEN $3 AND rested_until > now() ",
        "THEN ($2 * rested_bonus_pct::bigint + rested_carry::bigint) / 100 ELSE 0 END, ",
        "rested_carry = CASE WHEN $3 AND rested_until > now() ",
        "THEN (($2 * rested_bonus_pct::bigint + rested_carry::bigint) % 100)::smallint ELSE rested_carry END ",
        "WHERE id = $1 RETURNING xp"
    ))
    .bind(island_id)
    .bind(xp_gain)
    .bind(boosted)
    .fetch_one(&mut *conn)
    .await?;
    let level = level_for_xp(xp);
    let leveled = sqlx::query("UPDATE cf.islands SET level = $2 WHERE id = $1 AND level <> $2")
        .bind(island_id)
        .bind(level)
        .execute(&mut *conn)
        .await?;
    if leveled.rows_affected() > 0 {
        record_event(conn, user_id, "level_up", serde_json::json!({ "level": level })).await?;
    }
    Ok((xp, level))
}
