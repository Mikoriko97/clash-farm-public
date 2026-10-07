//! `POST /orders/{id}/submit`: claim the order, read the transaction from
//! chain, verify it (cf-chain), then fulfil burns + ledger + inventory + status
//! in one transaction. Anything unexpected parks the order in `needs_review`.

use cf_chain::{Commitment, Expectation};
use cf_world::Params;
use serde::Serialize;
use sqlx::{PgPool, Row};
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

use crate::error::PaymentsError;
pub(crate) use crate::fulfil::fulfil;
use crate::orders::memo_for;
use crate::PaymentsRuntime;

pub const SYSTEM_TREASURY_CLASH: &str = "system:treasury_clash";
pub const ACCOUNT_CLASH_PAID: &str = "clash_paid";
/// Grace after quote expiry for a transaction that was signed in time (docs/API.md).
const SUBMIT_GRACE: Duration = Duration::seconds(120);
const BLOCK_TIME_SLACK: Duration = Duration::seconds(60);

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum SubmitOutcome {
    Fulfilled { order_id: Uuid, amount_clash: i64 },
    Submitted { order_id: Uuid },
    Rejected { order_id: Uuid, reason_code: String },
    NeedsReview { order_id: Uuid },
}

pub(crate) struct Claimed {
    pub(crate) item_id: String,
    pub(crate) qty: i32,
    pub(crate) price_usd_cents: i32,
    pub(crate) burn_amount: i64,
    pub(crate) transfer_amount: i64,
    pub(crate) wallet: String,
    pub(crate) created_at: OffsetDateTime,
    pub(crate) quote_expires_at: OffsetDateTime,
}

pub async fn submit(
    pool: &PgPool,
    runtime: &PaymentsRuntime,
    user_id: Uuid,
    order_id: Uuid,
    signature: &str,
    params: Params,
) -> Result<SubmitOutcome, PaymentsError> {
    let signature = cf_chain::signature::normalize(signature).ok_or(PaymentsError::BadSignature)?;
    let order = match claim(pool, user_id, order_id, &signature).await {
        Ok(Some(order)) => order,
        // Already parked for an operator with this very signature: nothing changes.
        Ok(None) => return Ok(SubmitOutcome::NeedsReview { order_id }),
        // A client retry after its own fulfilment (same order, same signature) is still "fulfilled".
        Err(PaymentsError::WrongStatus) => {
            return match fulfilled_amount(pool, order_id, &signature).await? {
                Some(amount_clash) => Ok(SubmitOutcome::Fulfilled {
                    order_id,
                    amount_clash,
                }),
                None => Err(PaymentsError::WrongStatus),
            };
        }
        Err(err) => return Err(err),
    };

    let commitment = if i64::from(order.price_usd_cents) <= i64::from(params.pay_confirmed_max_usd_cents) {
        Commitment::Confirmed
    } else {
        Commitment::Finalized
    };
    if runtime.fake {
        // Local test mode: the quoted amounts count as paid; the ledger, split and burns
        // bookkeeping below run unchanged so the rest of the flow is exercised for real.
        tracing::warn!(%order_id, "FAKE payments: fulfilling without chain verification");
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
            block_time: time::OffsetDateTime::now_utc().unix_timestamp(),
        };
        let result = fulfil(pool, user_id, order_id, &order, &signature, &verified, commitment).await;
        return finish(pool, user_id, order_id, &order, &signature, result).await;
    }
    let fetched = match fetch_tx(pool, runtime, order_id, &signature, commitment).await? {
        Ok(tx) => tx,
        Err(outcome) => return Ok(outcome),
    };
    let verified = match cf_chain::verify(&fetched, &expectation_for(runtime, order_id, &order)) {
        Ok(v) => v,
        Err(cf_chain::RejectReason::Malformed) => {
            // A defective RPC answer is not the player's fault: keep the order reviewable.
            tracing::error!(%order_id, "malformed rpc transaction; parked for review");
            park(pool, order_id, "needs_review", "malformed").await?;
            return Ok(SubmitOutcome::NeedsReview { order_id });
        }
        Err(reason) => {
            let code = reason.code();
            tracing::warn!(%order_id, code, "order rejected");
            park(pool, order_id, "rejected", code).await?;
            return Ok(SubmitOutcome::Rejected {
                order_id,
                reason_code: code.to_owned(),
            });
        }
    };
    let result = fulfil(pool, user_id, order_id, &order, &signature, &verified, commitment).await;
    finish(pool, user_id, order_id, &order, &signature, result).await
}

/// `Err(outcome)` when the chain has no verdict yet (202) or the RPC failed (parked).
async fn fetch_tx(
    pool: &PgPool,
    runtime: &PaymentsRuntime,
    order_id: Uuid,
    signature: &str,
    commitment: Commitment,
) -> Result<Result<cf_chain::TransactionResult, SubmitOutcome>, PaymentsError> {
    match runtime
        .chain
        .get_transaction_cross_checked(signature, commitment)
        .await
    {
        Ok(Some(tx)) => Ok(Ok(tx)),
        Ok(None) => Ok(Err(SubmitOutcome::Submitted { order_id })),
        Err(err) => {
            // Details stay in logs; the client only sees a fixed code.
            tracing::error!(%order_id, error = %err, "rpc failed; parked for review");
            park(pool, order_id, "needs_review", "rpc_error").await?;
            Ok(Err(SubmitOutcome::NeedsReview { order_id }))
        }
    }
}

