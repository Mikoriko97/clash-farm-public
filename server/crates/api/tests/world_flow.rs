//! World integration tests (rules/server-ledger-world.md): ownership, DB clock,
//! daily caps, harvest race, idempotency. Against local Postgres + Memurai.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use axum::http::StatusCode;
use common::{app, get, login, post};
use serde_json::json;
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;
use uuid::Uuid;

async fn island(app: &axum::Router, session: &common::Session) -> serde_json::Value {
    let (status, body) = get(app, session, "/island").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body
}

async fn force_ready(pool: &sqlx::PgPool, plot_id: &str) {
    sqlx::query("UPDATE cf.plots SET ready_at = now() - interval '1 second' WHERE id = $1")
        .bind(Uuid::parse_str(plot_id).unwrap())
        .execute(pool)
        .await
        .unwrap();
}

#[tokio::test]
async fn test_island_created_with_plots_and_starter_seeds() {
    let (app, _) = app().await;
    let session = login(&app).await;
    let snapshot = island(&app, &session).await;
    assert_eq!(snapshot["island"]["level"], 1);
    assert_eq!(snapshot["plots"].as_array().unwrap().len(), 6);
    assert!(snapshot["plots"]
        .as_array()
        .unwrap()
        .iter()
        .all(|p| p["state"] == "empty"));
    assert!(snapshot["now"].is_string());
    // Second call: same island, still 6 plots (no duplicate creation).
    let again = island(&app, &session).await;
    assert_eq!(again["island"]["id"], snapshot["island"]["id"]);
    assert_eq!(again["plots"].as_array().unwrap().len(), 6);
    let (status, inv) = get(&app, &session, "/inventory").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(wheat(&inv), 6);
}

#[tokio::test]
async fn test_plant_uses_db_clock_and_consumes_seed() {
    let (app, _) = app().await;
    let session = login(&app).await;
    let snapshot = island(&app, &session).await;
    let plot_id = snapshot["plots"][0]["id"].as_str().unwrap().to_owned();
    let (status, plot) = post(
        &app,
        &session,
        &format!("/plots/{plot_id}/plant"),
        Some(json!({ "item_id": "seed_wheat" })),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{plot}");
    assert_eq!(plot["state"], "growing");
    assert_eq!(plot["crop_id"], "seed_wheat");
    let planted = OffsetDateTime::parse(plot["planted_at"].as_str().unwrap(), &Rfc3339).unwrap();
    let ready = OffsetDateTime::parse(plot["ready_at"].as_str().unwrap(), &Rfc3339).unwrap();
    assert_eq!(
        (ready - planted).whole_seconds(),
        1800,
        "wheat grows 30 minutes by DB clock"
    );
    let server_now = OffsetDateTime::parse(snapshot["now"].as_str().unwrap(), &Rfc3339).unwrap();
    assert!(
        (planted - server_now).whole_seconds().abs() < 30,
        "planted_at comes from the DB clock"
    );
    let (_, inv) = get(&app, &session, "/inventory").await;
    assert_eq!(wheat(&inv), 5);
    // Planting again on a growing plot is refused; unknown item refused.
    let (status, body) = post(
        &app,
        &session,
        &format!("/plots/{plot_id}/plant"),
        Some(json!({ "item_id": "seed_wheat" })),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["error"]["code"], "wrong_state");
    let other = snapshot["plots"][1]["id"].as_str().unwrap();
    let (status, body) = post(
        &app,
        &session,
        &format!("/plots/{other}/plant"),
        Some(json!({ "item_id": "fert_basic" })),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    assert_eq!(body["error"]["code"], "invalid_item");
}

#[tokio::test]
async fn test_harvest_requires_ready_then_awards_xp() {
    let (app, pool) = app().await;
    let session = login(&app).await;
    let snapshot = island(&app, &session).await;
    let plot_id = snapshot["plots"][0]["id"].as_str().unwrap().to_owned();
    post(
        &app,
        &session,
        &format!("/plots/{plot_id}/plant"),
        Some(json!({ "item_id": "seed_wheat" })),
        None,
    )
    .await;
    let (status, body) = post(&app, &session, &format!("/plots/{plot_id}/harvest"), None, None).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["error"]["code"], "not_ready");
    force_ready(&pool, &plot_id).await;
    let (status, result) = post(&app, &session, &format!("/plots/{plot_id}/harvest"), None, None).await;
    assert_eq!(status, StatusCode::OK, "{result}");
    assert_eq!(result["plot"]["state"], "empty");
    assert_eq!(result["item_id"], "crop_wheat", "harvest yields produce (0008)");
    assert_eq!(result["qty"], 1);
    assert_eq!(
        result["xp"], 2,
        "1 for planting (0019) + 10 credits of value / 10"
    );
    let (status, body) = post(&app, &session, &format!("/plots/{plot_id}/harvest"), None, None).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["error"]["code"], "wrong_state");
}

