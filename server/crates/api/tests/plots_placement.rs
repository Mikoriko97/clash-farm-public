//! Player-placed plots and beds (migration 0006): bounds, occupancy, level
//! limit, bed size, remove/return, bed plant/harvest as one transaction,
//! ownership. Against local Postgres + Memurai.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use axum::http::StatusCode;
use common::{app, get, login, post};
use serde_json::json;
use uuid::Uuid;

async fn island(app: &axum::Router, session: &common::Session) -> serde_json::Value {
    let (status, body) = get(app, session, "/island").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body
}

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

async fn set_level(pool: &sqlx::PgPool, session: &common::Session, level: i32) {
    sqlx::query("UPDATE cf.islands SET level = $2 WHERE user_id = $1")
        .bind(Uuid::parse_str(&session.user_id).unwrap())
        .bind(level)
        .execute(pool)
        .await
        .unwrap();
}

async fn force_all_ready(pool: &sqlx::PgPool, session: &common::Session) {
    sqlx::query(
        "UPDATE cf.plots SET ready_at = now() - interval '1 second'
         WHERE island_id = (SELECT id FROM cf.islands WHERE user_id = $1) AND state = 'growing'",
    )
    .bind(Uuid::parse_str(&session.user_id).unwrap())
    .execute(pool)
    .await
    .unwrap();
}

async fn place(
    app: &axum::Router,
    session: &common::Session,
    col: i16,
    row: i16,
) -> (StatusCode, serde_json::Value) {
    post(
        app,
        session,
        "/plots/place",
        Some(json!({ "col": col, "row": row })),
        None,
    )
    .await
}

fn qty(inv: &serde_json::Value, item: &str) -> i64 {
    inv["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["item_id"] == item)
        .map(|e| e["qty"].as_i64().unwrap())
        .unwrap_or(0)
}

#[tokio::test]
async fn test_starter_plots_have_cells_and_one_bed() {
    let (app, _) = app().await;
    let session = login(&app).await;
    let snapshot = island(&app, &session).await;
    let plots = snapshot["plots"].as_array().unwrap();
    assert_eq!(plots.len(), 6);
    assert_eq!(plots[0]["col"], -1);
    assert_eq!(plots[0]["row"], -3);
    assert_eq!(plots[5]["col"], 1);
    assert_eq!(plots[5]["row"], -2);
    // Two rows of three touch by their sides: one bed for all six.
    assert!(plots.iter().all(|p| p["bed"] == 0), "{plots:?}");
}

#[tokio::test]
async fn test_place_checks_limit_bounds_cell_and_items() {
    let (app, pool) = app().await;
    let session = login(&app).await;
    island(&app, &session).await;
    // Level 1 allows 6 plots: the starter six fill it (checked before the inventory).
    let (status, body) = place(&app, &session, 4, 4).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["error"]["code"], "plot_limit");
    set_level(&pool, &session, 3).await; // 7 plots (0019: one more every two levels)
                                         // No plot item yet.
    let (status, body) = place(&app, &session, 4, 4).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["error"]["code"], "insufficient_items");
    give(&pool, &session, "plot", 3).await;
    let (status, body) = place(&app, &session, 8, 0).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(body["error"]["code"], "out_of_bounds");
    let (status, body) = place(&app, &session, 0, -3).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["error"]["code"], "cell_taken");
    let (status, body) = place(&app, &session, 4, 4).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["col"], 4);
    assert_eq!(body["row"], 4);
    assert_eq!(body["state"], "empty");
    assert_eq!(body["idx"], 6);
    let snapshot = island(&app, &session).await;
    let plots = snapshot["plots"].as_array().unwrap();
    assert_eq!(plots.len(), 7);
    assert_eq!(plots[6]["bed"], 1, "a lone plot is its own bed");
    let (_, inv) = get(&app, &session, "/inventory").await;
    assert_eq!(qty(&inv, "plot"), 2);
}

#[tokio::test]
async fn test_bed_size_capped_at_nine() {
    let (app, pool) = app().await;
    let session = login(&app).await;
    island(&app, &session).await;
    give(&pool, &session, "plot", 5).await;
    set_level(&pool, &session, 12).await; // 12 plots
    for col in -1..=1 {
        let (status, body) = place(&app, &session, col, -1).await;
        assert_eq!(status, StatusCode::OK, "{body}");
    }
    let (status, body) = place(&app, &session, 2, -1).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["error"]["code"], "bed_too_large");
    // Diagonal neighbour is not adjacent: a new bed.
    let (status, body) = place(&app, &session, 2, 0).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let snapshot = island(&app, &session).await;
    let beds: std::collections::HashSet<i64> = snapshot["plots"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| p["bed"].as_i64().unwrap())
        .collect();
    assert_eq!(beds.len(), 2);
}