fn expectation_for(runtime: &PaymentsRuntime, order_id: Uuid, order: &Claimed) -> Expectation {
    Expectation {
        mint: runtime.mint.clone(),
        decimals: runtime.decimals,
        owner: order.wallet.clone(),
        burn_amount: u64::try_from(order.burn_amount).unwrap_or(u64::MAX),
        transfer_amount: u64::try_from(order.transfer_amount).unwrap_or(u64::MAX),
        treasury_token_account: runtime.treasury_token_account.clone(),
        memo: memo_for(order_id),
        min_block_time: (order.created_at - BLOCK_TIME_SLACK).unix_timestamp(),
        max_block_time: (order.quote_expires_at + SUBMIT_GRACE).unix_timestamp(),
    }
}

/// Maps the fulfilment result to an outcome, parking or tracing anything unexpected.
async fn finish(
    pool: &PgPool,
    user_id: Uuid,
    order_id: Uuid,
    order: &Claimed,
    signature: &str,
    result: Result<(), PaymentsError>,
) -> Result<SubmitOutcome, PaymentsError> {
    match result {
        Ok(()) => Ok(SubmitOutcome::Fulfilled {
            order_id,
            amount_clash: order.burn_amount + order.transfer_amount,
        }),
        Err(PaymentsError::SignatureReused) => {
            park(pool, order_id, "rejected", "sig_reused").await?;
            Ok(SubmitOutcome::Rejected {
                order_id,
                reason_code: "sig_reused".to_owned(),
            })
        }
        Err(PaymentsError::WrongStatus) => {
            // Lost the race to a parallel submit. Same signature → that submit fulfilled this order.
            if let Some(amount_clash) = fulfilled_amount(pool, order_id, signature).await? {
                return Ok(SubmitOutcome::Fulfilled {
                    order_id,
                    amount_clash,
                });
            }
            // A second valid payment for an order that was fulfilled meanwhile: keep a trace for support.
            tracing::error!(%order_id, signature = %signature, "orphan payment signature");
            record_orphan(pool, user_id, order_id, signature).await?;
            Err(PaymentsError::WrongStatus)
        }
        Err(err) => {
            tracing::error!(%order_id, error = %err, "fulfilment failed; parked for review");
            park(pool, order_id, "needs_review", "fulfil_error").await?;
            Ok(SubmitOutcome::NeedsReview { order_id })
        }
    }
}