#[tokio::test]
async fn test_foreign_plot_404() {
    let (app, pool) = app().await;
    let owner = login(&app).await;
    let intruder = login(&app).await;
    let snapshot = island(&app, &owner).await;
    let plot_id = snapshot["plots"][0]["id"].as_str().unwrap().to_owned();
    post(
        &app,
        &owner,
        &format!("/plots/{plot_id}/plant"),
        Some(json!({ "item_id": "seed_wheat" })),
        None,
    )
    .await;
    force_ready(&pool, &plot_id).await;
    island(&app, &intruder).await;
    for path in [
        format!("/plots/{plot_id}/harvest"),
        format!("/plots/{plot_id}/fertilize"),
    ] {
        let body = if path.ends_with("fertilize") {
            Some(json!({ "item_id": "fert_basic" }))
        } else {
            None
        };
        let (status, err) = post(&app, &intruder, &path, body, None).await;
        assert_eq!(status, StatusCode::NOT_FOUND, "{err}");
        assert_eq!(err["error"]["code"], "not_found");
    }
    // Owner can still harvest: nothing was consumed by the intruder.
    let (status, _) = post(&app, &owner, &format!("/plots/{plot_id}/harvest"), None, None).await;
    assert_eq!(status, StatusCode::OK);
}

#[tokio::test]
async fn test_harvest_race_single_winner() {
    let (app, pool) = app().await;
    let session = login(&app).await;
    let snapshot = island(&app, &session).await;
    let plot_id = snapshot["plots"][0]["id"].as_str().unwrap().to_owned();
    post(
        &app,
        &session,
        &format!("/plots/{plot_id}/plant"),
        Some(json!({ "item_id": "seed_wheat" })),
        None,
    )
    .await;
    force_ready(&pool, &plot_id).await;
    let path = format!("/plots/{plot_id}/harvest");
    let (a, b, c) = tokio::join!(
        post(&app, &session, &path, None, None),
        post(&app, &session, &path, None, None),
        post(&app, &session, &path, None, None)
    );
    let successes = [a.0, b.0, c.0].iter().filter(|s| **s == StatusCode::OK).count();
    assert_eq!(successes, 1, "exactly one harvest wins: {:?}", [a.0, b.0, c.0]);
    let snapshot = island(&app, &session).await;
    assert_eq!(
        snapshot["island"]["xp"], 2,
        "xp awarded exactly once (1 planting + 1 harvest)"
    );
}

