//! Retention (docs/DATA_MODEL.md): `events` 90 days, `rate_events` 30 days.
//! Deletes in batches so a long-lived table never holds a lock for long.
//! `ledger`, `burns`, `audit_log`, `orders` are kept forever by design, and so
//! are the payment-evidence events (`order_submitted`, `orphan_signature`):
//! they are the only record of the signature a parked order can be approved with.

use sqlx::PgPool;

use super::JobError;

pub const EVENTS_DAYS: i64 = 90;
pub const RATE_EVENTS_DAYS: i64 = 30;
const BATCH: i64 = 5_000;
/// Never purged: docs/SECURITY.md §4 keeps payment evidence forever.
pub const KEEP_EVENTS: [&str; 2] = ["order_submitted", "orphan_signature"];
/// Upper bound per run: the next run continues.
const MAX_BATCHES: usize = 200;

pub async fn run(pool: &PgPool) -> Result<String, JobError> {
    let (events, rate) = purge(pool, EVENTS_DAYS, RATE_EVENTS_DAYS).await?;
    Ok(format!("events={events} rate_events={rate}"))
}

/// Returns rows deleted from (`events`, `rate_events`).
pub async fn purge(pool: &PgPool, events_days: i64, rate_events_days: i64) -> Result<(u64, u64), JobError> {
    let mut events = 0u64;
    for _ in 0..MAX_BATCHES {
        let done = sqlx::query(
            "DELETE FROM cf.events WHERE id IN (
                SELECT id FROM cf.events
                WHERE created_at < now() - ($1::bigint * interval '1 day')
                  AND name <> ALL($3)
                ORDER BY id LIMIT $2)",
        )
        .bind(events_days.max(1))
        .bind(BATCH)
        .bind(KEEP_EVENTS.to_vec())
        .execute(pool)
        .await?
        .rows_affected();
        events += done;
        if done < u64::try_from(BATCH).unwrap_or(u64::MAX) {
            break;
        }
    }
    let rate =
        sqlx::query("DELETE FROM cf.rate_events WHERE day < (now() AT TIME ZONE 'UTC')::date - $1::int")
            .bind(i32::try_from(rate_events_days.max(1)).unwrap_or(i32::MAX))
            .execute(pool)
            .await?
            .rows_affected();
    Ok((events, rate))
}
