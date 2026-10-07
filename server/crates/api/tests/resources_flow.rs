//! Resource gathering (migration 0007): starter nodes in the snapshot, hits
//! and items, depletion with a DB-clock respawn, regrowth at a new spot (0017), daily cap,
//! ownership. Against local Postgres + Memurai.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use axum::http::StatusCode;
use common::{app, get, login, post};
use uuid::Uuid;

async fn island(app: &axum::Router, session: &common::Session) -> serde_json::Value {
    let (status, body) = get(app, session, "/island").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body
}

async fn gather(app: &axum::Router, session: &common::Session, idx: i16) -> (StatusCode, serde_json::Value) {
    post(app, session, &format!("/resources/{idx}/gather"), None, None).await
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
async fn test_snapshot_lists_starter_nodes() {
    let (app, _) = app().await;
    let session = login(&app).await;
    let snapshot = island(&app, &session).await;
    let nodes = snapshot["resources"].as_array().unwrap();
    assert_eq!(nodes.len(), 8, "{nodes:?}");
    assert_eq!(nodes[0]["kind"], "tree");
    assert_eq!(nodes[0]["hits_left"], 5);
    assert_eq!(nodes[3]["kind"], "rock");
    assert_eq!(nodes[3]["hits_left"], 8);
    assert!(nodes[0]["respawn_at"].is_null());
}

#[tokio::test]
async fn test_gather_gives_wood_then_depletes_and_regrows() {
    let (app, pool) = app().await;
    let session = login(&app).await;
    island(&app, &session).await;
    for expected_left in (0..5).rev() {
        let (status, body) = gather(&app, &session, 0).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["item_id"], "wood");
        assert_eq!(body["qty"], 1);
        assert_eq!(body["node"]["hits_left"], expected_left);
        assert_eq!(body["node"]["respawn_at"].is_null(), expected_left > 0);
    }
    let (_, inv) = get(&app, &session, "/inventory").await;
    assert_eq!(qty(&inv, "wood"), 5);
    assert_eq!(island(&app, &session).await["island"]["xp"], 5, "1 xp per hit");
    let (status, body) = gather(&app, &session, 0).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["error"]["code"], "depleted");
    // Regrowth is the database clock's call: move respawn_at into the past.
    sqlx::query(
        "UPDATE cf.resource_nodes SET respawn_at = now() - interval '1 second'
         WHERE idx = 0 AND island_id = (SELECT id FROM cf.islands WHERE user_id = $1)",
    )
    .bind(Uuid::parse_str(&session.user_id).unwrap())
    .execute(&pool)
    .await
    .unwrap();
    let snapshot = island(&app, &session).await;
    assert_eq!(
        snapshot["resources"][0]["hits_left"], 0,
        "still used up until the regrow job moves it (0017)"
    );
    let params = cf_world::params::Params::default();
    // Tests run in parallel and every pass takes all due islands, so another test's pass
    // may move this node first: the node state below is the check, not the pass summary.
    cf_world::regrow::run(&pool, params).await.unwrap();
    let snapshot = island(&app, &session).await;
    let node = &snapshot["resources"][0];
    assert_eq!(node["hits_left"], 5, "moved node is fresh");
    assert_eq!(node["stage"], 0, "moved tree comes back as a sapling");
    assert!(
        node["x_dm"] != -160 || node["z_dm"] != -140,
        "tree left its starter spot"
    );
    let (status, body) = gather(&app, &session, 0).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["error"]["code"], "not_ready", "saplings cannot be gathered");
    // Growth is the database clock's call too.
    sqlx::query(
        "UPDATE cf.resource_nodes SET grown_at = now() - interval '3 hours'
         WHERE idx = 0 AND island_id = (SELECT id FROM cf.islands WHERE user_id = $1)",
    )
    .bind(Uuid::parse_str(&session.user_id).unwrap())
    .execute(&pool)
    .await
    .unwrap();
    let (status, body) = gather(&app, &session, 0).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["node"]["hits_left"], 4);
    assert_eq!(body["node"]["stage"], 2);
}

