//! Crops of migration 0018 (owner ok 2026-09-24, no level gate): each is in the catalog
//! with numbers on the economy's ladder, and a brand-new island can buy, plant, harvest
//! and sell one end to end. Against local Postgres + Memurai.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use axum::http::StatusCode;
use cf_ledger::{Currency, Entry, ACCOUNT_CREDITS, SYSTEM_MINT};
use common::{app, app_with, get, login, post};
use serde_json::{json, Value};
use uuid::Uuid;

/// (crop, seed price, grow seconds, sell credits, level) as migrations 0018 + 0019 leave them.
const NEW_CROPS: [(&str, i64, i64, i64, i64); 13] = [
    ("barley", 8, 2_700, 16, 1),
    ("oats", 10, 3_600, 21, 1),
    ("onion", 12, 4_500, 26, 1),
    ("potato", 14, 5_400, 30, 1),
    ("beet", 20, 9_000, 46, 1),
    ("corn", 26, 10_800, 62, 3),
    ("strawberry", 32, 10_800, 74, 3),
    ("cabbage", 38, 14_400, 90, 4),
    ("pepper", 48, 18_000, 115, 5),
    ("blueberry", 75, 28_800, 185, 7),
    ("melon", 85, 32_400, 210, 8),
    ("raspberry", 90, 36_000, 230, 9),
    ("watermelon", 160, 57_600, 415, 12),
];

fn item<'a>(items: &'a [Value], id: &str) -> Option<&'a Value> {
    items.iter().find(|i| i["id"] == id)
}

#[tokio::test]
async fn test_new_crops_are_listed_on_the_price_ladder() {
    let (app, _) = app().await;
    let session = login(&app).await;
    let (status, body) = get(&app, &session, "/catalog").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let items = body["items"].as_array().unwrap();
    for (crop, price, grow, sell, level) in NEW_CROPS {
        let seed = item(items, &format!("seed_{crop}")).unwrap_or_else(|| panic!("seed_{crop} listed"));
        assert_eq!(seed["kind"], "seed");
        assert_eq!(seed["price_credits"], price, "{crop} price");
        assert!(
            seed["price_usd_cents"].is_null(),
            "{crop} is sold for credits only"
        );
        assert_eq!(seed["exclusive"], false, "{crop} needs no reputation");
        // 0019: crops from three hours up wait for a level; the quick ones are open at once.
        if level > 1 {
            assert_eq!(seed["meta"]["level_min"], level, "{crop} level gate");
        } else {
            assert!(seed["meta"]["level_min"].is_null(), "{crop} is open at level 1");
        }
        assert_eq!(seed["meta"]["grow_secs"], grow, "{crop} grow time");
        assert_eq!(seed["meta"]["produce"], format!("crop_{crop}"));
        assert_eq!(seed["meta"]["credits"], sell, "harvest XP follows the sale value");
        let produce = item(items, &format!("crop_{crop}")).unwrap_or_else(|| panic!("crop_{crop} listed"));
        assert_eq!(produce["kind"], "produce");
        // No buy-back loop: produce is never for sale, only sold at the market.
        assert!(
            produce["price_credits"].is_null() && produce["price_usd_cents"].is_null(),
            "{crop} produce is not for sale"
        );
        assert_eq!(produce["meta"]["sell_credits"], sell);
        // The ladder: sale = 2.0-2.6 x seed price (integer basis points, no floats).
        let ratio_bp = sell * 10_000 / price;
        assert!(
            (20_000..=26_000).contains(&ratio_bp),
            "{crop} ratio {ratio_bp} bp"
        );
    }
}

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

#[tokio::test]
async fn test_level_one_island_grows_and_sells_a_new_crop() {
    let (app, pool) = app().await;
    let session = login(&app).await;
    let (_, island) = get(&app, &session, "/island").await;
    assert_eq!(island["island"]["level"], 1);
    give_credits(&pool, Uuid::parse_str(&session.user_id).unwrap(), 100).await;
    // Corn waits for level 3 (0019): the shop refuses first, then sells once the island is there.
    let (status, locked) = post(
        &app,
        &session,
        "/shop/buy-credits",
        Some(json!({ "item_id": "seed_corn", "qty": 1 })),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{locked}");
    assert_eq!(locked["error"]["code"], "level_locked");
    sqlx::query("UPDATE cf.islands SET level = 3 WHERE user_id = $1")
        .bind(Uuid::parse_str(&session.user_id).unwrap())
        .execute(&pool)
        .await
        .unwrap();
    let (status, bought) = post(
        &app,
        &session,
        "/shop/buy-credits",
        Some(json!({ "item_id": "seed_corn", "qty": 1 })),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{bought}");
    let plot_id = island["plots"][0]["id"].as_str().unwrap().to_owned();
    let (status, plot) = post(
        &app,
        &session,
        &format!("/plots/{plot_id}/plant"),
        Some(json!({ "item_id": "seed_corn" })),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{plot}");
    assert_eq!(plot["crop_id"], "seed_corn");
    sqlx::query("UPDATE cf.plots SET ready_at = now() - interval '1 second' WHERE id = $1")
        .bind(Uuid::parse_str(&plot_id).unwrap())
        .execute(&pool)
        .await
        .unwrap();
    let (status, harvest) = post(&app, &session, &format!("/plots/{plot_id}/harvest"), None, None).await;
    assert_eq!(status, StatusCode::OK, "{harvest}");
    assert_eq!(harvest["item_id"], "crop_corn");
    assert_eq!(harvest["xp"], 7, "62 credits of value / 10 plus 1 for planting");
    let (status, sale) = post(
        &app,
        &session,
        "/market/sell",
        Some(json!({ "item_id": "crop_corn", "qty": 1 })),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{sale}");
    assert_eq!(sale["credits_earned"], 62);
}

/// The loop that would mint credits must stay closed: produce cannot be bought, seeds
/// cannot be sold at the market, and the new seeds cannot be ordered for CLASH.
#[tokio::test]
async fn test_new_crops_cannot_be_looped_or_bought_for_clash() {
    let (app, pool) = app_with(true).await;
    let session = login(&app).await;
    get(&app, &session, "/island").await;
    give_credits(&pool, Uuid::parse_str(&session.user_id).unwrap(), 500).await;
    sqlx::query("UPDATE cf.islands SET level = 3 WHERE user_id = $1") // corn opens at level 3 (0019)
        .bind(Uuid::parse_str(&session.user_id).unwrap())
        .execute(&pool)
        .await
        .unwrap();
    let (status, body) = post(
        &app,
        &session,
        "/shop/buy-credits",
        Some(json!({ "item_id": "crop_corn", "qty": 1 })),
        None,
    )
    .await;
    assert!(
        status.is_client_error(),
        "produce is not for sale: {status} {body}"
    );
    let (status, _) = post(
        &app,
        &session,
        "/shop/buy-credits",
        Some(json!({ "item_id": "seed_corn", "qty": 1 })),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, body) = post(
        &app,
        &session,
        "/market/sell",
        Some(json!({ "item_id": "seed_corn", "qty": 1 })),
        None,
    )
    .await;
    assert!(
        status.is_client_error(),
        "seeds are not sold back: {status} {body}"
    );
    let (status, body) = post(
        &app,
        &session,
        "/orders",
        Some(json!({ "item_id": "seed_corn", "qty": 1 })),
        None,
    )
    .await;
    assert!(
        status.is_client_error(),
        "credit-only seeds have no CLASH price: {status} {body}"
    );
}