#[tokio::test]
async fn test_remove_returns_item_and_refuses_growing_or_foreign() {
    let (app, pool) = app().await;
    let session = login(&app).await;
    let snapshot = island(&app, &session).await;
    let plot_a = snapshot["plots"][0]["id"].as_str().unwrap().to_owned();
    let plot_b = snapshot["plots"][1]["id"].as_str().unwrap().to_owned();
    let (status, _) = post(
        &app,
        &session,
        &format!("/plots/{plot_b}/plant"),
        Some(json!({ "item_id": "seed_wheat" })),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, body) = post(&app, &session, &format!("/plots/{plot_b}/remove"), None, None).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["error"]["code"], "wrong_state");
    let (status, body) = post(&app, &session, &format!("/plots/{plot_a}/remove"), None, None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["removed"], plot_a);
    let (_, inv) = get(&app, &session, "/inventory").await;
    assert_eq!(qty(&inv, "plot"), 1);
    assert_eq!(island(&app, &session).await["plots"].as_array().unwrap().len(), 5);
    // Re-placing on the freed cell works again at level 1 (count is back under the limit).
    let (status, body) = place(&app, &session, -1, -3).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    // Another player cannot remove it.
    let stranger = login(&app).await;
    let (status, _) = post(
        &app,
        &stranger,
        &format!("/plots/{}/remove", body["id"].as_str().unwrap()),
        None,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    drop(pool);
}

#[tokio::test]
async fn test_bed_actions_on_foreign_plot_are_404() {
    let (app, _) = app().await;
    let owner = login(&app).await;
    let snapshot = island(&app, &owner).await;
    let plot0 = snapshot["plots"][0]["id"].as_str().unwrap().to_owned();
    let stranger = login(&app).await;
    island(&app, &stranger).await;
    for path in [
        format!("/plots/{plot0}/bed/plant"),
        format!("/plots/{plot0}/bed/fertilize"),
        format!("/plots/{plot0}/bed/harvest"),
    ] {
        let body = if path.ends_with("harvest") {
            None
        } else {
            Some(json!({ "item_id": "seed_wheat" }))
        };
        let (status, _) = post(&app, &stranger, &path, body, None).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{path}");
    }
}

#[tokio::test]
async fn test_parallel_place_on_one_cell_single_winner() {
    let (app, pool) = app().await;
    let session = login(&app).await;
    island(&app, &session).await;
    set_level(&pool, &session, 5).await;
    give(&pool, &session, "plot", 2).await;
    let (a, b) = tokio::join!(place(&app, &session, 5, 5), place(&app, &session, 5, 5));
    let statuses = [a.0, b.0];
    assert!(statuses.contains(&StatusCode::OK), "{a:?} {b:?}");
    assert!(statuses.contains(&StatusCode::CONFLICT), "{a:?} {b:?}");
    let plots = island(&app, &session).await["plots"].as_array().unwrap().len();
    assert_eq!(plots, 7);
    let (_, inv) = get(&app, &session, "/inventory").await;
    assert_eq!(qty(&inv, "plot"), 1, "only the winner spent an item");
}

#[tokio::test]
async fn test_bed_plant_and_harvest_are_one_transaction() {
    let (app, pool) = app().await;
    let session = login(&app).await;
    let snapshot = island(&app, &session).await;
    let plot0 = snapshot["plots"][0]["id"].as_str().unwrap().to_owned();
    // Starter: 6 wheat seeds for a 6-plot bed. One seed short → nothing is planted.
    let (_, inv) = get(&app, &session, "/inventory").await;
    assert_eq!(qty(&inv, "seed_wheat"), 6);
    sqlx::query("UPDATE cf.inventory SET qty = 5 WHERE user_id = $1 AND item_id = 'seed_wheat'")
        .bind(Uuid::parse_str(&session.user_id).unwrap())
        .execute(&pool)
        .await
        .unwrap();
    let (status, body) = post(
        &app,
        &session,
        &format!("/plots/{plot0}/bed/plant"),
        Some(json!({ "item_id": "seed_wheat" })),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["error"]["code"], "insufficient_items");
    let after = island(&app, &session).await;
    assert!(
        after["plots"]
            .as_array()
            .unwrap()
            .iter()
            .all(|p| p["state"] == "empty"),
        "rolled back"
    );
    let (_, inv) = get(&app, &session, "/inventory").await;
    assert_eq!(qty(&inv, "seed_wheat"), 5, "no seed consumed on rollback");

    give(&pool, &session, "seed_wheat", 1).await;
    let (status, body) = post(
        &app,
        &session,
        &format!("/plots/{plot0}/bed/plant"),
        Some(json!({ "item_id": "seed_wheat" })),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["count"], 6);
    assert!(body["plots"]
        .as_array()
        .unwrap()
        .iter()
        .all(|p| p["state"] == "growing"));
    let (_, inv) = get(&app, &session, "/inventory").await;
    assert_eq!(qty(&inv, "seed_wheat"), 0);

    let (status, body) = post(&app, &session, &format!("/plots/{plot0}/bed/harvest"), None, None).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["error"]["code"], "not_ready");
    force_all_ready(&pool, &session).await;
    let (status, body) = post(&app, &session, &format!("/plots/{plot0}/bed/harvest"), None, None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["count"], 6);
    assert_eq!(
        body["items"][0]["item_id"], "crop_wheat",
        "produce summed per item (0008)"
    );
    assert_eq!(body["items"][0]["qty"], 6);
    let (_, inv) = get(&app, &session, "/inventory").await;
    assert_eq!(qty(&inv, "crop_wheat"), 6);
    assert!(body["plots"]
        .as_array()
        .unwrap()
        .iter()
        .all(|p| p["state"] == "empty"));
    // Nothing left to harvest.
    let (status, body) = post(&app, &session, &format!("/plots/{plot0}/bed/harvest"), None, None).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["error"]["code"], "wrong_state");
}
