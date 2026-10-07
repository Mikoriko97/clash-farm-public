//! Payments guard rails: kill switches, ownership, races, provider disagreement,
//! quote lifetime (docs/SECURITY.md §4, §9).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use axum::http::StatusCode;
use cf_api::build_app;
use cf_api::state::SwitchCache;
use common::payments::*;
use common::{get as api_get, login, post as api_post};
use serde_json::{json, Value};
use std::time::Instant;
use uuid::Uuid;

#[tokio::test]
async fn test_oracle_down_and_kill_switch_block_orders() {
    let (app, _, _) = app_with_payments(false).await;
    let session = login(&app).await;
    api_get(&app, &session, "/island").await;
    let (status, body) = quote(&app, &session, "fert_premium", 1).await;
    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{body}");
    assert_eq!(body["error"]["code"], "oracle_unavailable");

    // Kill switch via the cached settings (no global DB change → other tests unaffected).
    let (state, _pool, _) = state_with_payments(true).await;
    *state.switches.write().await = Some(SwitchCache {
        checked_at: Instant::now(),
        maintenance: false,
        shop_enabled: true,
        payments_enabled: false,
        renames_enabled: true,
    });
    let app = build_app(state);
    let session = login(&app).await;
    api_get(&app, &session, "/island").await;
    let (status, body) = quote(&app, &session, "fert_premium", 1).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["error"]["code"], "payments_disabled");
    let (status, body) = api_post(
        &app,
        &session,
        "/shop/buy-credits",
        Some(json!({ "item_id": "seed_wheat", "qty": 1 })),
        None,
    )
    .await;
    assert_ne!(
        body["error"]["code"], "payments_disabled",
        "shop stays open when only payments are paused: {status}"
    );
}

#[tokio::test]
async fn test_foreign_order_is_404_for_status_and_submit() {
    let (app, _, chain) = app_with_payments(true).await;
    let owner = login(&app).await;
    let stranger = login(&app).await;
    api_get(&app, &owner, "/island").await;
    api_get(&app, &stranger, "/island").await;
    let (_, order) = quote(&app, &owner, "fert_premium", 1).await;
    let order_id = order["order_id"].as_str().unwrap().to_owned();
    let (status, body) = api_get(&app, &stranger, &format!("/orders/{order_id}")).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let sig = signature();
    chain
        .txs
        .lock()
        .unwrap()
        .insert(sig.clone(), good_tx(&owner, &order));
    let (status, body) = submit(&app, &stranger, &order_id, &sig).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let (_, view) = api_get(&app, &owner, &format!("/orders/{order_id}")).await;
    assert_eq!(
        view["status"], "quoted",
        "stranger's submit must not touch the order"
    );
}

