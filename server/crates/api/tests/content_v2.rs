//! Content v2 (migration 0027, docs/CONTENT_V2.md): decor split by the owner's rule (simple for credits,
//! detailed for CLASH, some built from materials), twelve dishes priced above their inputs, a 45-second
//! tutorial crop in the starter kit, and the well and windmill reserving their real size.
//! Against local Postgres + Memurai.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use axum::http::StatusCode;
use common::{app, get, login, post};
use serde_json::{json, Value};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use uuid::Uuid;

const DECOR_FOR_CREDITS: [(&str, i64); 16] = [
    ("decor_bucket", 40),
    ("decor_watering_can", 40),
    ("decor_fence_cross", 50),
    ("decor_fence_picket", 60),
    ("decor_stool", 70),
    ("decor_milk_can", 80),
    ("decor_barrel_small", 90),
    ("decor_feeder", 90),
    ("decor_crate", 110),
    ("decor_haybale", 120),
    ("decor_chair", 140),
    ("decor_bench_stone", 150),
    ("decor_mailbox", 150),
    ("decor_lamp_short", 160),
    ("decor_wheelbarrow", 170),
    ("decor_table", 180),
];

/// (item, USD cents, level gate) — the popular, detailed pieces.
const DECOR_FOR_CLASH: [(&str, i64, i64); 11] = [
    ("decor_farm_sign", 190, 1),
    ("decor_coop", 390, 3),
    ("decor_pergola", 390, 4),
    ("decor_gazebo", 490, 4),
    ("decor_canopy", 490, 5),
    ("decor_shed", 590, 5),
    ("decor_tractor", 590, 6),
    ("decor_water_tower", 590, 7),
    ("decor_gazebo_large", 690, 8),
    ("decor_lookout_tower", 690, 9),
    ("decor_greenhouse", 790, 10),
];

const BUILD_ONLY: [&str; 4] = [
    "decor_firewood",
    "decor_signpost",
    "decor_bonfire",
    "decor_well_windlass",
];

/// (recipe, output item) added by 0027.
const RECIPES: [(&str, &str); 20] = [
    ("crate", "decor_crate"),
    ("firewood", "decor_firewood"),
    ("chair", "decor_chair"),
    ("table", "decor_table"),
    ("signpost", "decor_signpost"),
    ("bonfire", "decor_bonfire"),
    ("wheelbarrow", "decor_wheelbarrow"),
    ("well_windlass", "decor_well_windlass"),
    ("porridge", "dish_porridge"),
    ("baked_potato", "dish_baked_potato"),
    ("veggie_stew", "dish_veggie_stew"),
    ("herb_tea", "dish_herb_tea"),
    ("corn_bread", "dish_corn_bread"),
    ("mushroom_soup", "dish_mushroom_soup"),
    ("borscht", "dish_borscht"),
    ("stuffed_pepper", "dish_stuffed_pepper"),
    ("blueberry_muffin", "dish_blueberry_muffin"),
    ("berry_jam", "dish_berry_jam"),
    ("fruit_salad", "dish_fruit_salad"),
    ("mushroom_pie", "dish_mushroom_pie"),
];

fn item<'a>(items: &'a [Value], id: &str) -> &'a Value {
    items
        .iter()
        .find(|i| i["id"] == id)
        .unwrap_or_else(|| panic!("{id} listed"))
}

