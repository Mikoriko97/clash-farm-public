//! Free decor placement (migration 0009): bounds with expansions, overlap with
//! decor and plots, rotation step, count limit, ownership, parallel placement.
//! Against local Postgres + Memurai.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use axum::http::StatusCode;
use common::{app, get, login, post};
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

async fn place(
    app: &axum::Router,
    session: &common::Session,
    item: &str,
    x: i16,
    z: i16,
    yaw: i16,
) -> (StatusCode, serde_json::Value) {
    post(
        app,
        session,
        "/decor/place",
        Some(json!({ "item_id": item, "x_dm": x, "z_dm": z, "yaw_deg": yaw })),
        None,
    )
    .await
}

#[tokio::test]
async fn test_bounds_rotation_and_plot_overlap() {
    let (app, pool) = app().await;
    let session = login(&app).await;
    get(&app, &session, "/island").await;
    give(&pool, &session, "decor_lantern", 5).await;
    let (status, body) = place(&app, &session, "decor_lantern", 235, 0, 0).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(body["error"]["code"], "out_of_bounds");
    let (status, body) = place(&app, &session, "decor_lantern", 0, 100, 10).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(body["error"]["code"], "invalid_rotation");
    // Starter plot at cell (0, -2) covers x -15..15, z -75..-45 dm: a lantern (5 dm) at (0, -70) sits on it.
    let (status, body) = place(&app, &session, "decor_lantern", 0, -70, 0).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["error"]["code"], "overlap");
    let (status, body) = place(&app, &session, "decor_lantern", 0, -35, 345).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["yaw_deg"], 345);
    // One paid strip east makes x = 300 dm reachable.
    let (status, _) = place(&app, &session, "decor_lantern", 300, 0, 0).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    sqlx::query("UPDATE cf.islands SET expand_e = 1 WHERE user_id = $1")
        .bind(Uuid::parse_str(&session.user_id).unwrap())
        .execute(&pool)
        .await
        .unwrap();
    let (status, body) = place(&app, &session, "decor_lantern", 300, 0, 0).await;
    assert_eq!(status, StatusCode::OK, "{body}");
}

#[tokio::test]
async fn test_decor_limit_and_foreign_remove() {
    let (app, pool) = app().await;
    let session = login(&app).await;
    get(&app, &session, "/island").await;
    // 0027 raised the island's decor limit from 8 to 16 (`economy.decor_slots`).
    give(&pool, &session, "decor_lantern", 17).await;
    let mut last = String::new();
    for i in 0..16 {
        let (status, body) = place(
            &app,
            &session,
            "decor_lantern",
            100 + 50 * (i % 2),
            -200 + (i / 2) * 40,
            0,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{body}");
        last = body["id"].as_str().unwrap().to_owned();
    }
    let (status, body) = place(&app, &session, "decor_lantern", 150, 200, 0).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["error"]["code"], "decor_limit");
    let stranger = login(&app).await;
    get(&app, &stranger, "/island").await;
    let (status, _) = post(
        &app,
        &stranger,
        "/decor/remove",
        Some(json!({ "id": last })),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "foreign placement never confirmed");
    let (status, _) = post(&app, &session, "/decor/remove", Some(json!({ "id": last })), None).await;
    assert_eq!(status, StatusCode::OK);
    let (_, snapshot) = get(&app, &session, "/island").await;
    assert_eq!(snapshot["decor"].as_array().unwrap().len(), 15);
}

#[tokio::test]
async fn test_parallel_place_on_one_spot_single_winner() {
    let (app, pool) = app().await;
    let session = login(&app).await;
    get(&app, &session, "/island").await;
    give(&pool, &session, "decor_barrel", 2).await;
    let (a, b) = tokio::join!(
        place(&app, &session, "decor_barrel", 120, 120, 0),
        place(&app, &session, "decor_barrel", 125, 120, 90)
    );
    let ok = [a.0, b.0].iter().filter(|s| **s == StatusCode::OK).count();
    assert_eq!(ok, 1, "{:?}", [a.0, b.0]);
    let (_, inv) = get(&app, &session, "/inventory").await;
    let barrels = inv["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["item_id"] == "decor_barrel")
        .map(|i| i["qty"].as_i64().unwrap())
        .unwrap_or(0);
    assert_eq!(barrels, 1, "only the winner spent a barrel");
}
