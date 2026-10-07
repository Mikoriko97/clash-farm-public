//! `confirmed` → `finalized` (docs/SECURITY.md §4 p.6, migration 0011). Small
//! payments are fulfilled at `confirmed`; this job asks the RPC later whether the
//! transaction is finalized and writes the verdict to `burn_finality` (burns is
//! append-only). A transaction that never finalizes is an S1 alert: the item was
//! delivered, the ops team decides. Nothing is fulfilled or reversed here.

use cf_chain::Commitment;
use cf_payments::PaymentsRuntime;
use serde_json::json;
use sqlx::{PgPool, Row};
use uuid::Uuid;

use super::{alert, JobError};

/// Wait this long after the confirmed verdict before asking for finality (Solana: ~13 s).
const MIN_AGE_SECS: i64 = 90;
/// Still not finalized after this: alert and stop asking.
const GIVE_UP_SECS: i64 = 30 * 60;
const BATCH: i64 = 50;

#[derive(Debug, Clone)]
struct Pending {
    order_id: Uuid,
    signature: String,
    age_secs: i64,
}

pub async fn run(pool: &PgPool, runtime: Option<&PaymentsRuntime>) -> Result<String, JobError> {
    let Some(runtime) = runtime.filter(|r| !r.fake) else {
        return Ok("skipped (no chain)".to_owned());
    };
    let pending = pending(pool).await?;
    let (mut finalized, mut missing) = (0u32, 0u32);
    for p in pending {
        match runtime
            .chain
            .get_transaction_cross_checked(&p.signature, Commitment::Finalized)
            .await
        {
            Ok(Some(tx)) if tx.meta.as_ref().is_some_and(|m| m.err.is_none()) => {
                record(pool, &p, "finalized", Some(tx.slot)).await?;
                finalized += 1;
            }
            Ok(Some(_)) => {
                // Finalized but failed: a reorg replaced the confirmed transaction.
                record(pool, &p, "missing", None).await?;
                missing += 1;
            }
            Ok(None) if p.age_secs > GIVE_UP_SECS => {
                record(pool, &p, "missing", None).await?;
                missing += 1;
            }
            Ok(None) => {}
            Err(err) if p.age_secs > GIVE_UP_SECS => {
                // The RPC never answered in half an hour: hand it to a human rather than block the queue.
                tracing::error!(order_id = %p.order_id, error = %err, "finality unknown; parked as missing");
                record(pool, &p, "missing", None).await?;
                missing += 1;
            }
            Err(err) => {
                tracing::warn!(order_id = %p.order_id, error = %err, "finality check failed; retry later")
            }
        }
    }
    Ok(format!("finalized={finalized} missing={missing}"))
}

/// Confirmed burns old enough to check and without a verdict yet.
async fn pending(pool: &PgPool) -> Result<Vec<Pending>, JobError> {
    let rows = sqlx::query(
        "SELECT b.order_id, b.signature,
                EXTRACT(EPOCH FROM (now() - b.verified_at))::bigint AS age_secs
         FROM cf.burns b LEFT JOIN cf.burn_finality f ON f.order_id = b.order_id
         WHERE b.commitment = 'confirmed' AND f.order_id IS NULL
           AND b.verified_at < now() - ($1::bigint * interval '1 second')
         ORDER BY b.verified_at LIMIT $2",
    )
    .bind(MIN_AGE_SECS)
    .bind(BATCH)
    .fetch_all(pool)
    .await?;
    rows.iter()
        .map(|r| {
            Ok(Pending {
                order_id: r.try_get("order_id")?,
                signature: r.try_get("signature")?,
                age_secs: r.try_get("age_secs")?,
            })
        })
        .collect()
}

async fn record(pool: &PgPool, p: &Pending, outcome: &str, slot: Option<u64>) -> Result<(), JobError> {
    let inserted = sqlx::query("INSERT INTO cf.burn_finality (order_id, signature, outcome, slot) VALUES ($1, $2, $3, $4) ON CONFLICT (order_id) DO NOTHING")
        .bind(p.order_id)
        .bind(&p.signature)
        .bind(outcome)
        .bind(slot.and_then(|s| i64::try_from(s).ok()))
        .execute(pool)
        .await?
        .rows_affected();
    // A second instance racing on the same order must not raise the alarm twice.
    if outcome == "missing" && inserted == 1 {
        alert(
            pool,
            "finality",
            "burn_not_finalized",
            Some(&p.order_id.to_string()),
            json!({ "signature": p.signature, "age_secs": p.age_secs }),
        )
        .await?;
    }
    Ok(())
}