#[tokio::test]
async fn test_daily_cap_enforced() {
    let (app, pool) = app().await;
    let session = login(&app).await;
    let snapshot = island(&app, &session).await;
    let user_id = Uuid::parse_str(&session.user_id).unwrap();
    // 39 plants already "used" today → one more allowed, then the cap.
    sqlx::query(
        "INSERT INTO cf.rate_events (user_id, action, day, count) VALUES ($1, 'plant', (now() AT TIME ZONE 'UTC')::date, 39)",
    )
    .bind(user_id)
    .execute(&pool)
    .await
    .unwrap();
    let p0 = snapshot["plots"][0]["id"].as_str().unwrap();
    let p1 = snapshot["plots"][1]["id"].as_str().unwrap();
    let (status, _) = post(
        &app,
        &session,
        &format!("/plots/{p0}/plant"),
        Some(json!({ "item_id": "seed_wheat" })),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, body) = post(
        &app,
        &session,
        &format!("/plots/{p1}/plant"),
        Some(json!({ "item_id": "seed_wheat" })),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(body["error"]["code"], "daily_cap");
    // The refused plant consumed no seed and left the plot empty.
    let (_, inv) = get(&app, &session, "/inventory").await;
    assert_eq!(wheat(&inv), 5);
    let snapshot = island(&app, &session).await;
    assert_eq!(snapshot["plots"][1]["state"], "empty");
}

#[tokio::test]
async fn test_fertilize_shortens_once() {
    let (app, pool) = app().await;
    let session = login(&app).await;
    let snapshot = island(&app, &session).await;
    let plot_id = snapshot["plots"][0]["id"].as_str().unwrap().to_owned();
    let user_id = Uuid::parse_str(&session.user_id).unwrap();
    sqlx::query("INSERT INTO cf.inventory (user_id, item_id, qty) VALUES ($1, 'fert_basic', 2)")
        .bind(user_id)
        .execute(&pool)
        .await
        .unwrap();
    let (_, plot) = post(
        &app,
        &session,
        &format!("/plots/{plot_id}/plant"),
        Some(json!({ "item_id": "seed_wheat" })),
        None,
    )
    .await;
    let before = OffsetDateTime::parse(plot["ready_at"].as_str().unwrap(), &Rfc3339).unwrap();
    let (status, plot) = post(
        &app,
        &session,
        &format!("/plots/{plot_id}/fertilize"),
        Some(json!({ "item_id": "fert_basic" })),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{plot}");
    let after = OffsetDateTime::parse(plot["ready_at"].as_str().unwrap(), &Rfc3339).unwrap();
    let saved = (before - after).whole_seconds();
    assert!((440..=460).contains(&saved), "25% of 1800 s ≈ 450 s, got {saved}");
    assert_eq!(plot["fertilizer_id"], "fert_basic");
    let (status, body) = post(
        &app,
        &session,
        &format!("/plots/{plot_id}/fertilize"),
        Some(json!({ "item_id": "fert_basic" })),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(body["error"]["code"], "already_fertilized");
}

#[tokio::test]
async fn test_idempotency_key_replays_without_effect() {
    let (app, _) = app().await;
    let session = login(&app).await;
    let snapshot = island(&app, &session).await;
    let plot_id = snapshot["plots"][0]["id"].as_str().unwrap().to_owned();
    let key = Uuid::new_v4().to_string();
    let path = format!("/plots/{plot_id}/plant");
    let (status, first) = post(
        &app,
        &session,
        &path,
        Some(json!({ "item_id": "seed_wheat" })),
        Some(&key),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, replay) = post(
        &app,
        &session,
        &path,
        Some(json!({ "item_id": "seed_wheat" })),
        Some(&key),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "replay returns the stored response");
    assert_eq!(replay, first);
    let (_, inv) = get(&app, &session, "/inventory").await;
    assert_eq!(wheat(&inv), 5, "seed consumed once");
    // Missing key → 400.
    let req = axum::http::Request::builder()
        .method("POST")
        .uri(format!("/api/v1{path}"))
        .header("cookie", session.cookie())
        .header("x-csrf-token", &session.csrf)
        .header("content-type", "application/json")
        .body(axum::body::Body::from(
            json!({ "item_id": "seed_wheat" }).to_string(),
        ))
        .unwrap();
    let (status, body, _) = common::json(tower::ServiceExt::oneshot(app.clone(), req).await.unwrap()).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "idempotency_key_required");
}

#[tokio::test]
async fn test_clashberry_waits_for_level_fourteen() {
    let (app, pool) = app().await;
    let session = login(&app).await;
    let snapshot = island(&app, &session).await;
    let plot_id = snapshot["plots"][0]["id"].as_str().unwrap().to_owned();
    let user_id = Uuid::parse_str(&session.user_id).unwrap();
    sqlx::query("INSERT INTO cf.inventory (user_id, item_id, qty) VALUES ($1, 'seed_clashberry', 1)")
        .bind(user_id)
        .execute(&pool)
        .await
        .unwrap();
    let path = format!("/plots/{plot_id}/plant");
    let (status, body) = post(
        &app,
        &session,
        &path,
        Some(json!({ "item_id": "seed_clashberry" })),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(
        body["error"]["code"], "level_locked",
        "0019: a level instead of reputation"
    );
    let (_, inv) = get(&app, &session, "/inventory").await;
    let kept = inv["items"]
        .as_array()
        .unwrap()
        .iter()
        .any(|i| i["item_id"] == "seed_clashberry" && i["qty"] == 1);
    assert!(kept, "seed kept when refused");
    sqlx::query("UPDATE cf.islands SET level = 14 WHERE user_id = $1")
        .bind(user_id)
        .execute(&pool)
        .await
        .unwrap();
    let (status, plot) = post(
        &app,
        &session,
        &path,
        Some(json!({ "item_id": "seed_clashberry" })),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{plot}");
    assert_eq!(plot["crop_id"], "seed_clashberry");
}

#[tokio::test]
async fn test_idempotency_key_bound_to_route() {
    let (app, _) = app().await;
    let session = login(&app).await;
    let snapshot = island(&app, &session).await;
    let plot_id = snapshot["plots"][0]["id"].as_str().unwrap().to_owned();
    let key = Uuid::new_v4().to_string();
    let (status, _) = post(
        &app,
        &session,
        &format!("/plots/{plot_id}/plant"),
        Some(json!({ "item_id": "seed_wheat" })),
        Some(&key),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, body) = post(
        &app,
        &session,
        &format!("/plots/{plot_id}/harvest"),
        None,
        Some(&key),
    )
    .await;
    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY, "{body}");
    assert_eq!(body["error"]["code"], "idempotency_mismatch");
}

#[tokio::test]
async fn test_new_island_starts_with_credits_and_two_crops() {
    let (app, pool) = common::app().await;
    let session = login(&app).await;
    get(&app, &session, "/island").await;
    let (_, inv) = get(&app, &session, "/inventory").await;
    let qty = |id: &str| {
        inv["items"]
            .as_array()
            .unwrap()
            .iter()
            .find(|i| i["item_id"] == id)
            .map(|i| i["qty"].as_i64().unwrap())
            .unwrap_or(0)
    };
    assert_eq!(qty("seed_wheat"), 6);
    assert_eq!(qty("seed_carrot"), 2);
    let (_, me) = get(&app, &session, "/me").await;
    let credits = me["balances"]
        .as_array()
        .unwrap()
        .iter()
        .find(|b| b["account"] == "credits")
        .map(|b| b["amount"].as_i64().unwrap())
        .unwrap_or(0);
    assert_eq!(credits, 30, "starter credits booked once: {me}");
    let unbalanced: i64 = sqlx::query_scalar("SELECT count(*) FROM (SELECT ref_id, currency, sum(delta) s FROM cf.ledger WHERE ref_type = 'island_created' GROUP BY ref_id, currency) t WHERE s <> 0")
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(unbalanced, 0);
    // A second /island must not mint again.
    get(&app, &session, "/island").await;
    let (_, me2) = get(&app, &session, "/me").await;
    assert_eq!(me2["balances"], me["balances"]);
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
