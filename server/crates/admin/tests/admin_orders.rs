//! Manual order review through the real fulfilment path: once, and once under parallel clicks.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use axum::http::StatusCode;
use common::*;
use serde_json::{json, Value};
use uuid::Uuid;

#[tokio::test]
async fn test_manual_review_fulfils_once_and_rejects() {
    let h = harness().await;
    let (cookie, csrf) = login(&h, &h.key, &h.wallet).await.unwrap();
    let signature = fresh_signature();
    let (user_id, order_id) = parked_order(&h.pool, Some(&signature)).await;
    let (status, body) = get_json(&h.app, &cookie, "/api/orders").await;
    assert_eq!(status, StatusCode::OK);
    let listed = body["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|o| o["id"] == order_id.to_string())
        .unwrap();
    assert_eq!(listed["signature"], signature, "the submitted signature is shown");
    let (status, body) = get_json(&h.app, &cookie, "/api/orders?status=bogus").await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    let approve = |id: Uuid| {
        let (app, cookie, csrf) = (h.app.clone(), cookie.clone(), csrf.clone());
        async move {
            post_json(
                &app,
                &cookie,
                &csrf,
                &format!("/api/orders/{id}/approve"),
                Value::Null,
            )
            .await
        }
    };
    let (status, body) = approve(order_id).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body["amount_clash"], 90000,
        "the whole order amount; the burn share is booked in cf.burns"
    );
    let (st, reason) = order_row(&h.pool, order_id).await;
    assert_eq!(st, "fulfilled");
    assert_eq!(
        reason.as_deref(),
        Some("manual_review"),
        "manual payouts stay recognisable"
    );
    let qty: i32 =
        sqlx::query_scalar("SELECT qty FROM cf.inventory WHERE user_id = $1 AND item_id = 'decor_lantern'")
            .bind(user_id)
            .fetch_one(&h.pool)
            .await
            .unwrap();
    assert_eq!(qty, 1, "the item was delivered");
    let (burned, booked): (i64, String) =
        sqlx::query_as("SELECT amount, signature FROM cf.burns WHERE order_id = $1")
            .bind(order_id)
            .fetch_one(&h.pool)
            .await
            .unwrap();
    assert_eq!(burned, 63000);
    assert_eq!(
        booked, signature,
        "booked against the submitted signature, not a typed one"
    );
    assert_eq!(audit_count(&h.pool, "order_approve_attempt", order_id).await, 1);
    assert_eq!(audit_count(&h.pool, "order_approve", order_id).await, 1);
    // A second approval of the same order is refused and audited as a failure.
    let (status, _) = approve(order_id).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(audit_count(&h.pool, "order_approve_failed", order_id).await, 1);
    // The same submitted signature on another parked order is caught by `burns`: 409, still parked.
    let (_, second) = parked_order(&h.pool, Some(&signature)).await;
    let (status, _) = approve(second).await;
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(order_row(&h.pool, second).await.0, "needs_review");
    // Nothing submitted by the player: nothing to approve, no typed-in signatures.
    let (_, blank) = parked_order(&h.pool, None).await;
    let (status, _) = approve(blank).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(order_row(&h.pool, blank).await.0, "needs_review");
    // Reject a parked order by hand; an empty reason is refused.
    let (_, third) = parked_order(&h.pool, Some(&fresh_signature())).await;
    let (status, _) = post_json(
        &h.app,
        &cookie,
        &csrf,
        &format!("/api/orders/{third}/reject"),
        json!({ "reason": "  " }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    let (status, body) = post_json(
        &h.app,
        &cookie,
        &csrf,
        &format!("/api/orders/{third}/reject"),
        json!({ "reason": "no tx found" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (st, reason) = order_row(&h.pool, third).await;
    assert_eq!(st, "rejected");
    assert_eq!(reason.as_deref(), Some("admin:no tx found"));
    assert_eq!(audit_count(&h.pool, "order_reject", third).await, 1);
    // Rejecting a fulfilled order is impossible.
    let (status, _) = post_json(
        &h.app,
        &cookie,
        &csrf,
        &format!("/api/orders/{order_id}/reject"),
        json!({ "reason": "oops" }),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
}

#[tokio::test]
async fn test_parallel_approvals_pay_out_once() {
    let h = harness().await;
    let (cookie, csrf) = login(&h, &h.key, &h.wallet).await.unwrap();
    let (user_id, order_id) = parked_order(&h.pool, Some(&fresh_signature())).await;
    let path = format!("/api/orders/{order_id}/approve");
    let (a, b) = tokio::join!(
        post_json(&h.app, &cookie, &csrf, &path, Value::Null),
        post_json(&h.app, &cookie, &csrf, &path, Value::Null)
    );
    let statuses = [a.0, b.0];
    assert!(statuses.contains(&StatusCode::OK), "{a:?} {b:?}");
    assert!(statuses.contains(&StatusCode::CONFLICT), "{a:?} {b:?}");
    let qty: i32 =
        sqlx::query_scalar("SELECT qty FROM cf.inventory WHERE user_id = $1 AND item_id = 'decor_lantern'")
            .bind(user_id)
            .fetch_one(&h.pool)
            .await
            .unwrap();
    assert_eq!(qty, 1, "exactly one payout");
    let burns: i64 = sqlx::query_scalar("SELECT count(*) FROM cf.burns WHERE order_id = $1")
        .bind(order_id)
        .fetch_one(&h.pool)
        .await
        .unwrap();
    assert_eq!(burns, 1);
}