/// Locks the order, enforces ownership and status, binds the signature and marks the order
/// `submitted`. An order takes one signature only: the first one is recorded (`order_submitted`)
/// and any other is refused, so what an operator checks is what an approval books (review.rs).
/// A signature after the quote (`quoted` past its time, or `expired`) is verified like any other:
/// the block-time window in `verify` keeps the quoted price, a late burn is `too_late`.
/// `Ok(None)`: the order is already `needs_review` with this same signature.
async fn claim(
    pool: &PgPool,
    user_id: Uuid,
    order_id: Uuid,
    signature: &str,
) -> Result<Option<Claimed>, PaymentsError> {
    let mut tx = pool.begin().await?;
    let row = sqlx::query(
        "SELECT item_id, qty, price_usd_cents, burn_amount, transfer_amount, wallet, status, quote_expires_at, created_at
         FROM cf.orders WHERE id = $1 AND user_id = $2 FOR UPDATE",
    )
    .bind(order_id)
    .bind(user_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(PaymentsError::NotFound)?;
    let status: String = row.try_get("status")?;
    let recorded = recorded_signature(&mut tx, user_id, order_id).await?;
    if recorded.as_deref().is_some_and(|r| r != signature) {
        // A second payment for one order (two tabs, a re-signed transaction): never bound to the
        // order, but kept on record for support — the player may have burned the tokens.
        log_signature(&mut tx, user_id, "orphan_signature", order_id, signature).await?;
        tx.commit().await?;
        return Err(PaymentsError::WrongStatus);
    }
    if !may_claim(&status, recorded.as_deref(), signature)? {
        return Ok(None);
    }
    let used: Option<Uuid> = sqlx::query_scalar("SELECT order_id FROM cf.burns WHERE signature = $1")
        .bind(signature)
        .fetch_optional(&mut *tx)
        .await?;
    if used.is_some() {
        return Err(PaymentsError::SignatureReused);
    }
    if recorded.is_none() {
        // The trail first: whatever happens next, the signature is on record.
        log_signature(&mut tx, user_id, "order_submitted", order_id, signature).await?;
    }
    let expires: OffsetDateTime = row.try_get("quote_expires_at")?;
    sqlx::query("UPDATE cf.orders SET status = 'submitted', updated_at = now() WHERE id = $1")
        .bind(order_id)
        .execute(&mut *tx)
        .await?;
    let claimed = Claimed {
        item_id: row.try_get("item_id")?,
        qty: row.try_get("qty")?,
        price_usd_cents: row.try_get("price_usd_cents")?,
        burn_amount: row.try_get("burn_amount")?,
        transfer_amount: row.try_get("transfer_amount")?,
        wallet: row.try_get("wallet")?,
        created_at: row.try_get("created_at")?,
        quote_expires_at: expires,
    };
    tx.commit().await?;
    Ok(Some(claimed))
}

/// Whether `signature` may (re)start verification of an order in `status` whose first signature
/// was `recorded`: `Ok(false)` = already parked for review with this signature (nothing to do);
/// another signature than the recorded one, or a closed order, is refused.
fn may_claim(status: &str, recorded: Option<&str>, signature: &str) -> Result<bool, PaymentsError> {
    if recorded.is_some_and(|r| r != signature) {
        return Err(PaymentsError::WrongStatus);
    }
    match status {
        "quoted" | "submitted" | "expired" => Ok(true),
        "needs_review" if recorded.is_some() => Ok(false),
        _ => Err(PaymentsError::WrongStatus),
    }
}

async fn log_signature(
    conn: &mut sqlx::PgConnection,
    user_id: Uuid,
    name: &str,
    order_id: Uuid,
    signature: &str,
) -> Result<(), PaymentsError> {
    sqlx::query("INSERT INTO cf.events (user_id, name, props) VALUES ($1, $2, $3)")
        .bind(user_id)
        .bind(name)
        .bind(serde_json::json!({ "order_id": order_id, "signature": signature }))
        .execute(conn)
        .await?;
    Ok(())
}

/// The signature this order was first submitted with, if any.
async fn recorded_signature(
    conn: &mut sqlx::PgConnection,
    user_id: Uuid,
    order_id: Uuid,
) -> Result<Option<String>, PaymentsError> {
    Ok(sqlx::query_scalar(
        "SELECT props->>'signature' FROM cf.events
         WHERE user_id = $1 AND name = 'order_submitted' AND props->>'order_id' = $2
         ORDER BY id LIMIT 1",
    )
    .bind(user_id)
    .bind(order_id.to_string())
    .fetch_optional(conn)
    .await?
    .flatten())
}

/// `Some(amount)` when this exact signature already fulfilled this order.
async fn fulfilled_amount(
    pool: &PgPool,
    order_id: Uuid,
    signature: &str,
) -> Result<Option<i64>, PaymentsError> {
    Ok(sqlx::query_scalar(
        "SELECT o.amount_clash FROM cf.orders o JOIN cf.burns b ON b.order_id = o.id
         WHERE o.id = $1 AND b.signature = $2 AND o.status = 'fulfilled'",
    )
    .bind(order_id)
    .bind(signature)
    .fetch_optional(pool)
    .await?)
}

async fn record_orphan(
    pool: &PgPool,
    user_id: Uuid,
    order_id: Uuid,
    signature: &str,
) -> Result<(), PaymentsError> {
    sqlx::query("INSERT INTO cf.events (user_id, name, props) VALUES ($1, 'orphan_signature', $2)")
        .bind(user_id)
        .bind(serde_json::json!({ "order_id": order_id, "signature": signature }))
        .execute(pool)
        .await?;
    Ok(())
}

/// Sets a terminal/parked status with its reason outside any open transaction.
pub(crate) async fn park(
    pool: &PgPool,
    order_id: Uuid,
    status: &str,
    reason: &str,
) -> Result<(), PaymentsError> {
    sqlx::query("UPDATE cf.orders SET status = $2, reason_code = left($3, 120), updated_at = now() WHERE id = $1 AND status IN ('quoted', 'submitted')")
        .bind(order_id)
        .bind(status)
        .bind(reason)
        .execute(pool)
        .await?;
    Ok(())
}

#[cfg(test)]
mod claim_tests {
    use super::may_claim;

    #[test]
    fn an_order_takes_one_signature() {
        assert!(matches!(may_claim("quoted", None, "a"), Ok(true)));
        assert!(
            matches!(may_claim("expired", None, "a"), Ok(true)),
            "verified against its window"
        );
        assert!(
            matches!(may_claim("submitted", Some("a"), "a"), Ok(true)),
            "the client polls"
        );
        assert!(
            may_claim("submitted", Some("a"), "b").is_err(),
            "no second signature"
        );
        assert!(matches!(may_claim("needs_review", Some("a"), "a"), Ok(false)));
        assert!(
            may_claim("needs_review", Some("a"), "b").is_err(),
            "no swap under review"
        );
        assert!(may_claim("needs_review", None, "a").is_err());
        assert!(may_claim("fulfilled", Some("a"), "a").is_err());
        assert!(may_claim("rejected", None, "a").is_err());
    }
}
