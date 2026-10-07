//! Economy v2 (migration 0019, docs/ECONOMY_LAB.md §4): the daily credits cap follows
//! the island level, long crops and expansion permits wait for a level, planting gives
//! XP, extra plots cost more each, and a stuck island gets its seed stipend.
//! Against local Postgres + Memurai.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use axum::http::StatusCode;
use cf_ledger::{Currency, Entry, ACCOUNT_CREDITS, SYSTEM_MINT};
use common::{app, app_with, get, login, post};
use serde_json::{json, Value};
use uuid::Uuid;

fn uid(session: &common::Session) -> Uuid {
    Uuid::parse_str(&session.user_id).unwrap()
}

async fn set_level(pool: &sqlx::PgPool, session: &common::Session, level: i32) {
    sqlx::query("UPDATE cf.islands SET level = $2 WHERE user_id = $1")
        .bind(uid(session))
        .bind(level)
        .execute(pool)
        .await
        .unwrap();
}

async fn give(pool: &sqlx::PgPool, session: &common::Session, item: &str, qty: i32) {
    sqlx::query(
        "INSERT INTO cf.inventory (user_id, item_id, qty) VALUES ($1, $2, $3)
         ON CONFLICT (user_id, item_id) DO UPDATE SET qty = cf.inventory.qty + EXCLUDED.qty",
    )
    .bind(uid(session))
    .bind(item)
    .bind(qty)
    .execute(pool)
    .await
    .unwrap();
}

