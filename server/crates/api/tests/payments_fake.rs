//! Fake payments (CF_PAYMENTS_FAKE, local only): a quote at the fixed price,
//! any well-formed signature fulfils the order with the real ledger/burn
//! bookkeeping, replays and reuse behave exactly as with a real chain, and the
//! catalog announces the mode. Against local Postgres + Memurai.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use axum::http::StatusCode;
use common::{app_with, get, login, post};
use serde_json::json;

fn fake_signature() -> String {
    let mut bytes = Vec::with_capacity(64);
    for _ in 0..4 {
        bytes.extend_from_slice(uuid::Uuid::new_v4().as_bytes());
    }
    bs58::encode(bytes).into_string()
}

#[tokio::test]
async fn test_catalog_announces_fake_mode() {
    let (app, _) = app_with(true).await;
    let session = login(&app).await;
    let (status, body) = get(&app, &session, "/catalog").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["payments"]["available"], true);
    assert_eq!(body["payments"]["fake"], true);
    let (plain, _) = app_with(false).await;
    let session = login(&plain).await;
    let (_, body) = get(&plain, &session, "/catalog").await;
    assert_eq!(body["payments"]["available"], false);
    assert_eq!(body["payments"]["fake"], false);
}

#[tokio::test]
async fn test_fake_order_fulfils_with_real_bookkeeping() {
    let (app, pool) = app_with(true).await;
    let session = login(&app).await;
    get(&app, &session, "/island").await;
    let (status, quote) = post(
        &app,
        &session,
        "/orders",
        Some(json!({ "item_id": "fert_premium", "qty": 2 })),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{quote}");
    // $0.90 × 2 at $0.001 per CLASH = 1 800 CLASH = 1 800 000 000 atomic units, split 70/30.
    assert_eq!(quote["amount_clash"], 1_800_000_000_i64);
    assert_eq!(quote["burn_amount"], 1_260_000_000_i64);
    let order_id = quote["order_id"].as_str().unwrap().to_owned();
    let signature = fake_signature();
    let (status, outcome) = post(
        &app,
        &session,
        &format!("/orders/{order_id}/submit"),
        Some(json!({ "signature": signature })),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{outcome}");
    assert_eq!(outcome["status"], "fulfilled");
    let (_, inv) = get(&app, &session, "/inventory").await;
    let ferts = inv["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["item_id"] == "fert_premium")
        .map(|i| i["qty"].as_i64().unwrap())
        .unwrap_or(0);
    assert_eq!(ferts, 20, "two packs of ten delivered (0019)");
    let burns: i64 = sqlx::query_scalar("SELECT count(*) FROM cf.burns WHERE signature = $1")
        .bind(&signature)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(burns, 1, "burn recorded like a real one");
    // Same signature again on the same order: still fulfilled, nothing delivered twice.
    let (status, again) = post(
        &app,
        &session,
        &format!("/orders/{order_id}/submit"),
        Some(json!({ "signature": signature })),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{again}");
    assert_eq!(again["status"], "fulfilled");
    let (_, inv) = get(&app, &session, "/inventory").await;
    let ferts = inv["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["item_id"] == "fert_premium")
        .map(|i| i["qty"].as_i64().unwrap())
        .unwrap_or(0);
    assert_eq!(ferts, 20);
    // A second order cannot reuse the signature.
    let (_, quote2) = post(
        &app,
        &session,
        "/orders",
        Some(json!({ "item_id": "fert_premium", "qty": 1 })),
        None,
    )
    .await;
    let order2 = quote2["order_id"].as_str().unwrap().to_owned();
    let (status, body) = post(
        &app,
        &session,
        &format!("/orders/{order2}/submit"),
        Some(json!({ "signature": signature })),
        None,
    )
    .await;
    assert_ne!(body["status"], "fulfilled", "{status} {body}");
    // A malformed signature is still refused.
    let (status, body) = post(
        &app,
        &session,
        &format!("/orders/{order2}/submit"),
        Some(json!({ "signature": "nope" })),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
}
