//! Manual review of parked orders (week 5 admin). An operator who has checked
//! the player's transaction in an explorer approves a `needs_review` order, or
//! rejects it. The signature is never typed in: only the one the player really
//! submitted (`events.order_submitted`) can be approved, so an operator (or a
//! stolen admin session) cannot invent a payment. Approval reuses the very same
//! fulfilment as the automatic path (status lock, `burns` UNIQUE signature,
//! ledger, delivery) and leaves `reason_code = manual_review` on the order so
//! reconciliation can tell manual payouts from chain-verified ones. Every
//! decision is written to `audit_log` by the caller.

use cf_chain::Commitment;
use serde::Serialize;
use sqlx::{PgPool, Row};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::error::PaymentsError;
use crate::submit::{fulfil, park, Claimed};

pub const NEEDS_REVIEW: &str = "needs_review";
/// Statuses an operator may list.
pub const LISTABLE: [&str; 6] = [
    "needs_review",
    "submitted",
    "fulfilled",
    "rejected",
    "expired",
    "quoted",
];

#[derive(Debug, Clone, Serialize)]
pub struct ReviewOrder {
    pub id: Uuid,
    pub user_id: Uuid,
    pub wallet: String,
    pub item_id: String,
    pub qty: i32,
    pub price_usd_cents: i32,
    pub amount_clash: i64,
    pub status: String,
    pub reason_code: Option<String>,
    /// The signature the order is bound to: the first one submitted (submit.rs refuses any other).
    pub signature: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

/// Newest first; `limit` is clamped to 1..=500.
pub async fn list(pool: &PgPool, status: &str, limit: i64) -> Result<Vec<ReviewOrder>, PaymentsError> {
    if !LISTABLE.contains(&status) {
        return Err(PaymentsError::WrongStatus);
    }
    let rows = sqlx::query(
        "SELECT o.id, o.user_id, coalesce(u.wallet, o.wallet) AS wallet, o.item_id, o.qty, o.price_usd_cents, o.amount_clash,
                o.status, o.reason_code, o.created_at, o.updated_at, s.signature
         FROM cf.orders o JOIN cf.users u ON u.id = o.user_id
         LEFT JOIN LATERAL (
             SELECT e.props->>'signature' AS signature FROM cf.events e
             WHERE e.user_id = o.user_id AND e.name = 'order_submitted' AND e.props->>'order_id' = o.id::text
             ORDER BY e.id LIMIT 1
         ) s ON true
         WHERE o.status = $1 ORDER BY o.updated_at DESC LIMIT $2",
    )
    .bind(status)
    .bind(limit.clamp(1, 500))
    .fetch_all(pool)
    .await?;
    rows.iter()
        .map(|r| {
            Ok(ReviewOrder {
                id: r.try_get("id")?,
                user_id: r.try_get("user_id")?,
                wallet: r.try_get("wallet")?,
                item_id: r.try_get("item_id")?,
                qty: r.try_get("qty")?,
                price_usd_cents: r.try_get("price_usd_cents")?,
                amount_clash: r.try_get("amount_clash")?,
                status: r.try_get("status")?,
                reason_code: r.try_get("reason_code")?,
                signature: r.try_get("signature")?,
                created_at: r.try_get("created_at")?,
                updated_at: r.try_get("updated_at")?,
            })
        })
        .collect()
}

/// Operator-approved payment: `needs_review` → `submitted` → the ordinary
/// fulfilment with the quoted amounts, booked against the signature the player
/// submitted. Returns `(signature, CLASH amount booked)`.
pub async fn approve(pool: &PgPool, order_id: Uuid) -> Result<(String, i64), PaymentsError> {
    let submitted = submitted_signature(pool, order_id)
        .await?
        .ok_or(PaymentsError::BadSignature)?;
    let signature = cf_chain::signature::normalize(&submitted).ok_or(PaymentsError::BadSignature)?;
    let (user_id, order) = take_for_review(pool, order_id, &signature).await?;
    let (burn, transfer) = match (
        u64::try_from(order.burn_amount),
        u64::try_from(order.transfer_amount),
    ) {
        (Ok(b), Ok(t)) => (b, t),
        _ => return Err(PaymentsError::NeedsReview("negative order amounts".to_owned())),
    };
    let verified = cf_chain::Verified {
        burn,
        transfer,
        slot: 0,
        block_time: OffsetDateTime::now_utc().unix_timestamp(),
    };
    match fulfil(
        pool,
        user_id,
        order_id,
        &order,
        &signature,
        &verified,
        Commitment::Finalized,
    )
    .await
    {
        Ok(()) => {
            // `deliver` clears reason_code; keep the manual mark for reconciliation.
            sqlx::query("UPDATE cf.orders SET reason_code = 'manual_review' WHERE id = $1")
                .bind(order_id)
                .execute(pool)
                .await?;
            Ok((signature, order.burn_amount.saturating_add(order.transfer_amount)))
        }
        Err(PaymentsError::SignatureReused) => {
            park(pool, order_id, "rejected", "sig_reused").await?;
            Err(PaymentsError::SignatureReused)
        }
        Err(err) => {
            tracing::error!(%order_id, error = %err, "manual fulfilment failed; parked again");
            park(pool, order_id, NEEDS_REVIEW, "manual_fulfil_error").await?;
            Err(err)
        }
    }
}

/// The signature `order_id` is bound to: the first one the player submitted (submit.rs refuses others).
pub async fn submitted_signature(pool: &PgPool, order_id: Uuid) -> Result<Option<String>, PaymentsError> {
    let user_id: Option<Uuid> = sqlx::query_scalar("SELECT user_id FROM cf.orders WHERE id = $1")
        .bind(order_id)
        .fetch_optional(pool)
        .await?;
    let Some(user_id) = user_id else {
        return Err(PaymentsError::NotFound);
    };
    Ok(sqlx::query_scalar(
        "SELECT props->>'signature' FROM cf.events
         WHERE user_id = $1 AND name = 'order_submitted' AND props->>'order_id' = $2
         ORDER BY id LIMIT 1",
    )
    .bind(user_id)
    .bind(order_id.to_string())
    .fetch_optional(pool)
    .await?
    .flatten())
}

/// Locks the order, requires `needs_review` and an unused signature, marks it `submitted`.
async fn take_for_review(
    pool: &PgPool,
    order_id: Uuid,
    signature: &str,
) -> Result<(Uuid, Claimed), PaymentsError> {
    let mut tx = pool.begin().await?;
    let row = sqlx::query(
        "SELECT o.user_id, o.status, o.item_id, o.qty, o.price_usd_cents, o.burn_amount, o.transfer_amount,
                coalesce(u.wallet, o.wallet) AS wallet, o.created_at, o.quote_expires_at
         FROM cf.orders o JOIN cf.users u ON u.id = o.user_id
         WHERE o.id = $1 FOR UPDATE OF o",
    )
    .bind(order_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(PaymentsError::NotFound)?;
    let status: String = row.try_get("status")?;
    if status != NEEDS_REVIEW {
        return Err(PaymentsError::WrongStatus);
    }
    let used: Option<Uuid> = sqlx::query_scalar("SELECT order_id FROM cf.burns WHERE signature = $1")
        .bind(signature)
        .fetch_optional(&mut *tx)
        .await?;
    if used.is_some() {
        return Err(PaymentsError::SignatureReused);
    }
    sqlx::query(
        "UPDATE cf.orders SET status = 'submitted', reason_code = 'manual_review', updated_at = now() WHERE id = $1",
    )
    .bind(order_id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok((
        row.try_get("user_id")?,
        Claimed {
            item_id: row.try_get("item_id")?,
            qty: row.try_get("qty")?,
            price_usd_cents: row.try_get("price_usd_cents")?,
            burn_amount: row.try_get("burn_amount")?,
            transfer_amount: row.try_get("transfer_amount")?,
            wallet: row.try_get("wallet")?,
            created_at: row.try_get("created_at")?,
            quote_expires_at: row.try_get("quote_expires_at")?,
        },
    ))
}

/// Operator rejection; only a parked order can be rejected by hand.
pub async fn reject(pool: &PgPool, order_id: Uuid, reason: &str) -> Result<(), PaymentsError> {
    let done = sqlx::query(
        "UPDATE cf.orders SET status = 'rejected', reason_code = left($2, 120), updated_at = now()
         WHERE id = $1 AND status = 'needs_review'",
    )
    .bind(order_id)
    .bind(format!("admin:{}", reason.trim()))
    .execute(pool)
    .await?;
    if done.rows_affected() == 0 {
        return Err(PaymentsError::WrongStatus);
    }
    Ok(())
}
