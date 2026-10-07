//! Daily caps per account via an atomic upsert on `rate_events`
//! (docs/DATA_MODEL.md §4). Runs inside the caller's transaction so a failed
//! action does not consume the cap. Days are UTC (docs/API.md: all times UTC).

use sqlx::PgConnection;
use uuid::Uuid;

use crate::error::WorldError;

/// Consumes one unit of `action` for today; `Err(DailyCap)` once `limit` is reached.
/// A limit of 0 (or less) disables the action entirely.
pub async fn consume(
    conn: &mut PgConnection,
    user_id: Uuid,
    action: &str,
    limit: i32,
) -> Result<i32, WorldError> {
    if limit <= 0 {
        return Err(WorldError::DailyCap);
    }
    let row = sqlx::query_scalar::<_, i32>(
        "INSERT INTO cf.rate_events (user_id, action, day, count)
         VALUES ($1, $2, (now() AT TIME ZONE 'UTC')::date, 1)
         ON CONFLICT (user_id, action, day)
         DO UPDATE SET count = cf.rate_events.count + 1 WHERE cf.rate_events.count < $3
         RETURNING count",
    )
    .bind(user_id)
    .bind(action)
    .bind(limit)
    .fetch_optional(conn)
    .await?;
    row.ok_or(WorldError::DailyCap)
}

/// Returns `amount` of today's budget that a caller granted but could not use
/// (a sale pays whole units only). Same transaction as the grant, so the day's
/// count always equals what was actually paid out.
pub async fn release(
    conn: &mut PgConnection,
    user_id: Uuid,
    action: &str,
    amount: i32,
) -> Result<(), WorldError> {
    if amount <= 0 {
        return Ok(());
    }
    sqlx::query(
        "UPDATE cf.rate_events SET count = GREATEST(count - $3, 0)
         WHERE user_id = $1 AND action = $2 AND day = (now() AT TIME ZONE 'UTC')::date",
    )
    .bind(user_id)
    .bind(action)
    .bind(amount)
    .execute(conn)
    .await?;
    Ok(())
}

/// Grants as much of `wanted` as today's remaining budget allows (0 when exhausted).
/// Used for credits: a harvest past the cap still completes but pays less.
pub async fn consume_up_to(
    conn: &mut PgConnection,
    user_id: Uuid,
    action: &str,
    wanted: i32,
    limit: i32,
) -> Result<i32, WorldError> {
    if wanted <= 0 || limit <= 0 {
        return Ok(0);
    }
    // Create today's row if missing, then read-and-bump it under a row lock so
    // two concurrent grants cannot both see the old count (no reliance on the
    // caller holding another lock).
    sqlx::query(
        "INSERT INTO cf.rate_events (user_id, action, day, count)
         VALUES ($1, $2, (now() AT TIME ZONE 'UTC')::date, 0)
         ON CONFLICT (user_id, action, day) DO NOTHING",
    )
    .bind(user_id)
    .bind(action)
    .execute(&mut *conn)
    .await?;
    let granted = sqlx::query_scalar::<_, i32>(
        "UPDATE cf.rate_events r SET count = LEAST(o.old + $3, GREATEST(o.old, $4))
         FROM (SELECT count AS old FROM cf.rate_events
               WHERE user_id = $1 AND action = $2 AND day = (now() AT TIME ZONE 'UTC')::date
               FOR UPDATE) o
         WHERE r.user_id = $1 AND r.action = $2 AND r.day = (now() AT TIME ZONE 'UTC')::date
         RETURNING r.count - o.old",
    )
    .bind(user_id)
    .bind(action)
    .bind(wanted)
    .bind(limit)
    .fetch_one(conn)
    .await?;
    Ok(granted.clamp(0, wanted))
}