#[tokio::test]
async fn test_concurrent_submits_fulfil_exactly_once() {
    let (app, pool, chain) = app_with_payments(true).await;
    let session = login(&app).await;
    api_get(&app, &session, "/island").await;
    // (a) one signature raced across two orders → exactly one fulfilment.
    let (_, a) = quote(&app, &session, "fert_premium", 1).await;
    let (_, b) = quote(&app, &session, "fert_premium", 1).await;
    let sig = signature();
    chain
        .txs
        .lock()
        .unwrap()
        .insert(sig.clone(), good_tx(&session, &a));
    let (ra, rb) = tokio::join!(
        submit(&app, &session, a["order_id"].as_str().unwrap(), &sig),
        submit(&app, &session, b["order_id"].as_str().unwrap(), &sig)
    );
    let fulfilled = [&ra.1, &rb.1]
        .iter()
        .filter(|o| o["status"] == "fulfilled")
        .count();
    assert_eq!(fulfilled, 1, "a={:?} b={:?}", ra, rb);
    let burns: i64 = sqlx::query_scalar("SELECT count(*) FROM cf.burns WHERE signature = $1")
        .bind(&sig)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(burns, 1);
    // (b) one order raced with two valid signatures → one fulfilment, the other signature recorded as orphan.
    let (_, c) = quote(&app, &session, "fert_premium", 1).await;
    let c_id = c["order_id"].as_str().unwrap().to_owned();
    let (s1, s2) = (signature(), signature());
    chain
        .txs
        .lock()
        .unwrap()
        .insert(s1.clone(), good_tx(&session, &c));
    chain
        .txs
        .lock()
        .unwrap()
        .insert(s2.clone(), good_tx(&session, &c));
    let (r1, r2) = tokio::join!(
        submit(&app, &session, &c_id, &s1),
        submit(&app, &session, &c_id, &s2)
    );
    let ok = [&r1.1, &r2.1]
        .iter()
        .filter(|o| o["status"] == "fulfilled")
        .count();
    assert_eq!(ok, 1, "r1={:?} r2={:?}", r1, r2);
    let burns_c: i64 = sqlx::query_scalar("SELECT count(*) FROM cf.burns WHERE order_id = $1")
        .bind(Uuid::parse_str(&c_id).unwrap())
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(burns_c, 1);
    let (_, inv) = api_get(&app, &session, "/inventory").await;
    let premium = inv["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["item_id"] == "fert_premium")
        .unwrap();
    assert_eq!(premium["qty"], 20, "one pack of ten per fulfilled order");
}

#[tokio::test]
async fn test_provider_disagreement_and_rpc_failure_park_for_review() {
    let (state, pool, chain) = state_with_options(true, Providers::Disagreeing).await;
    let app = build_app(state);
    let session = login(&app).await;
    api_get(&app, &session, "/island").await;
    let (_, order) = quote(&app, &session, "fert_premium", 1).await;
    let order_id = order["order_id"].as_str().unwrap().to_owned();
    let sig = signature();
    chain
        .txs
        .lock()
        .unwrap()
        .insert(sig.clone(), good_tx(&session, &order));
    let (status, body) = submit(&app, &session, &order_id, &sig).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["status"], "needs_review");
    let (_, view) = api_get(&app, &session, &format!("/orders/{order_id}")).await;
    assert_eq!(view["status"], "needs_review");
    assert_eq!(
        view["reason_code"], "rpc_error",
        "fixed code, no provider text: {view}"
    );
    // While an operator checks it, the player cannot swap in another signature for the approval
    // to book; the same one only repeats the answer.
    let (status, body) = submit(&app, &session, &order_id, &signature()).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["error"]["code"], "wrong_status");
    let (status, body) = submit(&app, &session, &order_id, &sig).await;
    assert_eq!(
        (status, body["status"].clone()),
        (StatusCode::OK, json!("needs_review"))
    );
    let bound = cf_payments::review::submitted_signature(&pool, Uuid::parse_str(&order_id).unwrap())
        .await
        .unwrap();
    assert_eq!(
        bound.as_deref(),
        Some(sig.as_str()),
        "the approval books the checked signature"
    );

    let (state, _pool, _) = state_with_options(true, Providers::Broken).await;
    let app = build_app(state);
    let session = login(&app).await;
    api_get(&app, &session, "/island").await;
    let (_, order) = quote(&app, &session, "fert_premium", 1).await;
    let order_id = order["order_id"].as_str().unwrap().to_owned();
    let (status, body) = submit(&app, &session, &order_id, &signature()).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["status"], "needs_review");
    let (_, view) = api_get(&app, &session, &format!("/orders/{order_id}")).await;
    assert_eq!(view["reason_code"], "rpc_error");
}

