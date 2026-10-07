//! Catalog edits, the ledger explorer, analytics, CSV exports and the heavy-read limit.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use axum::body::Body;
use axum::http::{header, Request, StatusCode};
use common::*;
use serde_json::{json, Value};
use tower::ServiceExt;
use uuid::Uuid;

#[tokio::test]
async fn test_catalog_edits_are_bounded_live_and_audited() {
    let h = harness().await;
    let (cookie, csrf) = login(&h, &h.key, &h.wallet).await.unwrap();
    let id = format!("test_item_{}", Uuid::new_v4().simple());
    sqlx::query("INSERT INTO cf.items (id, kind, price_credits, meta, tier) VALUES ($1, 'decor', 100, '{\"piece\": \"x\", \"sell_credits\": 40}', 1)")
        .bind(&id)
        .execute(&h.pool)
        .await
        .unwrap();
    let produce = format!("{id}_produce");
    sqlx::query(
        "INSERT INTO cf.items (id, kind, meta, tier) VALUES ($1, 'produce', '{\"sell_credits\": 7}', 1)",
    )
    .bind(&produce)
    .execute(&h.pool)
    .await
    .unwrap();
    let edit = |body: Value| {
        let (app, cookie, csrf, id) = (h.app.clone(), cookie.clone(), csrf.clone(), id.clone());
        async move { post_json(&app, &cookie, &csrf, &format!("/api/catalog/{id}"), body).await }
    };
    assert_eq!(
        edit(json!({ "price_credits": -1 })).await.0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        edit(json!({ "price_usd_cents": 0 })).await.0,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(edit(json!({ "tier": 9 })).await.0, StatusCode::BAD_REQUEST);
    assert_eq!(edit(json!({ "level_min": 1 })).await.0, StatusCode::BAD_REQUEST);
    // Economy invariants: no buy → sell loop (price ≤ market value), no prices on produce.
    assert_eq!(
        edit(json!({ "price_credits": 40 })).await.0,
        StatusCode::BAD_REQUEST,
        "price at market value"
    );
    let (status, body) = post_json(
        &h.app,
        &cookie,
        &csrf,
        &format!("/api/catalog/{produce}"),
        json!({ "price_credits": 100 }),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::BAD_REQUEST,
        "produce is never for sale: {body}"
    );
    let (status, body) = post_json(
        &h.app,
        &cookie,
        &csrf,
        &format!("/api/catalog/{produce}"),
        json!({ "price_usd_cents": 100 }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "nor for CLASH: {body}");
    let (status, body) = post_json(
        &h.app,
        &cookie,
        "bad",
        &format!("/api/catalog/{id}"),
        json!({ "enabled": false }),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    let (status, body) =
        edit(json!({ "price_credits": 250, "level_min": 5, "tier": 2, "enabled": false })).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["before"]["price_credits"], 100);
    assert_eq!(body["after"]["price_credits"], 250);
    assert_eq!(body["after"]["meta"]["level_min"], 5);
    assert_eq!(body["after"]["meta"]["piece"], "x", "other meta keys survive");
    assert_eq!(body["after"]["enabled"], false);
    let (status, body) = edit(json!({ "level_min": null, "price_credits": null })).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(
        body["after"]["meta"]["level_min"].is_null(),
        "gate removed: {body}"
    );
    assert!(
        body["after"]["price_credits"].is_null(),
        "not for credits any more: {body}"
    );
    let rows: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM cf.audit_log WHERE action = 'catalog_update' AND target = $1",
    )
    .bind(&id)
    .fetch_one(&h.pool)
    .await
    .unwrap();
    assert_eq!(rows, 2);
    let (status, _) = edit(json!({ "enabled": true })).await;
    assert_eq!(status, StatusCode::OK);
    let (status, _) = post_json(
        &h.app,
        &cookie,
        &csrf,
        "/api/catalog/no_such_item",
        json!({ "enabled": true }),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    sqlx::query("DELETE FROM cf.items WHERE id = $1 OR id = $2")
        .bind(&id)
        .bind(&produce)
        .execute(&h.pool)
        .await
        .unwrap();
}

#[tokio::test]
async fn test_ledger_analytics_and_exports_read_only() {
    let h = harness().await;
    let (cookie, csrf) = login(&h, &h.key, &h.wallet).await.unwrap();
    let (status, body) = get_json(&h.app, &cookie, "/api/ledger/summary").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body["days"].as_array().unwrap().len() == 30, "{body}");
    assert!(body["in_circulation"].is_number());
    let (status, body) = get_json(&h.app, &cookie, "/api/ledger?q=%25%25%25&account=credits").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["total"], 0, "wildcards are literal: {body}");
    let (status, _) = get_json(&h.app, &cookie, "/api/ledger?account=drop%20table").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, body) = post_json(&h.app, &cookie, &csrf, "/api/ledger/reconcile", Value::Null).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body["mismatches"].is_array());
    let audited: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM cf.audit_log WHERE action = 'ledger_reconcile' AND actor = $1",
    )
    .bind(&h.wallet)
    .fetch_one(&h.pool)
    .await
    .unwrap();
    assert_eq!(audited, 1);
    let (status, body) = get_json(&h.app, &cookie, "/api/analytics").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    for key in [
        "funnel", "crops", "bought", "sold", "actions", "levels", "cohorts",
    ] {
        assert!(!body[key].is_null(), "{key} missing: {body}");
    }
    let req = Request::builder()
        .uri("/api/export/orders.csv")
        .header(header::COOKIE, &cookie)
        .body(Body::empty())
        .unwrap();
    let response = h.app.clone().oneshot(req).await.unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let ctype = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    let disp = response
        .headers()
        .get(header::CONTENT_DISPOSITION)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    assert!(ctype.starts_with("text/csv"), "{ctype}");
    assert!(disp.contains("clash-farm-orders.csv"), "{disp}");
    let (status, _) = get(&h.app, &cookie, "/api/export/users.csv").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = get(&h.app, "", "/api/export/players.csv").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

/// Exports and reconcile scan whole tables: five a minute per admin, then 429.
#[tokio::test]
async fn test_heavy_reads_are_rate_limited() {
    let h = harness().await;
    let (cookie, _) = login(&h, &h.key, &h.wallet).await.unwrap();
    for i in 0..5 {
        let (status, _) = get(&h.app, &cookie, "/api/export/players.csv").await;
        assert_eq!(status, StatusCode::OK, "export {i}");
    }
    let (status, _) = get(&h.app, &cookie, "/api/export/players.csv").await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS);
    let (status, _) = get(&h.app, &cookie, "/api/ledger").await;
    assert_eq!(status, StatusCode::OK, "ordinary reads are not affected");
}
