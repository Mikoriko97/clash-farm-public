//! Ledger/economy integration tests (rules/server-ledger-world.md): double entry,
//! no negative balances, append-only history, credits cap, purchase race, decor.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use axum::http::StatusCode;
use cf_ledger::{Currency, Entry, ACCOUNT_CREDITS, SYSTEM_MINT, SYSTEM_SINK};
use common::{app, get, login, post};
use serde_json::json;
use uuid::Uuid;

/// economy.starter_credits (migration 0005): every new island starts with these.
const STARTER_CREDITS: i64 = 30;

async fn give_credits(pool: &sqlx::PgPool, user_id: Uuid, amount: i64) {
    let mut tx = pool.begin().await.unwrap();
    cf_ledger::post(
        &mut tx,
        "test_grant",
        Uuid::new_v4(),
        &[
            Entry::system(SYSTEM_MINT, Currency::Credits, -amount),
            Entry::user(user_id, ACCOUNT_CREDITS, Currency::Credits, amount),
        ],
    )
    .await
    .unwrap();
    tx.commit().await.unwrap();
}

async fn ready_plot(app: &axum::Router, pool: &sqlx::PgPool, session: &common::Session) -> String {
    let (_, snapshot) = get(app, session, "/island").await;
    let plot_id = snapshot["plots"][0]["id"].as_str().unwrap().to_owned();
    post(
        app,
        session,
        &format!("/plots/{plot_id}/plant"),
        Some(json!({ "item_id": "seed_wheat" })),
        None,
    )
    .await;
    sqlx::query("UPDATE cf.plots SET ready_at = now() - interval '1 second' WHERE id = $1")
        .bind(Uuid::parse_str(&plot_id).unwrap())
        .execute(pool)
        .await
        .unwrap();
    plot_id
}

