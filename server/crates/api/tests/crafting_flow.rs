//! Crafting v1 (migration 0008): recipes list, crafting is all-or-nothing,
//! dishes sell above their inputs or feed XP, decor can be built from
//! materials, caps apply. Against local Postgres + Memurai.
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
async fn test_recipes_are_listed_with_inputs() {
    let (app, _) = app().await;
    let session = login(&app).await;
    let (status, body) = get(&app, &session, "/recipes").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let recipes = body["recipes"].as_array().unwrap();
    let bread = recipes.iter().find(|r| r["id"] == "bread").expect("bread recipe");
    assert_eq!(bread["output_item"], "dish_bread");
    assert_eq!(bread["inputs"][0]["item_id"], "crop_wheat");
    assert_eq!(bread["inputs"][0]["qty"], 3);
    assert!(recipes
        .iter()
        .any(|r| r["id"] == "fence" && r["output_item"] == "decor_fence"));
}

#[tokio::test]
async fn test_craft_is_all_or_nothing_and_awards_xp() {
    let (app, pool) = app().await;
    let session = login(&app).await;
    get(&app, &session, "/island").await;
    give(&pool, &session, "crop_wheat", 2).await;
    let (status, body) = post(
        &app,
        &session,
        "/craft",
        Some(json!({ "recipe_id": "bread" })),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["error"]["code"], "insufficient_items");
    let (_, inv) = get(&app, &session, "/inventory").await;
    assert_eq!(
        qty(&inv, "crop_wheat"),
        2,
        "nothing consumed when an input is short"
    );
    give(&pool, &session, "crop_wheat", 4).await;
    let (status, body) = post(
        &app,
        &session,
        "/craft",
        Some(json!({ "recipe_id": "bread", "times": 2 })),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["output_item"], "dish_bread");
    assert_eq!(body["qty"], 2);
    assert_eq!(body["xp"], 6, "3 xp per bread");
    let (_, inv) = get(&app, &session, "/inventory").await;
    assert_eq!(qty(&inv, "crop_wheat"), 0);
    assert_eq!(qty(&inv, "dish_bread"), 2);
    let (status, body) = post(
        &app,
        &session,
        "/craft",
        Some(json!({ "recipe_id": "nope" })),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(body["error"]["code"], "invalid_item");
}

#[tokio::test]
async fn test_dish_sells_above_inputs_or_feeds_xp() {
    let (app, pool) = app().await;
    let session = login(&app).await;
    get(&app, &session, "/island").await;
    give(&pool, &session, "dish_bread", 2).await;
    let (status, sale) = post(
        &app,
        &session,
        "/market/sell",
        Some(json!({ "item_id": "dish_bread", "qty": 1 })),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{sale}");
    assert_eq!(
        sale["credits_earned"], 40,
        "bread sells for 40, three wheat for 30"
    );
    let (status, meal) = post(&app, &session, "/items/dish_bread/eat", None, None).await;
    assert_eq!(status, StatusCode::OK, "{meal}");
    assert_eq!(meal["xp"], 4);
    let (_, inv) = get(&app, &session, "/inventory").await;
    assert_eq!(qty(&inv, "dish_bread"), 0);
    // Only dishes are food; produce and materials are not.
    let (status, body) = post(&app, &session, "/items/crop_wheat/eat", None, None).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    // Seeds and materials have no sell price.
    let (status, body) = post(
        &app,
        &session,
        "/market/sell",
        Some(json!({ "item_id": "seed_wheat", "qty": 1 })),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(body["error"]["code"], "invalid_item");
}

#[tokio::test]
async fn test_decor_can_be_built_from_materials() {
    let (app, pool) = app().await;
    let session = login(&app).await;
    get(&app, &session, "/island").await;
    give(&pool, &session, "wood", 10).await;
    give(&pool, &session, "stone", 4).await;
    let (status, body) = post(
        &app,
        &session,
        "/craft",
        Some(json!({ "recipe_id": "fence" })),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["output_item"], "decor_fence");
    let (_, inv) = get(&app, &session, "/inventory").await;
    assert_eq!(qty(&inv, "decor_fence"), 1);
    assert_eq!(qty(&inv, "wood"), 0);
    assert_eq!(qty(&inv, "stone"), 0);
}

#[tokio::test]
async fn test_quantities_out_of_range_are_rejected() {
    let (app, pool) = app().await;
    let session = login(&app).await;
    get(&app, &session, "/island").await;
    give(&pool, &session, "crop_wheat", 300).await;
    for qty in [0, 101] {
        let (status, body) = post(
            &app,
            &session,
            "/market/sell",
            Some(json!({ "item_id": "crop_wheat", "qty": qty })),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        assert_eq!(body["error"]["code"], "invalid_qty");
        let (status, body) = post(
            &app,
            &session,
            "/craft",
            Some(json!({ "recipe_id": "bread", "times": qty })),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
        assert_eq!(body["error"]["code"], "invalid_qty");
    }
    let (_, inv) = get(&app, &session, "/inventory").await;
    assert_eq!(qty(&inv, "crop_wheat"), 300, "nothing moved");
}

#[tokio::test]
async fn test_craft_cap_counts_units_and_sale_returns_unused_budget() {
    let (app, pool) = app().await;
    let session = login(&app).await;
    get(&app, &session, "/island").await;
    let user_id = Uuid::parse_str(&session.user_id).unwrap();
    give(&pool, &session, "crop_wheat", 9).await;
    sqlx::query("INSERT INTO cf.rate_events (user_id, action, day, count) VALUES ($1, 'craft', (now() AT TIME ZONE 'UTC')::date, 99)")
        .bind(user_id)
        .execute(&pool)
        .await
        .unwrap();
    let (status, body) = post(
        &app,
        &session,
        "/craft",
        Some(json!({ "recipe_id": "bread", "times": 3 })),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["qty"], 1, "only the one craft left in the day's budget");
    let (_, inv) = get(&app, &session, "/inventory").await;
    assert_eq!(qty(&inv, "crop_wheat"), 6, "inputs taken for one bread only");
    let (status, body) = post(
        &app,
        &session,
        "/craft",
        Some(json!({ "recipe_id": "bread" })),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "{body}");
    // Credits budget (650 a day at level 1, 0019): 48 left, carrots cost 38 → one sells and
    // the 10 unpaid credits stay available.
    give(&pool, &session, "crop_carrot", 2).await;
    sqlx::query("INSERT INTO cf.rate_events (user_id, action, day, count) VALUES ($1, 'credits', (now() AT TIME ZONE 'UTC')::date, 602)")
        .bind(user_id)
        .execute(&pool)
        .await
        .unwrap();
    let (status, sale) = post(
        &app,
        &session,
        "/market/sell",
        Some(json!({ "item_id": "crop_carrot", "qty": 2 })),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{sale}");
    assert_eq!(sale["qty_sold"], 1);
    let count: i32 = sqlx::query_scalar("SELECT count FROM cf.rate_events WHERE user_id = $1 AND action = 'credits' AND day = (now() AT TIME ZONE 'UTC')::date")
        .bind(user_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(
        count, 640,
        "day's count equals what was minted (602 + one carrot at 38), unused grant returned"
    );
    let (status, sale) = post(
        &app,
        &session,
        "/market/sell",
        Some(json!({ "item_id": "crop_wheat", "qty": 1 })),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{sale}");
    assert_eq!(
        sale["credits_earned"], 10,
        "the returned 10 credits still buy one wheat sale"
    );
}

#[tokio::test]
async fn test_parallel_crafts_with_inputs_for_one_single_winner() {
    let (app, pool) = app().await;
    let session = login(&app).await;
    get(&app, &session, "/island").await;
    give(&pool, &session, "crop_wheat", 3).await;
    let body = json!({ "recipe_id": "bread" });
    let (a, b) = tokio::join!(
        post(&app, &session, "/craft", Some(body.clone()), None),
        post(&app, &session, "/craft", Some(body.clone()), None)
    );
    let ok = [a.0, b.0].iter().filter(|s| **s == StatusCode::OK).count();
    assert_eq!(ok, 1, "{:?}", [a.0, b.0]);
    let (_, inv) = get(&app, &session, "/inventory").await;
    assert_eq!(qty(&inv, "dish_bread"), 1);
    assert_eq!(qty(&inv, "crop_wheat"), 0);
}

#[tokio::test]
async fn test_craft_replays_idempotently_without_a_second_output() {
    let (app, pool) = app().await;
    let session = login(&app).await;
    get(&app, &session, "/island").await;
    give(&pool, &session, "crop_wheat", 6).await;
    let key = Uuid::new_v4().to_string();
    let (status, first) = post(
        &app,
        &session,
        "/craft",
        Some(json!({ "recipe_id": "bread" })),
        Some(&key),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{first}");
    let (status, again) = post(
        &app,
        &session,
        "/craft",
        Some(json!({ "recipe_id": "bread" })),
        Some(&key),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{again}");
    assert_eq!(again["xp"], first["xp"], "replay returns the stored answer");
    let (_, inv) = get(&app, &session, "/inventory").await;
    assert_eq!(qty(&inv, "dish_bread"), 1, "no second bread on replay");
    assert_eq!(qty(&inv, "crop_wheat"), 3);
}