fn qty(inv: &Value, id: &str) -> i64 {
    inv["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["item_id"] == id)
        .map(|e| e["qty"].as_i64().unwrap())
        .unwrap_or(0)
}

async fn give(pool: &sqlx::PgPool, session: &common::Session, id: &str, qty: i64) {
    sqlx::query(
        "INSERT INTO cf.inventory (user_id, item_id, qty) VALUES ($1, $2, $3)
         ON CONFLICT (user_id, item_id) DO UPDATE SET qty = cf.inventory.qty + EXCLUDED.qty",
    )
    .bind(Uuid::parse_str(&session.user_id).unwrap())
    .bind(id)
    .bind(i32::try_from(qty).unwrap())
    .execute(pool)
    .await
    .unwrap();
}

#[tokio::test]
async fn test_decor_is_split_by_tier() {
    let (app, _) = app().await;
    let session = login(&app).await;
    let (status, body) = get(&app, &session, "/catalog").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let items = body["items"].as_array().unwrap();
    for (id, credits) in DECOR_FOR_CREDITS {
        let it = item(items, id);
        assert_eq!(it["kind"], "decor");
        assert_eq!(it["price_credits"], credits, "{id}");
        assert!(it["price_usd_cents"].is_null(), "{id} is simple: credits only");
        assert!(
            it["meta"]["footprint_dm"].as_i64().unwrap() > 0,
            "{id} reserves room"
        );
    }
    for (id, cents, level) in DECOR_FOR_CLASH {
        let it = item(items, id);
        assert_eq!(it["price_usd_cents"], cents, "{id}");
        assert!(it["price_credits"].is_null(), "{id} is premium: CLASH only");
        assert_eq!(it["exclusive"], true);
        if level > 1 {
            assert_eq!(it["meta"]["level_min"], level, "{id} level gate");
        }
    }
    for id in BUILD_ONLY {
        let it = item(items, id);
        assert!(
            it["price_credits"].is_null() && it["price_usd_cents"].is_null(),
            "{id} is only built"
        );
    }
    // The paid well is no longer a wind pump, and both big pieces reserve their real size.
    assert_eq!(item(items, "decor_well")["meta"]["footprint_dm"], 16);
    assert_eq!(item(items, "decor_windmill")["meta"]["footprint_dm"], 50);
}

#[tokio::test]
async fn test_every_new_recipe_builds_its_item_and_dishes_pay_more_than_inputs() {
    let (app, pool) = app().await;
    let session = login(&app).await;
    get(&app, &session, "/island").await;
    let (_, catalog) = get(&app, &session, "/catalog").await;
    let items = catalog["items"].as_array().unwrap();
    let (_, recipes) = get(&app, &session, "/recipes").await;
    let recipes = recipes["recipes"].as_array().unwrap();
    for (recipe_id, output) in RECIPES {
        let recipe = recipes
            .iter()
            .find(|r| r["id"] == recipe_id)
            .unwrap_or_else(|| panic!("{recipe_id} listed"));
        assert_eq!(recipe["output_item"], output);
        let mut input_value = 0;
        // Mainland goods (herbs, mushrooms) have no market price: those dishes are a credit tap bounded by the
        // gather, craft and credit caps (docs/ECONOMY.md DISHES v2), so only the xp band is checked for them.
        let mut all_priced = true;
        for input in recipe["inputs"].as_array().unwrap() {
            let id = input["item_id"].as_str().unwrap();
            let n = input["qty"].as_i64().unwrap();
            give(&pool, &session, id, n).await;
            match item(items, id)["meta"]["sell_credits"].as_i64() {
                Some(price) => input_value += n * price,
                None => all_priced = false,
            }
        }
        let (status, body) = post(
            &app,
            &session,
            "/craft",
            Some(json!({ "recipe_id": recipe_id })),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{recipe_id}: {body}");
        assert_eq!(body["output_item"], output);
        if output.starts_with("dish_") {
            let sell = item(items, output)["meta"]["sell_credits"].as_i64().unwrap();
            let xp = item(items, output)["meta"]["xp"].as_i64().unwrap();
            assert!(
                sell > input_value,
                "{output}: {sell} must beat the inputs ({input_value})"
            );
            if all_priced {
                assert!(
                    sell * 10 <= input_value * 14,
                    "{output}: {sell} at most +40 % over {input_value}"
                );
            }
            assert!(
                xp * 20 >= sell && xp * 5 <= sell,
                "{output}: xp {xp} between sale / 20 and sale / 5"
            );
        }
    }
    let (_, inv) = get(&app, &session, "/inventory").await;
    for (_, output) in RECIPES {
        assert!(qty(&inv, output) >= 1, "{output} in the inventory");
    }
}

#[tokio::test]
async fn test_new_island_harvests_a_radish_within_a_minute() {
    let (app, pool) = app().await;
    let session = login(&app).await;
    let (_, island) = get(&app, &session, "/island").await;
    let (_, inv) = get(&app, &session, "/inventory").await;
    assert_eq!(
        qty(&inv, "seed_radish"),
        4,
        "the starter kit holds the tutorial crop"
    );
    let plot_id = island["plots"][0]["id"].as_str().unwrap().to_owned();
    let (status, plot) = post(
        &app,
        &session,
        &format!("/plots/{plot_id}/plant"),
        Some(json!({ "item_id": "seed_radish" })),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{plot}");
    let planted = OffsetDateTime::parse(plot["planted_at"].as_str().unwrap(), &Rfc3339).unwrap();
    let ready = OffsetDateTime::parse(plot["ready_at"].as_str().unwrap(), &Rfc3339).unwrap();
    assert_eq!(
        (ready - planted).whole_seconds(),
        45,
        "radish grows in 45 s on the DB clock"
    );
    // Too early: the server refuses before the time is up.
    let (status, body) = post(&app, &session, &format!("/plots/{plot_id}/harvest"), None, None).await;
    assert_eq!(status, StatusCode::CONFLICT, "no harvest before ready_at: {body}");
    assert_eq!(body["error"]["code"], "not_ready");
    sqlx::query("UPDATE cf.plots SET ready_at = now() - interval '1 second' WHERE id = $1::uuid")
        .bind(&plot_id)
        .execute(&pool)
        .await
        .unwrap();
    let (status, body) = post(&app, &session, &format!("/plots/{plot_id}/harvest"), None, None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["item_id"], "crop_radish");
}

#[tokio::test]
async fn test_decor_never_trades_back_into_credits() {
    let (app, pool) = app().await;
    let session = login(&app).await;
    get(&app, &session, "/island").await;
    // CLASH decor and build-only decor have no credit price: the credit shop refuses them.
    for id in [
        "decor_gazebo",
        "decor_greenhouse",
        "decor_bonfire",
        "decor_well_windlass",
    ] {
        let (status, body) = post(
            &app,
            &session,
            "/shop/buy-credits",
            Some(json!({ "item_id": id, "qty": 1 })),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{id}: {body}");
        assert_eq!(body["error"]["code"], "invalid_item");
    }
    // No decor sells on the market, bought, built or paid: buying and selling never prints credits.
    for id in ["decor_bucket", "decor_table", "decor_bonfire", "decor_farm_sign"] {
        give(&pool, &session, id, 1).await;
        let (status, body) = post(
            &app,
            &session,
            "/market/sell",
            Some(json!({ "item_id": id, "qty": 1 })),
            None,
        )
        .await;
        assert_eq!(status, StatusCode::BAD_REQUEST, "{id}: {body}");
        assert_eq!(body["error"]["code"], "invalid_item");
    }
}

#[tokio::test]
async fn test_radish_ignores_the_plant_and_harvest_caps() {
    let (app, pool) = app().await;
    let session = login(&app).await;
    let (_, island) = get(&app, &session, "/island").await;
    let user_id = Uuid::parse_str(&session.user_id).unwrap();
    // Both daily caps already spent (40 each by default).
    for action in ["plant", "harvest"] {
        sqlx::query(
            "INSERT INTO cf.rate_events (user_id, action, day, count) VALUES ($1, $2, (now() AT TIME ZONE 'UTC')::date, 40)",
        )
        .bind(user_id)
        .bind(action)
        .execute(&pool)
        .await
        .unwrap();
    }
    let plot_id = island["plots"][0]["id"].as_str().unwrap().to_owned();
    let (status, body) = post(
        &app,
        &session,
        &format!("/plots/{plot_id}/plant"),
        Some(json!({ "item_id": "seed_radish" })),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "radish is outside the plant cap: {body}");
    sqlx::query("UPDATE cf.plots SET ready_at = now() - interval '1 second' WHERE id = $1::uuid")
        .bind(&plot_id)
        .execute(&pool)
        .await
        .unwrap();
    let (status, body) = post(&app, &session, &format!("/plots/{plot_id}/harvest"), None, None).await;
    assert_eq!(
        status,
        StatusCode::OK,
        "radish is outside the harvest cap: {body}"
    );
    // Any other crop still hits the cap, and the message says when it resets.
    let (status, body) = post(
        &app,
        &session,
        &format!("/plots/{plot_id}/plant"),
        Some(json!({ "item_id": "seed_wheat" })),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "{body}");
    assert_eq!(body["error"]["code"], "daily_cap");
    assert!(
        body["error"]["message"].as_str().unwrap().contains("resets in"),
        "{body}"
    );
}
