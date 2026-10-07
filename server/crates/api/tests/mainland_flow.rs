//! Mainland chunks (docs/MAINLAND.md M2): the session sees a deterministic
//! window of nodes, bad windows are refused, no session → 401.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use axum::body::Body;
use axum::http::{Request, StatusCode};
use common::{app, get, login};
use tower::ServiceExt;

#[tokio::test]
async fn test_chunk_window_needs_session_and_is_stable() {
    let (app, _) = app().await;
    let anon = Request::builder()
        .uri("/api/v1/mainland/chunks?cx=8&cz=14")
        .body(Body::empty())
        .unwrap();
    let resp = app.clone().oneshot(anon).await.unwrap();
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);

    let session = login(&app).await;
    let (status, body) = get(&app, &session, "/mainland/chunks?cx=8&cz=14&r=3").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let chunks = body["chunks"].as_array().unwrap();
    assert_eq!(chunks.len(), 35, "7 × 5 chunks fit inside the map at the dock");
    let beach = chunks.iter().find(|c| c["cx"] == 8 && c["cz"] == 14).unwrap();
    assert_eq!(beach["biome"], "B");
    assert!(beach["nodes"].as_array().unwrap().is_empty());
    let meadow = chunks.iter().find(|c| c["cx"] == 8 && c["cz"] == 12).unwrap();
    assert_eq!(meadow["biome"], "M");
    let nodes = meadow["nodes"].as_array().unwrap();
    assert_eq!(nodes.len(), 6);
    assert!(nodes.iter().all(|n| n["id"].is_string()
        && n["kind"].is_string()
        && n["x_dm"].is_i64()
        && n["hits_left"].as_i64().unwrap() > 0));

    let (again, body2) = get(&app, &session, "/mainland/chunks?cx=8&cz=14&r=3").await;
    assert_eq!(again, StatusCode::OK);
    assert_eq!(body, body2, "same window twice → same nodes");

    let (status, map) = get(&app, &session, "/mainland/map").await;
    assert_eq!(status, StatusCode::OK, "{map}");
    assert_eq!(map["chunks"], 16);
    assert_eq!(map["rows"].as_array().unwrap().len(), 16);
    assert_eq!(map["rows"][14].as_str().unwrap().chars().nth(8), Some('B'));

    let (status, city) = get(&app, &session, "/city").await;
    assert_eq!(status, StatusCode::OK, "{city}");
    let buildings = city["buildings"].as_array().unwrap();
    assert_eq!(buildings.len(), 8);
    assert!(buildings
        .iter()
        .any(|b| b["id"] == "market" && b["kind"] == "market"));
    assert!(
        buildings.iter().all(|b| b["partner"].is_null()),
        "no partners yet"
    );

    let (bad, _) = get(&app, &session, "/mainland/chunks?cx=16&cz=0").await;
    assert_eq!(bad, StatusCode::BAD_REQUEST);
    let (bad_r, _) = get(&app, &session, "/mainland/chunks?cx=8&cz=8&r=5").await;
    assert_eq!(bad_r, StatusCode::BAD_REQUEST);
}
