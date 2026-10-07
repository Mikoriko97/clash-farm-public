//! Quotes that outlived `quote_expires_at` + the submit grace (docs/API.md
//! §Payments) become `expired`, so a stale rate can never be honoured later.
//! A `submitted` order means the player signed: it is never expired behind an
//! in-flight verification (which needs `status = submitted` to fulfil). Once it
//! has sat untouched well past any RPC timeout it is parked in `needs_review`
//! with the submitted signature still on record for the operator.

use sqlx::PgPool;

use super::JobError;

/// Same grace as `cf_payments::submit::SUBMIT_GRACE` (120 s).
const GRACE_SECS: i64 = 120;
/// A submission is "stuck" when nothing touched it for this long (RPC path ≤ ~1 min).
const STUCK_SUBMIT_SECS: i64 = 10 * 60;

pub async fn run(pool: &PgPool) -> Result<String, JobError> {
    let expired = expire_quotes(pool).await?;
    let parked = park_stuck_submissions(pool).await?;
    Ok(format!("expired={expired} parked={parked}"))
}

/// `quoted` past the grace → `expired`.
pub async fn expire_quotes(pool: &PgPool) -> Result<u64, JobError> {
    let done = sqlx::query(
        "UPDATE cf.orders SET status = 'expired', updated_at = now()
         WHERE status = 'quoted'
           AND quote_expires_at + ($1::bigint * interval '1 second') < now()",
    )
    .bind(GRACE_SECS)
    .execute(pool)
    .await?;
    Ok(done.rows_affected())
}

/// `submitted` past the grace and untouched for `STUCK_SUBMIT_SECS` → `needs_review`.
pub async fn park_stuck_submissions(pool: &PgPool) -> Result<u64, JobError> {
    let done = sqlx::query(
        "UPDATE cf.orders SET status = 'needs_review', reason_code = 'submit_timeout', updated_at = now()
         WHERE status = 'submitted'
           AND quote_expires_at + ($1::bigint * interval '1 second') < now()
           AND updated_at < now() - ($2::bigint * interval '1 second')",
    )
    .bind(GRACE_SECS)
    .bind(STUCK_SUBMIT_SECS)
    .execute(pool)
    .await?;
    Ok(done.rows_affected())
}
