//! Clearing wild bushes (migration 0023): the spot is remembered on the player's own island
//! (snapped to 0.5 m, idempotent), bounded by the island shape and a per-island cap, private
//! to its owner, and shows in the island snapshot. Cosmetic: nothing else changes.
//! Against local Postgres + Memurai.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use axum::http::StatusCode;
use common::{app, get, login, post};
use serde_json::json;
use uuid::Uuid;

async fn clear(app: &axum::Router, s: &common::Session, x: i16, z: i16) -> (StatusCode, serde_json::Value) {
    post(
        app,
        s,
        "/island/clear",
        Some(json!({ "x_dm": x, "z_dm": z })),
        None,
    )
    .await
}

#[tokio::test]
async fn test_clearing_is_remembered_snapped_idempotent_and_private() {
    let (app, pool) = app().await;
    let a = login(&app).await;
    let b = login(&app).await;
    get(&app, &a, "/island").await;
    get(&app, &b, "/island").await;
    let (status, body) = clear(&app, &a, 103, -57).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["cleared"], json!([[105, -55]]), "snapped to the 0.5 m grid");
    // The same bush (a click a hair to the side) is the same row.
    let (status, body) = clear(&app, &a, 105, -55).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["cleared"].as_array().unwrap().len(), 1);
    let (_, snapshot) = get(&app, &a, "/island").await;
    assert_eq!(
        snapshot["cleared"],
        json!([[105, -55]]),
        "the snapshot carries the cleared spots"
    );
    let (_, other) = get(&app, &b, "/island").await;
    assert_eq!(
        other["cleared"],
        json!([]),
        "another player's island is untouched"
    );
    // Cosmetic only: the plots, inventory and credits did not move.
    let events: i64 =
        sqlx::query_scalar("SELECT count(*) FROM cf.events WHERE user_id = $1 AND name = 'island_clear'")
            .bind(Uuid::parse_str(&a.user_id).unwrap())
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(events, 1, "one event for one spot, not for the repeat");
}

#[tokio::test]
async fn test_clearing_stays_on_the_island_and_under_the_cap() {
    let (app, pool) = app().await;
    let s = login(&app).await;
    get(&app, &s, "/island").await;
    // The base island is 48 m square (±240 dm): outside is refused.
    let (status, body) = clear(&app, &s, 300, 0).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(body["error"]["code"], "out_of_bounds");
    let (status, _) = clear(&app, &s, 0, -300).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    // Fill the island to the cap directly, then one more distinct spot is refused while a repeat is fine.
    let island: Uuid = sqlx::query_scalar("SELECT id FROM cf.islands WHERE user_id = $1")
        .bind(Uuid::parse_str(&s.user_id).unwrap())
        .fetch_one(&pool)
        .await
        .unwrap();
    sqlx::query(
        "INSERT INTO cf.island_clearings (island_id, x_dm, z_dm)
         SELECT $1, (-200 + (g % 40) * 10)::smallint, (-200 + (g / 40) * 10)::smallint FROM generate_series(0, 399) g",
    )
    .bind(island)
    .execute(&pool)
    .await
    .unwrap();
    let (status, body) = clear(&app, &s, 5, 5).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["error"]["code"], "clearing_limit");
    let (status, body) = clear(&app, &s, -200, -200).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "an already cleared spot stays fine at the cap: {body}"
    );
    assert_eq!(body["cleared"].as_array().unwrap().len(), 400);
}

#[tokio::test]
async fn test_clearing_needs_a_session_and_a_valid_body() {
    let (app, _) = app().await;
    let s = login(&app).await;
    get(&app, &s, "/island").await;
    let (status, _) = post(
        &app,
        &s,
        "/island/clear",
        Some(json!({ "x_dm": "far", "z_dm": 0 })),
        None,
    )
    .await;
    assert!(
        status == StatusCode::BAD_REQUEST || status == StatusCode::UNPROCESSABLE_ENTITY,
        "{status}"
    );
    let (status, _) = post(
        &app,
        &s,
        "/island/clear",
        Some(json!({ "x_dm": 40000, "z_dm": 0 })),
        None,
    )
    .await;
    assert!(
        status == StatusCode::BAD_REQUEST || status == StatusCode::UNPROCESSABLE_ENTITY,
        "{status}"
    );
}