async fn give_credits(pool: &sqlx::PgPool, session: &common::Session, amount: i64) {
    let mut tx = pool.begin().await.unwrap();
    cf_ledger::post(
        &mut tx,
        "test_grant",
        Uuid::new_v4(),
        &[
            Entry::system(SYSTEM_MINT, Currency::Credits, -amount),
            Entry::user(uid(session), ACCOUNT_CREDITS, Currency::Credits, amount),
        ],
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
}

async fn buy(app: &axum::Router, session: &common::Session, item: &str, qty: i32) -> (StatusCode, Value) {
    post(
        app,
        session,
        "/shop/buy-credits",
        Some(json!({ "item_id": item, "qty": qty })),
        None,
    )
    .await
}

async fn sell(app: &axum::Router, session: &common::Session, item: &str, qty: i32) -> (StatusCode, Value) {
    post(
        app,
        session,
        "/market/sell",
        Some(json!({ "item_id": item, "qty": qty })),
        None,
    )
    .await
}

fn qty_of(inv: &Value, item: &str) -> i64 {
    inv["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["item_id"] == item)
        .and_then(|i| i["qty"].as_i64())
        .unwrap_or(0)
}

#[tokio::test]
async fn test_credits_cap_grows_with_the_island_level() {
    let (app, pool) = app().await;
    let session = login(&app).await;
    get(&app, &session, "/island").await;
    give(&pool, &session, "crop_wheat", 300).await;
    // Level 1: 500 + 150 × 1 = 650 credits a day = 65 wheat.
    let (status, sale) = sell(&app, &session, "crop_wheat", 100).await;
    assert_eq!(status, StatusCode::OK, "{sale}");
    assert_eq!(sale["qty_sold"], 65);
    assert_eq!(sale["credits_earned"], 650);
    let (status, body) = sell(&app, &session, "crop_wheat", 1).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "{body}");
    assert_eq!(body["error"]["code"], "daily_cap");
    // Level 10 lifts today's cap to 2 000: another 100 wheat (1 000) fit into the 1 350 left.
    set_level(&pool, &session, 10).await;
    let (status, sale) = sell(&app, &session, "crop_wheat", 100).await;
    assert_eq!(status, StatusCode::OK, "{sale}");
    assert_eq!(sale["credits_earned"], 1_000);
    let (status, sale) = sell(&app, &session, "crop_wheat", 100).await;
    assert_eq!(status, StatusCode::OK, "{sale}");
    assert_eq!(sale["credits_earned"], 350, "the rest of the day's budget");
}

#[tokio::test]
async fn test_long_crops_wait_for_a_level_in_shop_and_plot() {
    let (app, pool) = app().await;
    let session = login(&app).await;
    let (_, island) = get(&app, &session, "/island").await;
    give_credits(&pool, &session, 1_000).await;
    let (status, body) = buy(&app, &session, "seed_tomato", 1).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(body["error"]["code"], "level_locked");
    assert_eq!(body["error"]["message"], "requires island level 5");
    let (status, body) = buy(&app, &session, "seed_wheat", 1).await;
    assert_eq!(status, StatusCode::OK, "quick crops stay open: {body}");
    set_level(&pool, &session, 5).await;
    let (status, body) = buy(&app, &session, "seed_tomato", 1).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    // A seed in the bag does not bypass the gate when planting.
    give(&pool, &session, "seed_pumpkin", 1).await;
    let plot_id = island["plots"][0]["id"].as_str().unwrap().to_owned();
    let (status, body) = post(
        &app,
        &session,
        &format!("/plots/{plot_id}/plant"),
        Some(json!({ "item_id": "seed_pumpkin" })),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(body["error"]["code"], "level_locked");
}

#[tokio::test]
async fn test_planting_gives_xp_and_extra_plots_cost_more_each() {
    let (app, pool) = app().await;
    let session = login(&app).await;
    let (_, island) = get(&app, &session, "/island").await;
    let plot_id = island["plots"][0]["id"].as_str().unwrap().to_owned();
    let (status, body) = post(
        &app,
        &session,
        &format!("/plots/{plot_id}/plant"),
        Some(json!({ "item_id": "seed_wheat" })),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (_, snapshot) = get(&app, &session, "/island").await;
    assert_eq!(snapshot["island"]["xp"], 1, "one XP per planting (0019)");
    // Level 1 holds only the six starters: a plot it could never place is not sold.
    give_credits(&pool, &session, 5_000).await;
    let (status, body) = buy(&app, &session, "plot", 1).await;
    assert_eq!(
        (status, body["error"]["code"].clone()),
        (StatusCode::CONFLICT, json!("plot_limit")),
        "{body}"
    );
    assert_eq!(snapshot["plot_limit"], 6);
    // Level 10 holds 11. Starters are six: the 7th plot costs 200, the 8th 300, then two more 400 + 500.
    set_level(&pool, &session, 10).await;
    let (status, first) = buy(&app, &session, "plot", 1).await;
    assert_eq!(status, StatusCode::OK, "{first}");
    assert_eq!(first["spent_credits"], 200);
    let (_, snapshot) = get(&app, &session, "/island").await;
    assert_eq!(snapshot["plot_price"], 300, "the snapshot quotes the next plot");
    let (status, second) = buy(&app, &session, "plot", 1).await;
    assert_eq!(status, StatusCode::OK, "{second}");
    assert_eq!(second["spent_credits"], 300);
    let (status, pair) = buy(&app, &session, "plot", 2).await;
    assert_eq!(status, StatusCode::OK, "{pair}");
    assert_eq!(pair["spent_credits"], 900);
    let (_, inv) = get(&app, &session, "/inventory").await;
    assert_eq!(qty_of(&inv, "plot"), 4);
    let (_, snapshot) = get(&app, &session, "/island").await;
    assert_eq!(snapshot["plot_limit"], 11);
    let (status, body) = buy(&app, &session, "plot", 2).await;
    assert_eq!(
        body["error"]["code"], "plot_limit",
        "10 owned + 2 > 11: {status} {body}"
    );
    let (status, body) = buy(&app, &session, "plot", 1).await;
    assert_eq!(status, StatusCode::OK, "the 11th still fits: {body}");
}

#[tokio::test]
async fn test_expansion_permit_waits_for_the_level() {
    let (app, pool) = app().await;
    let session = login(&app).await;
    get(&app, &session, "/island").await;
    give(&pool, &session, "island_expand_1", 1).await;
    let (status, body) = post(
        &app,
        &session,
        "/island/expand",
        Some(json!({ "side": "n" })),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(body["error"]["code"], "level_locked");
    let (_, inv) = get(&app, &session, "/inventory").await;
    assert_eq!(qty_of(&inv, "island_expand_1"), 1, "the permit is kept");
    set_level(&pool, &session, 5).await;
    let (status, body) = post(
        &app,
        &session,
        "/island/expand",
        Some(json!({ "side": "n" })),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["island"]["expand_n"], 1);
}

#[tokio::test]
async fn test_stuck_island_gets_a_seed_stipend_once_a_day() {
    let (app, pool) = app().await;
    let session = login(&app).await;
    get(&app, &session, "/island").await;
    // Not stuck: the starter seeds are there.
    let (_, inv) = get(&app, &session, "/inventory").await;
    assert_eq!(qty_of(&inv, "seed_wheat"), 6);
    // Spend everything: no seeds, nothing growing, no credits.
    sqlx::query("DELETE FROM cf.inventory WHERE user_id = $1")
        .bind(uid(&session))
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE cf.balances SET amount = 0 WHERE user_id = $1 AND account = 'credits'")
        .bind(uid(&session))
        .execute(&pool)
        .await
        .unwrap();
    get(&app, &session, "/island").await;
    let (_, inv) = get(&app, &session, "/inventory").await;
    assert_eq!(
        qty_of(&inv, "seed_wheat"),
        3,
        "three wheat seeds to restart the loop"
    );
    // Plant one so the day's stipend cannot be farmed by emptying the bag again.
    sqlx::query("DELETE FROM cf.inventory WHERE user_id = $1")
        .bind(uid(&session))
        .execute(&pool)
        .await
        .unwrap();
    get(&app, &session, "/island").await;
    let (_, inv) = get(&app, &session, "/inventory").await;
    assert_eq!(qty_of(&inv, "seed_wheat"), 0, "once a day");
    let events: i64 =
        sqlx::query_scalar("SELECT count(*) FROM cf.events WHERE user_id = $1 AND name = 'stipend'")
            .bind(uid(&session))
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(events, 1);
}

#[tokio::test]
async fn test_locked_permit_is_not_even_quoted() {
    let (app, pool) = app_with(true).await;
    let session = login(&app).await;
    get(&app, &session, "/island").await;
    let order = json!({ "item_id": "island_expand_1", "qty": 1 });
    let (status, body) = post(&app, &session, "/orders", Some(order.clone()), None).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(body["error"]["code"], "level_locked");
    set_level(&pool, &session, 5).await;
    let (status, body) = post(&app, &session, "/orders", Some(order), None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
}

#[tokio::test]
async fn test_parallel_plot_purchases_climb_the_price_ladder() {
    let (app, pool) = app().await;
    let session = login(&app).await;
    get(&app, &session, "/island").await;
    give_credits(&pool, &session, 5_000).await;
    set_level(&pool, &session, 10).await;
    let (a, b, c) = tokio::join!(
        buy(&app, &session, "plot", 1),
        buy(&app, &session, "plot", 1),
        buy(&app, &session, "plot", 1)
    );
    let mut spent: Vec<i64> = [a, b, c]
        .iter()
        .map(|(status, body)| {
            assert_eq!(*status, StatusCode::OK, "{body}");
            body["spent_credits"].as_i64().unwrap()
        })
        .collect();
    spent.sort_unstable();
    assert_eq!(spent, vec![200, 300, 400], "each purchase sees the previous ones");
}