#[tokio::test]
async fn test_double_entry_sums_to_zero() {
    let (app, pool) = app().await;
    let session = login(&app).await;
    let user_id = Uuid::parse_str(&session.user_id).unwrap();
    let plot_id = ready_plot(&app, &pool, &session).await;
    let (status, result) = post(&app, &session, &format!("/plots/{plot_id}/harvest"), None, None).await;
    assert_eq!(status, StatusCode::OK, "{result}");
    assert_eq!(result["item_id"], "crop_wheat");
    // Credits are minted by selling the produce (0008), same value as the old harvest reward.
    let (status, sale) = post(
        &app,
        &session,
        "/market/sell",
        Some(json!({ "item_id": "crop_wheat", "qty": 1 })),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{sale}");
    assert_eq!(sale["credits_earned"], 10);
    assert_eq!(sale["credits"], STARTER_CREDITS + 10);
    let (_, me) = get(&app, &session, "/me").await;
    assert_eq!(me["balances"][0]["account"], "credits");
    assert_eq!(me["balances"][0]["amount"], STARTER_CREDITS + 10);
    // Every ref_id balances to zero and the user's ledger sum equals the materialised balance.
    let unbalanced: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM (SELECT ref_id, currency, sum(delta) s FROM cf.ledger GROUP BY ref_id, currency) t WHERE s <> 0",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(unbalanced, 0);
    let ledger_sum: i64 = sqlx::query_scalar(
        "SELECT coalesce(sum(delta), 0)::bigint FROM cf.ledger WHERE user_id = $1 AND account = 'credits'",
    )
    .bind(user_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(ledger_sum, STARTER_CREDITS + 10);
    // An unbalanced posting is refused before touching the database.
    let mut tx = pool.begin().await.unwrap();
    let err = cf_ledger::post(
        &mut tx,
        "bad",
        Uuid::new_v4(),
        &[
            Entry::system(SYSTEM_MINT, Currency::Credits, -5),
            Entry::user(user_id, ACCOUNT_CREDITS, Currency::Credits, 6),
        ],
    )
    .await
    .unwrap_err();
    assert!(matches!(err, cf_ledger::LedgerError::Unbalanced(_)));
}

#[tokio::test]
async fn test_negative_balance_impossible() {
    let (app, pool) = app().await;
    let session = login(&app).await;
    let user_id = Uuid::parse_str(&session.user_id).unwrap();
    get(&app, &session, "/island").await;
    // 5 credits, wheat seed costs 5: one is affordable, two are not.
    give_credits(&pool, user_id, 5 - STARTER_CREDITS).await; // exactly 5 left
    let (status, body) = post(
        &app,
        &session,
        "/shop/buy-credits",
        Some(json!({ "item_id": "seed_wheat", "qty": 2 })),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["error"]["code"], "insufficient_funds");
    let (_, inv) = get(&app, &session, "/inventory").await;
    assert_eq!(wheat(&inv), 6, "nothing added on refusal");
    let (status, purchase) = post(
        &app,
        &session,
        "/shop/buy-credits",
        Some(json!({ "item_id": "seed_wheat", "qty": 1 })),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{purchase}");
    assert_eq!(purchase["credits_left"], 0);
    let (_, inv) = get(&app, &session, "/inventory").await;
    assert_eq!(wheat(&inv), 7);
    // Direct posting below zero is rejected by the database CHECK.
    let mut tx = pool.begin().await.unwrap();
    let err = cf_ledger::post(
        &mut tx,
        "test",
        Uuid::new_v4(),
        &[
            Entry::user(user_id, ACCOUNT_CREDITS, Currency::Credits, -1),
            Entry::system(SYSTEM_SINK, Currency::Credits, 1),
        ],
    )
    .await
    .unwrap_err();
    assert!(matches!(err, cf_ledger::LedgerError::InsufficientFunds));
}

#[tokio::test]
async fn test_ledger_is_append_only() {
    let (_, pool) = app().await;
    let session_user: Uuid = sqlx::query_scalar("SELECT id FROM cf.users LIMIT 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    give_credits(&pool, session_user, 1).await;
    let update = sqlx::query("UPDATE cf.ledger SET delta = delta + 1 WHERE user_id = $1")
        .bind(session_user)
        .execute(&pool)
        .await;
    assert!(update.is_err(), "UPDATE on ledger must be refused by the trigger");
    let delete = sqlx::query("DELETE FROM cf.ledger WHERE user_id = $1")
        .bind(session_user)
        .execute(&pool)
        .await;
    assert!(delete.is_err(), "DELETE on ledger must be refused by the trigger");
}

#[tokio::test]
async fn test_credits_daily_cap_pays_partial() {
    let (app, pool) = app().await;
    let session = login(&app).await;
    let user_id = Uuid::parse_str(&session.user_id).unwrap();
    let plot_id = ready_plot(&app, &pool, &session).await;
    let (status, result) = post(&app, &session, &format!("/plots/{plot_id}/harvest"), None, None).await;
    assert_eq!(status, StatusCode::OK, "{result}");
    assert_eq!(
        result["plot"]["state"], "empty",
        "harvest always completes: produce is not capped"
    );
    sqlx::query("INSERT INTO cf.inventory (user_id, item_id, qty) VALUES ($1, 'crop_wheat', 2) ON CONFLICT (user_id, item_id) DO UPDATE SET qty = cf.inventory.qty + 2")
        .bind(user_id)
        .execute(&pool)
        .await
        .unwrap();
    // 635 of the level-1 cap (650, 0019) earned today: 15 credits of budget cover one wheat (10), not three.
    sqlx::query("INSERT INTO cf.rate_events (user_id, action, day, count) VALUES ($1, 'credits', (now() AT TIME ZONE 'UTC')::date, 635)")
        .bind(user_id)
        .execute(&pool)
        .await
        .unwrap();
    let (status, sale) = post(
        &app,
        &session,
        "/market/sell",
        Some(json!({ "item_id": "crop_wheat", "qty": 3 })),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{sale}");
    assert_eq!(
        sale["qty_sold"], 1,
        "only whole units the daily budget covers are sold"
    );
    assert_eq!(sale["credits_earned"], 10);
    assert_eq!(sale["credits"], STARTER_CREDITS + 10);
    let (status, body) = post(
        &app,
        &session,
        "/market/sell",
        Some(json!({ "item_id": "crop_wheat", "qty": 1 })),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "{body}");
    assert_eq!(body["error"]["code"], "daily_cap");
}

#[tokio::test]
async fn test_buy_race_single_winner() {
    let (app, pool) = app().await;
    let session = login(&app).await;
    let user_id = Uuid::parse_str(&session.user_id).unwrap();
    get(&app, &session, "/island").await;
    give_credits(&pool, user_id, 120).await; // exactly one decor_fence
    let body = || Some(json!({ "item_id": "decor_fence", "qty": 1 }));
    let (a, b, c) = tokio::join!(
        post(&app, &session, "/shop/buy-credits", body(), None),
        post(&app, &session, "/shop/buy-credits", body(), None),
        post(&app, &session, "/shop/buy-credits", body(), None)
    );
    let ok = [a.0, b.0, c.0].iter().filter(|s| **s == StatusCode::OK).count();
    assert_eq!(ok, 1, "one purchase wins: {:?}", [a.0, b.0, c.0]);
    assert_eq!(
        cf_ledger::balance(&pool, user_id, ACCOUNT_CREDITS).await.unwrap(),
        STARTER_CREDITS
    );
    let (_, inv) = get(&app, &session, "/inventory").await;
    let fences = inv["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["item_id"] == "decor_fence")
        .map(|i| i["qty"].clone());
    assert_eq!(fences, Some(json!(1)));
}

#[tokio::test]
async fn test_decor_place_and_remove() {
    let (app, pool) = app().await;
    let session = login(&app).await;
    let user_id = Uuid::parse_str(&session.user_id).unwrap();
    get(&app, &session, "/island").await;
    give_credits(&pool, user_id, 240).await;
    let (status, _) = post(
        &app,
        &session,
        "/shop/buy-credits",
        Some(json!({ "item_id": "decor_fence", "qty": 2 })),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, placed) = post(
        &app,
        &session,
        "/decor/place",
        Some(json!({ "item_id": "decor_fence", "x_dm": 80, "z_dm": 80, "yaw_deg": 45 })),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{placed}");
    let id = placed["id"].as_str().unwrap().to_owned();
    let (status, body) = post(
        &app,
        &session,
        "/decor/place",
        Some(json!({ "item_id": "decor_fence", "x_dm": 90, "z_dm": 80, "yaw_deg": 0 })),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["error"]["code"], "overlap");
    let (status, body) = post(
        &app,
        &session,
        "/decor/place",
        Some(json!({ "item_id": "seed_wheat", "x_dm": -80, "z_dm": 80 })),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    let (_, snapshot) = get(&app, &session, "/island").await;
    assert_eq!(
        snapshot["decor"],
        json!([{ "id": id, "item_id": "decor_fence", "x_dm": 80, "z_dm": 80, "yaw_deg": 45 }])
    );
    let (status, _) = post(&app, &session, "/decor/remove", Some(json!({ "id": id })), None).await;
    assert_eq!(status, StatusCode::OK);
    let (status, body) = post(&app, &session, "/decor/remove", Some(json!({ "id": id })), None).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "{body}");
    let (_, inv) = get(&app, &session, "/inventory").await;
    let fences = inv["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["item_id"] == "decor_fence")
        .map(|i| i["qty"].clone());
    assert_eq!(fences, Some(json!(2)), "both fences back in inventory");
}

#[tokio::test]
async fn test_shop_rejects_bad_qty_and_exclusive_without_reputation() {
    let (app, pool) = app().await;
    let session = login(&app).await;
    let user_id = Uuid::parse_str(&session.user_id).unwrap();
    get(&app, &session, "/island").await;
    give_credits(&pool, user_id, 100_000).await;
    for qty in [0, -1, 101] {
        let (status, body) = post(
            &app,
            &session,
            "/shop/buy-credits",
            Some(json!({ "item_id": "seed_wheat", "qty": qty })),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "qty {qty}: {body}");
        assert_eq!(body["error"]["code"], "invalid_qty");
    }
    let (status, body) = post(
        &app,
        &session,
        "/shop/buy-credits",
        Some(json!({ "item_id": "seed_clashberry", "qty": 1 })),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(
        body["error"]["code"], "level_locked",
        "clashberry waits for level 14 (0019)"
    );
    let (status, body) = post(
        &app,
        &session,
        "/shop/buy-credits",
        Some(json!({ "item_id": "fert_premium", "qty": 1 })),
        None,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "CLASH-only item is not sold for credits: {body}"
    );
    assert_eq!(
        cf_ledger::balance(&pool, user_id, ACCOUNT_CREDITS).await.unwrap(),
        100_000 + STARTER_CREDITS,
        "nothing charged"
    );
}

#[tokio::test]
async fn test_credits_cap_holds_under_parallel_harvests() {
    let (app, pool) = app().await;
    let session = login(&app).await;
    let user_id = Uuid::parse_str(&session.user_id).unwrap();
    let (_, snapshot) = get(&app, &session, "/island").await;
    let ids: Vec<String> = snapshot["plots"]
        .as_array()
        .unwrap()
        .iter()
        .take(3)
        .map(|p| p["id"].as_str().unwrap().to_owned())
        .collect();
    for id in &ids {
        post(
            &app,
            &session,
            &format!("/plots/{id}/plant"),
            Some(json!({ "item_id": "seed_wheat" })),
            None,
        )
        .await;
        sqlx::query("UPDATE cf.plots SET ready_at = now() - interval '1 second' WHERE id = $1")
            .bind(Uuid::parse_str(id).unwrap())
            .execute(&pool)
            .await
            .unwrap();
    }
    for id in &ids {
        let (status, body) = post(&app, &session, &format!("/plots/{id}/harvest"), None, None).await;
        assert_eq!(status, StatusCode::OK, "{body}");
    }
    // 635 of 650 already earned today: 15 credits of budget for three parallel 10-credit sales → one sells.
    sqlx::query("INSERT INTO cf.rate_events (user_id, action, day, count) VALUES ($1, 'credits', (now() AT TIME ZONE 'UTC')::date, 635)")
        .bind(user_id)
        .execute(&pool)
        .await
        .unwrap();
    let body = json!({ "item_id": "crop_wheat", "qty": 1 });
    let (a, b, c) = tokio::join!(
        post(&app, &session, "/market/sell", Some(body.clone()), None),
        post(&app, &session, "/market/sell", Some(body.clone()), None),
        post(&app, &session, "/market/sell", Some(body.clone()), None)
    );
    let ok = [a.0, b.0, c.0].iter().filter(|s| **s == StatusCode::OK).count();
    assert_eq!(ok, 1, "{:?}", [a.0, b.0, c.0]);
    assert_eq!(
        cf_ledger::balance(&pool, user_id, ACCOUNT_CREDITS).await.unwrap(),
        STARTER_CREDITS + 10,
        "cap never overpaid"
    );
}

/// Wheat count from an /inventory response (starter inventory also holds carrots).
fn wheat(inv: &serde_json::Value) -> i64 {
    inv["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["item_id"] == "seed_wheat")
        .map(|i| i["qty"].as_i64().unwrap())
        .unwrap_or(0)
}
