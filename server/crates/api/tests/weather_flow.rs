//! Shared weather and lightning: the schedule comes from the database clock,
//! `weather.force` pins it, a storm strikes an island once per slot and drops
//! wood, outside a storm the call is refused. Against local Postgres + Memurai.
//! One test function: the forced state lives in the shared `settings` table.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use axum::http::StatusCode;
use common::{app, get, login, post};
use serde_json::json;

async fn force(pool: &sqlx::PgPool, state: Option<&str>) {
    match state {
        Some(s) => {
            sqlx::query(
                "INSERT INTO cf.settings (key, value) VALUES ('weather.force', $1)
                 ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value",
            )
            .bind(json!(s))
            .execute(pool)
            .await
            .unwrap();
        }
        None => {
            sqlx::query("DELETE FROM cf.settings WHERE key = 'weather.force'")
                .execute(pool)
                .await
                .unwrap();
        }
    }
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
async fn test_weather_schedule_and_lightning() {
    let (app, pool) = app().await;
    force(&pool, Some("storm")).await;
    // A fresh app loads params on its first request, so the pin is visible at once.
    let (app, _) = app_reloaded(app).await;
    let session = login(&app).await;
    let (status, snapshot) = get(&app, &session, "/island").await;
    assert_eq!(status, StatusCode::OK, "{snapshot}");
    assert_eq!(snapshot["weather"]["state"], "storm");
    let (status, weather) = get(&app, &session, "/weather").await;
    assert_eq!(status, StatusCode::OK, "{weather}");
    assert_eq!(weather["state"], "storm");
    assert!(weather["until"].as_str().unwrap() > weather["since"].as_str().unwrap());
    let slot = weather["slot"].as_i64().unwrap();

    let (status, hit) = post(&app, &session, "/weather/lightning", None, None).await;
    assert_eq!(status, StatusCode::OK, "{hit}");
    assert_eq!(hit["struck"], true);
    assert_eq!(hit["slot"], slot);
    assert_eq!(hit["item_id"], "wood");
    assert_eq!(hit["qty"], 3, "ECONOMY: LIGHTNING_WOOD=3");
    let idx = hit["resource_idx"].as_i64().unwrap();
    let trees: Vec<i64> = snapshot["resources"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|r| r["kind"] == "tree")
        .map(|r| r["idx"].as_i64().unwrap())
        .collect();
    assert!(
        trees.contains(&idx),
        "struck one of the island's trees: {idx} in {trees:?}"
    );
    let (_, inv) = get(&app, &session, "/inventory").await;
    assert_eq!(qty(&inv, "wood"), 3);
    // Same storm slot again: nothing more falls.
    let (status, again) = post(&app, &session, "/weather/lightning", None, None).await;
    assert_eq!(status, StatusCode::OK, "{again}");
    assert_eq!(again["struck"], false);
    let (_, inv) = get(&app, &session, "/inventory").await;
    assert_eq!(qty(&inv, "wood"), 3, "no double drop");

    // Clear skies: the request is refused and nothing changes.
    force(&pool, Some("clear")).await;
    let (app, _) = app_reloaded(app).await;
    let (status, body) = post(&app, &session, "/weather/lightning", None, None).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["error"]["code"], "no_storm");
    let (_, weather) = get(&app, &session, "/weather").await;
    assert_eq!(weather["state"], "clear");
    force(&pool, None).await;
}

/// Params are cached per app for 30 s: a new app sees the settings change.
async fn app_reloaded(_old: axum::Router) -> (axum::Router, sqlx::PgPool) {
    app().await
}