#[tokio::test]
async fn test_bought_strips_get_their_own_nodes_on_free_spots() {
    let (app, pool) = app().await;
    let session = login(&app).await;
    island(&app, &session).await;
    let user_id = Uuid::parse_str(&session.user_id).unwrap();
    sqlx::query("UPDATE cf.islands SET expand_e = 2, expand_n = 1 WHERE user_id = $1")
        .bind(user_id)
        .execute(&pool)
        .await
        .unwrap();
    let params = cf_world::params::Params::default();
    // A parallel test's pass may plant here first; the counts below are the check.
    cf_world::regrow::run(&pool, params).await.unwrap();
    let nodes = island(&app, &session).await["resources"]
        .as_array()
        .unwrap()
        .clone();
    let trees = nodes.iter().filter(|n| n["kind"] == "tree").count();
    let rocks = nodes.iter().filter(|n| n["kind"] == "rock").count();
    assert_eq!((trees, rocks), (6, 8), "starters 3+5 plus 3 strips");
    // Every node sits inside the expanded top and none overlap each other or the house.
    for (i, a) in nodes.iter().enumerate() {
        let (ax, az) = (a["x_dm"].as_i64().unwrap(), a["z_dm"].as_i64().unwrap());
        assert!(
            (-240..=400).contains(&ax) && (-320..=240).contains(&az),
            "inside: {a}"
        );
        assert!(
            (ax + 130).pow(2) + (az + 70).pow(2) >= 60 * 60,
            "off the house: {a}"
        );
        for b in nodes.iter().skip(i + 1) {
            let (bx, bz) = (b["x_dm"].as_i64().unwrap(), b["z_dm"].as_i64().unwrap());
            assert!(
                (ax - bx).pow(2) + (az - bz).pow(2) >= 28 * 28,
                "apart: {a} vs {b}"
            );
        }
    }
    cf_world::regrow::run(&pool, params).await.unwrap();
    let after = island(&app, &session).await["resources"]
        .as_array()
        .unwrap()
        .len();
    assert_eq!(after, nodes.len(), "target reached, nothing more to plant");
}

#[tokio::test]
async fn test_rock_gives_stone_and_daily_cap_applies() {
    let (app, pool) = app().await;
    let session = login(&app).await;
    island(&app, &session).await;
    let (status, body) = gather(&app, &session, 3).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["item_id"], "stone");
    assert_eq!(body["node"]["hits_left"], 7);
    // 59 hits already counted today: one more fits, the next is over the cap.
    sqlx::query(
        "INSERT INTO cf.rate_events (user_id, action, day, count) VALUES ($1, 'gather', (now() AT TIME ZONE 'UTC')::date, 59)
         ON CONFLICT (user_id, action, day) DO UPDATE SET count = 59",
    )
    .bind(Uuid::parse_str(&session.user_id).unwrap())
    .execute(&pool)
    .await
    .unwrap();
    let (status, _) = gather(&app, &session, 3).await;
    assert_eq!(status, StatusCode::OK);
    let (status, body) = gather(&app, &session, 3).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "{body}");
    assert_eq!(body["error"]["code"], "daily_cap");
    assert_eq!(
        island(&app, &session).await["resources"][3]["hits_left"],
        6,
        "capped hit did not land"
    );
}

#[tokio::test]
async fn test_unknown_or_foreign_node_is_404() {
    let (app, _) = app().await;
    let session = login(&app).await;
    island(&app, &session).await;
    let (status, _) = gather(&app, &session, 42).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    // Without an island of their own, idx 0 resolves to nothing.
    let stranger = login(&app).await;
    let (status, _) = gather(&app, &stranger, 0).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    // With an island, idx 0 is the stranger's own tree: the victim's node stays untouched.
    island(&app, &stranger).await;
    let (status, body) = gather(&app, &stranger, 0).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["node"]["hits_left"], 4);
    assert_eq!(
        island(&app, &session).await["resources"][0]["hits_left"],
        5,
        "foreign hit never lands"
    );
}

#[tokio::test]
async fn test_gather_race_single_winner_on_last_hit() {
    let (app, pool) = app().await;
    let session = login(&app).await;
    island(&app, &session).await;
    sqlx::query(
        "UPDATE cf.resource_nodes SET hits_left = 1
         WHERE idx = 0 AND island_id = (SELECT id FROM cf.islands WHERE user_id = $1)",
    )
    .bind(Uuid::parse_str(&session.user_id).unwrap())
    .execute(&pool)
    .await
    .unwrap();
    let (a, b, c) = tokio::join!(
        gather(&app, &session, 0),
        gather(&app, &session, 0),
        gather(&app, &session, 0)
    );
    let statuses = [a.0, b.0, c.0];
    assert_eq!(
        statuses.iter().filter(|s| **s == StatusCode::OK).count(),
        1,
        "{statuses:?}"
    );
    assert_eq!(
        statuses.iter().filter(|s| **s == StatusCode::CONFLICT).count(),
        2,
        "{statuses:?}"
    );
    let (_, inv) = get(&app, &session, "/inventory").await;
    assert_eq!(qty(&inv, "wood"), 1, "only the winning hit paid out");
    assert_eq!(island(&app, &session).await["island"]["xp"], 1);
}
