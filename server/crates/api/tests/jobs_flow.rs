//! Background jobs (migration 0011) as plain functions: expiry of stale quotes,
//! retention of events/rate_events, ledger reconciliation, finality skip without
//! a chain, and the oracle guard's persisted pause. Against local Postgres + Memurai.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use cf_api::jobs::{expire, finality, oracle_guard, reconcile, retention};
use common::{app, get, login};
use serde_json::Value;
use uuid::Uuid;

async fn user_with_island(app: &axum::Router) -> (common::Session, Uuid) {
    let session = login(app).await;
    get(app, &session, "/island").await;
    (session.clone(), Uuid::parse_str(&session.user_id).unwrap())
}

async fn order_status(pool: &sqlx::PgPool, id: Uuid) -> String {
    sqlx::query_scalar("SELECT status FROM cf.orders WHERE id = $1")
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn test_stale_quotes_expire_and_fresh_ones_stay() {
    let (app, pool) = app().await;
    let (_, user_id) = user_with_island(&app).await;
    let insert = |offset_secs: i64, status: &'static str| {
        let pool = pool.clone();
        async move {
            sqlx::query_scalar::<_, Uuid>(
                "INSERT INTO cf.orders (user_id, item_id, qty, price_usd_cents, clash_per_usd, amount_clash,
                                        burn_amount, transfer_amount, quote_expires_at, status)
                 VALUES ($1, 'decor_lantern', 1, 90, 1000, 90000, 63000, 27000,
                         now() + ($2::bigint * interval '1 second'), $3) RETURNING id",
            )
            .bind(user_id)
            .bind(offset_secs)
            .bind(status)
            .fetch_one(&pool)
            .await
            .unwrap()
        }
    };
    let stale_quoted = insert(-600, "quoted").await;
    let stale_submitted = insert(-600, "submitted").await;
    let within_grace = insert(-60, "quoted").await;
    let fresh = insert(60, "quoted").await;
    let expired = expire::expire_quotes(&pool).await.unwrap();
    assert!(expired >= 1, "{expired}");
    assert_eq!(order_status(&pool, stale_quoted).await, "expired");
    assert_eq!(order_status(&pool, within_grace).await, "quoted", "120 s grace");
    assert_eq!(order_status(&pool, fresh).await, "quoted");
    // A submission is never expired: fresh ones are left to the in-flight verification,
    // stuck ones go to the review queue with their signature still on record.
    expire::park_stuck_submissions(&pool).await.unwrap();
    assert_eq!(
        order_status(&pool, stale_submitted).await,
        "submitted",
        "touched recently: in flight"
    );
    sqlx::query("UPDATE cf.orders SET updated_at = now() - interval '11 minutes' WHERE id = $1")
        .bind(stale_submitted)
        .execute(&pool)
        .await
        .unwrap();
    expire::park_stuck_submissions(&pool).await.unwrap();
    assert_eq!(order_status(&pool, stale_submitted).await, "needs_review");
    let reason: Option<String> = sqlx::query_scalar("SELECT reason_code FROM cf.orders WHERE id = $1")
        .bind(stale_submitted)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(reason.as_deref(), Some("submit_timeout"));
}