#[tokio::test]
async fn test_submitted_order_cannot_outlive_its_quote() {
    let (app, pool, chain) = app_with_payments(true).await;
    let session = login(&app).await;
    api_get(&app, &session, "/island").await;
    // Park an order in `submitted` with a signature the RPC never sees, then age the quote.
    let (_, order) = quote(&app, &session, "fert_premium", 1).await;
    let order_id = order["order_id"].as_str().unwrap().to_owned();
    let sig = signature();
    let (status, _) = submit(&app, &session, &order_id, &sig).await;
    assert_eq!(status, StatusCode::ACCEPTED);
    sqlx::query("UPDATE cf.orders SET quote_expires_at = now() - interval '3 minutes' WHERE id = $1")
        .bind(Uuid::parse_str(&order_id).unwrap())
        .execute(&pool)
        .await
        .unwrap();
    // The signature came in time (the order is `submitted`), so it is still verified past the
    // grace — and the price window is the block time: mined now, after quote + grace, it is too late.
    chain
        .txs
        .lock()
        .unwrap()
        .insert(sig.clone(), good_tx(&session, &order));
    let (status, body) = submit(&app, &session, &order_id, &sig).await;
    assert_eq!(
        (status, body["reason_code"].clone()),
        (StatusCode::OK, json!("too_late")),
        "{body}"
    );
    // Mined inside the window (the chain was only slow to answer): fulfilled, not expired.
    let (_, order) = quote(&app, &session, "fert_premium", 1).await;
    let order_id = order["order_id"].as_str().unwrap().to_owned();
    let sig = signature();
    let (status, _) = submit(&app, &session, &order_id, &sig).await;
    assert_eq!(status, StatusCode::ACCEPTED);
    // Past the grace (expired 150 s ago), mined 40 s ago: inside [created - 60 s, expires + 120 s].
    sqlx::query("UPDATE cf.orders SET quote_expires_at = now() - interval '150 seconds' WHERE id = $1")
        .bind(Uuid::parse_str(&order_id).unwrap())
        .execute(&pool)
        .await
        .unwrap();
    let mut tx = good_tx(&session, &order);
    tx["blockTime"] = json!(time::OffsetDateTime::now_utc().unix_timestamp() - 40);
    chain.txs.lock().unwrap().insert(sig.clone(), tx);
    let (status, body) = submit(&app, &session, &order_id, &sig).await;
    assert_eq!(
        (status, body["status"].clone()),
        (StatusCode::OK, json!("fulfilled")),
        "{body}"
    );
    // A block mined after quote_expires_at + grace is rejected even inside the DB window.
    let (_, order) = quote(&app, &session, "fert_premium", 1).await;
    let order_id = order["order_id"].as_str().unwrap().to_owned();
    let sig = signature();
    let mut tx = good_tx(&session, &order);
    tx["blockTime"] = json!(time::OffsetDateTime::now_utc().unix_timestamp() + 600);
    chain.txs.lock().unwrap().insert(sig.clone(), tx);
    let (status, body) = submit(&app, &session, &order_id, &sig).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["reason_code"], "too_late");
    let (_, order) = quote(&app, &session, "fert_premium", 1).await;
    let sig = signature();
    let mut tx = good_tx(&session, &order);
    tx["blockTime"] = Value::Null;
    chain.txs.lock().unwrap().insert(sig.clone(), tx);
    let (_, body) = submit(&app, &session, order["order_id"].as_str().unwrap(), &sig).await;
    assert_eq!(
        body["status"], "needs_review",
        "rpc defect is reviewable, not terminal: {body}"
    );
    let (_, view) = api_get(
        &app,
        &session,
        &format!("/orders/{}", order["order_id"].as_str().unwrap()),
    )
    .await;
    assert_eq!(view["reason_code"], "malformed");
}

#[tokio::test]
async fn test_lagging_secondary_provider_is_not_a_disagreement() {
    let (state, _pool, chain) = state_with_options(true, Providers::Lagging).await;
    let app = build_app(state);
    let session = login(&app).await;
    api_get(&app, &session, "/island").await;
    let (_, order) = quote(&app, &session, "fert_premium", 1).await;
    let order_id = order["order_id"].as_str().unwrap().to_owned();
    let sig = signature();
    chain
        .txs
        .lock()
        .unwrap()
        .insert(sig.clone(), good_tx(&session, &order));
    let (status, body) = submit(&app, &session, &order_id, &sig).await;
    assert_eq!(status, StatusCode::ACCEPTED, "{body}");
    assert_eq!(body["status"], "submitted");
    let (_, view) = api_get(&app, &session, &format!("/orders/{order_id}")).await;
    assert_eq!(view["status"], "submitted");
}

#[tokio::test]
async fn test_a_signed_order_is_verified_while_payments_are_paused() {
    let (state, _pool, chain) = state_with_payments(true).await;
    let app = build_app(state.clone());
    let session = login(&app).await;
    api_get(&app, &session, "/island").await;
    let (_, order) = quote(&app, &session, "fert_premium", 1).await;
    let order_id = order["order_id"].as_str().unwrap().to_owned();
    // The player signed; then the operator pauses payments (or the oracle pauses itself).
    *state.switches.write().await = Some(SwitchCache {
        checked_at: Instant::now(),
        maintenance: false,
        shop_enabled: false,
        payments_enabled: false,
        renames_enabled: true,
    });
    let (status, body) = quote(&app, &session, "fert_premium", 1).await;
    assert_eq!(status, StatusCode::CONFLICT, "no new orders: {body}");
    let sig = signature();
    chain
        .txs
        .lock()
        .unwrap()
        .insert(sig.clone(), good_tx(&session, &order));
    let (status, body) = submit(&app, &session, &order_id, &sig).await;
    assert_eq!(
        (status, body["status"].clone()),
        (StatusCode::OK, json!("fulfilled")),
        "{body}"
    );
    let (status, view) = api_get(&app, &session, &format!("/orders/{order_id}")).await;
    assert_eq!(
        (status, view["status"].clone()),
        (StatusCode::OK, json!("fulfilled"))
    );
}
