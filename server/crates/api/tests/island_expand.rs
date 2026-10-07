//! Island expansion (ADR-015, migration 0010): permits are catalog items sold
//! for CLASH; expanding a side needs the permit of the next tier, four strips
//! per side at most, and the new strip is usable by decor placement at once.
//! Against local Postgres + Memurai.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use axum::http::StatusCode;
use common::{app, app_with, get, login, post};
use serde_json::json;
use uuid::Uuid;

async fn give(pool: &sqlx::PgPool, session: &common::Session, item: &str, qty: i32) {
    sqlx::query(
        "INSERT INTO cf.inventory (user_id, item_id, qty) VALUES ($1, $2, $3)
         ON CONFLICT (user_id, item_id) DO UPDATE SET qty = cf.inventory.qty + EXCLUDED.qty",
    )
    .bind(Uuid::parse_str(&session.user_id).unwrap())
    .bind(item)
    .bind(qty)
    .execute(pool)
    .await
    .unwrap();
}

/// Permits are level-gated (0019): tier 4 needs level 22, so the flow runs on a grown island.
async fn set_level(pool: &sqlx::PgPool, session: &common::Session, level: i32) {
    sqlx::query("UPDATE cf.islands SET level = $2 WHERE user_id = $1")
        .bind(Uuid::parse_str(&session.user_id).unwrap())
        .bind(level)
        .execute(pool)
        .await
        .unwrap();
}

async fn expand(
    app: &axum::Router,
    session: &common::Session,
    side: &str,
) -> (StatusCode, serde_json::Value) {
    post(
        app,
        session,
        "/island/expand",
        Some(json!({ "side": side })),
        None,
    )
    .await
}

#[tokio::test]
async fn test_permits_are_in_the_catalog_for_clash() {
    let (app, _) = app().await;
    let session = login(&app).await;
    let (_, body) = get(&app, &session, "/catalog").await;
    let items = body["items"].as_array().unwrap();
    let permit = items
        .iter()
        .find(|i| i["id"] == "island_expand_1")
        .expect("tier 1 permit");
    assert_eq!(permit["price_usd_cents"], 290);
    assert!(permit["price_credits"].is_null(), "CLASH only");
    assert!(items
        .iter()
        .any(|i| i["id"] == "island_expand_4" && i["price_usd_cents"] == 1290));
}

#[tokio::test]
async fn test_expand_needs_the_next_tier_permit_and_stops_at_four() {
    let (app, pool) = app().await;
    let session = login(&app).await;
    get(&app, &session, "/island").await;
    set_level(&pool, &session, 22).await;
    let (status, body) = expand(&app, &session, "e").await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["error"]["code"], "insufficient_items");
    let (status, body) = expand(&app, &session, "up").await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(body["error"]["code"], "invalid_side");
    give(&pool, &session, "island_expand_1", 2).await;
    let (status, body) = expand(&app, &session, "e").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["island"]["expand_e"], 1);
    assert_eq!(body["level"], 1);
    assert_eq!(body["permit_used"], "island_expand_1");
    // Second strip east needs tier 2; the spare tier-1 permit does not count.
    let (status, body) = expand(&app, &session, "e").await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["error"]["code"], "insufficient_items");
    // Tier 1 still works for another side.
    let (status, body) = expand(&app, &session, "n").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["island"]["expand_n"], 1);
    for tier in 2..=4 {
        give(&pool, &session, &format!("island_expand_{tier}"), 1).await;
        let (status, body) = expand(&app, &session, "e").await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["island"]["expand_e"], tier);
    }
    give(&pool, &session, "island_expand_4", 1).await;
    let (status, body) = expand(&app, &session, "e").await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["error"]["code"], "expand_max");
    let (_, snapshot) = get(&app, &session, "/island").await;
    assert_eq!(snapshot["island"]["expand_e"], 4);
    assert_eq!(snapshot["island"]["expand_n"], 1);
    // The new eastern strip takes decor: x = 500 dm needs 4 strips (24 + 32 m).
    give(&pool, &session, "decor_lantern", 1).await;
    let (status, body) = post(
        &app,
        &session,
        "/decor/place",
        Some(json!({ "item_id": "decor_lantern", "x_dm": 500, "z_dm": 0, "yaw_deg": 0 })),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
}

#[tokio::test]
async fn test_parallel_expand_spends_one_permit_once() {
    let (app, pool) = app().await;
    let session = login(&app).await;
    get(&app, &session, "/island").await;
    set_level(&pool, &session, 10).await; // tier 2 opens at 10: the loser below fails on the permit, not the level
                                          // One tier-1 permit, two sides at once: exactly one strip appears.
    give(&pool, &session, "island_expand_1", 1).await;
    let (a, b) = tokio::join!(expand(&app, &session, "n"), expand(&app, &session, "s"));
    let statuses = [a.0, b.0];
    assert!(statuses.contains(&StatusCode::OK), "{a:?} {b:?}");
    assert!(statuses.contains(&StatusCode::CONFLICT), "{a:?} {b:?}");
    let (_, snapshot) = get(&app, &session, "/island").await;
    let strips =
        snapshot["island"]["expand_n"].as_i64().unwrap() + snapshot["island"]["expand_s"].as_i64().unwrap();
    assert_eq!(strips, 1, "{snapshot}");
    // Same side twice with two permits of the same tier: the loser needs tier 2.
    give(&pool, &session, "island_expand_1", 2).await;
    let (a, b) = tokio::join!(expand(&app, &session, "e"), expand(&app, &session, "e"));
    let statuses = [a.0, b.0];
    assert!(statuses.contains(&StatusCode::OK), "{a:?} {b:?}");
    assert!(statuses.contains(&StatusCode::CONFLICT), "{a:?} {b:?}");
    let (_, snapshot) = get(&app, &session, "/island").await;
    assert_eq!(snapshot["island"]["expand_e"], 1, "{snapshot}");
    let (_, inv) = get(&app, &session, "/inventory").await;
    let left = inv["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["item_id"] == "island_expand_1")
        .map(|i| i["qty"].as_i64().unwrap())
        .unwrap_or(0);
    assert_eq!(left, 1, "only the winner spent a permit: {inv}");
}

#[tokio::test]
async fn test_permit_bought_with_fake_clash_expands_the_island() {
    let (app, pool) = app_with(true).await;
    let session = login(&app).await;
    get(&app, &session, "/island").await;
    set_level(&pool, &session, 5).await;
    let (status, quote) = post(
        &app,
        &session,
        "/orders",
        Some(json!({ "item_id": "island_expand_1", "qty": 1 })),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{quote}");
    let order_id = quote["order_id"].as_str().unwrap().to_owned();
    let mut bytes = Vec::with_capacity(64);
    for _ in 0..4 {
        bytes.extend_from_slice(Uuid::new_v4().as_bytes());
    }
    let signature = bs58::encode(bytes).into_string();
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
    let (status, body) = expand(&app, &session, "w").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["island"]["expand_w"], 1);
}