#[tokio::test]
async fn test_retention_purges_only_old_rows() {
    let (app, pool) = app().await;
    let (_, user_id) = user_with_island(&app).await;
    for (age_days, name) in [
        (100i64, "old_event"),
        (1, "young_event"),
        (400, "order_submitted"),
    ] {
        sqlx::query(
            "INSERT INTO cf.events (user_id, name, props, created_at)
             VALUES ($1, $2, '{}', now() - ($3::bigint * interval '1 day'))",
        )
        .bind(user_id)
        .bind(name)
        .bind(age_days)
        .execute(&pool)
        .await
        .unwrap();
    }
    for (age_days, action) in [(40i32, "old_cap"), (2, "young_cap")] {
        sqlx::query("INSERT INTO cf.rate_events (user_id, action, day, count) VALUES ($1, $2, current_date - $3::int, 1)")
            .bind(user_id)
            .bind(action)
            .bind(age_days)
            .execute(&pool)
            .await
            .unwrap();
    }
    let (events, rate) = retention::purge(&pool, 90, 30).await.unwrap();
    assert!(events >= 1 && rate >= 1, "events={events} rate={rate}");
    let names: Vec<String> =
        sqlx::query_scalar("SELECT name FROM cf.events WHERE user_id = $1 ORDER BY name")
            .bind(user_id)
            .fetch_all(&pool)
            .await
            .unwrap();
    assert!(
        !names.contains(&"old_event".to_owned()) && names.contains(&"young_event".to_owned()),
        "{names:?}"
    );
    assert!(
        names.contains(&"order_submitted".to_owned()),
        "payment evidence is never purged: {names:?}"
    );
    let actions: Vec<String> =
        sqlx::query_scalar("SELECT action FROM cf.rate_events WHERE user_id = $1 ORDER BY action")
            .bind(user_id)
            .fetch_all(&pool)
            .await
            .unwrap();
    assert!(
        !actions.contains(&"old_cap".to_owned()) && actions.contains(&"young_cap".to_owned()),
        "{actions:?}"
    );
}

#[tokio::test]
async fn test_reconcile_flags_a_tampered_balance_and_alerts() {
    let (app, pool) = app().await;
    let (_, user_id) = user_with_island(&app).await;
    let clean = reconcile::ledger_vs_balances(&pool).await.unwrap();
    assert!(
        clean.iter().all(|m| m.user_id != user_id),
        "starter credits balance matches its ledger: {clean:?}"
    );
    sqlx::query("UPDATE cf.balances SET amount = amount + 7 WHERE user_id = $1 AND account = 'credits'")
        .bind(user_id)
        .execute(&pool)
        .await
        .unwrap();
    let found = reconcile::ledger_vs_balances(&pool).await.unwrap();
    let mine = found
        .iter()
        .find(|m| m.user_id == user_id)
        .expect("mismatch listed");
    assert_eq!(mine.balance - mine.ledger_sum, 7);
    let summary = reconcile::run(&pool).await.unwrap();
    assert!(summary.starts_with("mismatches="), "{summary}");
    let alerts: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM cf.audit_log WHERE actor = 'job:reconcile' AND action = 'ledger_mismatch' AND created_at > now() - interval '1 minute'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(alerts >= 1);
    // Put the balance back so other tests see a clean ledger.
    sqlx::query("UPDATE cf.balances SET amount = amount - 7 WHERE user_id = $1 AND account = 'credits'")
        .bind(user_id)
        .execute(&pool)
        .await
        .unwrap();
    let again = reconcile::ledger_vs_balances(&pool).await.unwrap();
    assert!(again.iter().all(|m| m.user_id != user_id));
}

#[tokio::test]
async fn test_finality_skips_without_a_chain() {
    let (_, pool) = app().await;
    let summary = finality::run(&pool, None).await.unwrap();
    assert!(summary.contains("skipped"), "{summary}");
    let fake = cf_payments::PaymentsRuntime::fake("9mM1Mc4Ta9UJJ32v5qsHef91PiXi7EWyiSsqF5WXpump", 6).unwrap();
    let summary = finality::run(&pool, Some(&fake)).await.unwrap();
    assert!(
        summary.contains("skipped"),
        "fake runtime has no chain: {summary}"
    );
}

#[tokio::test]
async fn test_oracle_pause_is_persisted_and_audited() {
    let (_, pool) = app().await;
    oracle_guard::disable_payments(&pool, "stale price")
        .await
        .unwrap();
    let value: Value = sqlx::query_scalar("SELECT value FROM cf.settings WHERE key = 'payments_enabled'")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(value, Value::Bool(false));
    let (actor, after): (String, Value) = sqlx::query_as(
        "SELECT actor, after FROM cf.audit_log WHERE action = 'oracle_pause' ORDER BY id DESC LIMIT 1",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(actor, "job:oracle");
    assert_eq!(after["reason"], "stale price");
    sqlx::query("UPDATE cf.settings SET value = 'true'::jsonb WHERE key = 'payments_enabled'")
        .execute(&pool)
        .await
        .unwrap();
}
