//! Payments happy path and rejection matrix (rules/server-payments.md §Тести)
//! against local Postgres/Memurai with fake RPC/Jupiter (tests/common/payments.rs).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use axum::http::StatusCode;
use common::payments::*;
use common::{get as api_get, login, post as api_post};
use serde_json::{json, Value};
use uuid::Uuid;

#[tokio::test]
async fn test_quote_math_and_limits() {
    let (app, _, _) = app_with_payments(true).await;
    let session = login(&app).await;
    api_get(&app, &session, "/island").await;
    let (status, order) = quote(&app, &session, "fert_premium", 1).await;
    assert_eq!(status, StatusCode::OK, "{order}");
    // $0.90 at $0.0031/CLASH = 290.322580… CLASH → ceil in 6 dp; burn = ceil(70%).
    assert_eq!(order["price_usd_cents"], 90);
    assert_eq!(order["amount_clash"], 290_322_581);
    assert_eq!(order["burn_amount"], 203_225_807);
    assert_eq!(order["transfer_amount"], 87_096_774);
    assert_eq!(
        order["memo"],
        format!("cf:{}", order["order_id"].as_str().unwrap())
    );
    assert_eq!(order["treasury_token_account"], TREASURY_ATA);
    assert_eq!(order["mint"], MINT);
    let (status, body) = quote(&app, &session, "fert_premium", 0).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "invalid_qty");
    let (status, body) = quote(&app, &session, "seed_wheat", 1).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "credit-only item: {body}");
    assert_eq!(body["error"]["code"], "invalid_item");
}

#[tokio::test]
async fn test_fulfill_is_atomic() {
    let (app, pool, chain) = app_with_payments(true).await;
    let session = login(&app).await;
    let user_id = Uuid::parse_str(&session.user_id).unwrap();
    api_get(&app, &session, "/island").await;
    let (_, order) = quote(&app, &session, "fert_premium", 2).await;
    let order_id = order["order_id"].as_str().unwrap().to_owned();
    let sig = signature();
    chain.txs.lock().unwrap().insert(
        sig.clone(),
        payment_tx(
            &wallet_of(&session),
            order["burn_amount"].as_i64().unwrap(),
            order["transfer_amount"].as_i64().unwrap(),
            TREASURY_ATA,
            &format!("cf:{order_id}"),
            Value::Null,
        ),
    );
    let (status, outcome) = submit(&app, &session, &order_id, &sig).await;
    assert_eq!(status, StatusCode::OK, "{outcome}");
    assert_eq!(outcome["status"], "fulfilled");
    let (_, inv) = api_get(&app, &session, "/inventory").await;
    let premium = inv["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["item_id"] == "fert_premium")
        .unwrap();
    assert_eq!(premium["qty"], 20, "a pack of ten per unit ordered (0019)");
    let burns: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM cf.burns WHERE signature = $1 AND block_time IS NOT NULL AND treasury_amount = $2",
    )
    .bind(&sig)
    .bind(order["transfer_amount"].as_i64().unwrap())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(burns, 1, "burn row carries block_time and the treasury share");
    let unbalanced: i64 = sqlx::query_scalar("SELECT count(*) FROM (SELECT ref_id, currency, sum(delta) s FROM cf.ledger GROUP BY ref_id, currency) t WHERE s <> 0").fetch_one(&pool).await.unwrap();
    assert_eq!(unbalanced, 0);
    let paid: i64 =
        sqlx::query_scalar("SELECT amount FROM cf.balances WHERE user_id = $1 AND account = 'clash_paid'")
            .bind(user_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(paid, order["amount_clash"].as_i64().unwrap());
    let rep: i32 = sqlx::query_scalar("SELECT reputation FROM cf.users WHERE id = $1")
        .bind(user_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(rep, 10);
    let (status, view) = api_get(&app, &session, &format!("/orders/{order_id}")).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(view["status"], "fulfilled");
    // Same order + same signature again (client retry) → still fulfilled, no orphan event;
    // same signature on a fresh order → sig_reused.
    let (status, body) = submit(&app, &session, &order_id, &sig).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["status"], "fulfilled");
    let orphans: i64 =
        sqlx::query_scalar("SELECT count(*) FROM cf.events WHERE user_id = $1 AND name = 'orphan_signature'")
            .bind(user_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(orphans, 0);
    let (_, order2) = quote(&app, &session, "fert_premium", 1).await;
    let (status, body) = submit(&app, &session, order2["order_id"].as_str().unwrap(), &sig).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["error"]["code"], "sig_reused");
    // The premium fertilizer stays exclusive: one fulfilled CLASH order unlocks it (ECONOMY §3).
    // Clashberry itself waits for level 14 (0019), so the island is grown here.
    sqlx::query("INSERT INTO cf.inventory (user_id, item_id, qty) VALUES ($1, 'seed_clashberry', 1)")
        .bind(user_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE cf.islands SET level = 14 WHERE user_id = $1")
        .bind(user_id)
        .execute(&pool)
        .await
        .unwrap();
    let (_, snapshot) = api_get(&app, &session, "/island").await;
    let plot_id = snapshot["plots"][0]["id"].as_str().unwrap();
    let (status, plot) = api_post(
        &app,
        &session,
        &format!("/plots/{plot_id}/plant"),
        Some(json!({ "item_id": "seed_clashberry" })),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{plot}");
}

#[tokio::test]
async fn test_rejections_keep_inventory_untouched() {
    let (app, pool, chain) = app_with_payments(true).await;
    let session = login(&app).await;
    let owner = wallet_of(&session);
    api_get(&app, &session, "/island").await;
    let cases: Vec<(&str, TxBuilder<'_>)> = vec![
        (
            "forbidden_instruction",
            Box::new(|memo, b, t| {
                let mut tx = payment_tx(&owner, b, t, TREASURY_ATA, memo, Value::Null);
                tx["transaction"]["message"]["instructions"]
                    .as_array_mut()
                    .unwrap()
                    .push(token_ix("approve", 1, &owner, json!({ "delegate": "Evil" })));
                tx
            }),
        ),
        (
            "wrong_mint",
            Box::new(|memo, b, t| {
                let mut tx = payment_tx(&owner, b, t, TREASURY_ATA, memo, Value::Null);
                tx["transaction"]["message"]["instructions"][1]["parsed"]["info"]["mint"] =
                    json!("OtherMint1111111111111111111111111111111111");
                tx
            }),
        ),
        (
            "wrong_owner",
            Box::new(|memo, b, t| {
                payment_tx(
                    "SomeoneE1se1111111111111111111111111111111111",
                    b,
                    t,
                    TREASURY_ATA,
                    memo,
                    Value::Null,
                )
            }),
        ),
        (
            "amount_low",
            Box::new(|memo, b, t| payment_tx(&owner, b - 1, t, TREASURY_ATA, memo, Value::Null)),
        ),
        (
            "memo_mismatch",
            Box::new(|_memo, b, t| payment_tx(&owner, b, t, TREASURY_ATA, "cf:other", Value::Null)),
        ),
        (
            "tx_failed",
            Box::new(|memo, b, t| {
                payment_tx(
                    &owner,
                    b,
                    t,
                    TREASURY_ATA,
                    memo,
                    json!({ "InstructionError": [1, "Custom"] }),
                )
            }),
        ),
        (
            "forbidden_instruction",
            Box::new(|memo, b, t| {
                payment_tx(
                    &owner,
                    b,
                    t,
                    "Attacker1111111111111111111111111111111111111",
                    memo,
                    Value::Null,
                )
            }),
        ),
    ];
    for (expected, build) in cases {
        let (_, order) = quote(&app, &session, "fert_premium", 1).await;
        let order_id = order["order_id"].as_str().unwrap().to_owned();
        let sig = signature();
        let tx = build(
            &format!("cf:{order_id}"),
            order["burn_amount"].as_i64().unwrap(),
            order["transfer_amount"].as_i64().unwrap(),
        );
        chain.txs.lock().unwrap().insert(sig.clone(), tx);
        let (status, outcome) = submit(&app, &session, &order_id, &sig).await;
        assert_eq!(status, StatusCode::OK, "{expected}: {outcome}");
        assert_eq!(outcome["status"], "rejected", "{expected}");
        assert_eq!(outcome["reason_code"], expected);
        let (_, view) = api_get(&app, &session, &format!("/orders/{order_id}")).await;
        assert_eq!(view["status"], "rejected");
    }
    let (_, inv) = api_get(&app, &session, "/inventory").await;
    assert!(
        inv["items"]
            .as_array()
            .unwrap()
            .iter()
            .all(|i| i["item_id"] != "fert_premium"),
        "nothing fulfilled"
    );
    let burns: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM cf.burns b JOIN cf.orders o ON o.id = b.order_id WHERE o.user_id = $1",
    )
    .bind(Uuid::parse_str(&session.user_id).unwrap())
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(burns, 0);
}

#[tokio::test]
async fn test_quote_expired_and_pending() {
    let (app, pool, chain) = app_with_payments(true).await;
    let session = login(&app).await;
    api_get(&app, &session, "/island").await;
    let (_, order) = quote(&app, &session, "fert_premium", 1).await;
    let order_id = order["order_id"].as_str().unwrap().to_owned();
    sqlx::query("UPDATE cf.orders SET quote_expires_at = now() - interval '3 minutes' WHERE id = $1")
        .bind(Uuid::parse_str(&order_id).unwrap())
        .execute(&pool)
        .await
        .unwrap();
    // Submitted after the grace, burned now (after quote + grace): verified like any signature,
    // and the block-time window refuses the late burn — the quoted price never floats.
    let late_sig = signature();
    chain
        .txs
        .lock()
        .unwrap()
        .insert(late_sig.clone(), good_tx(&session, &order));
    let (status, body) = submit(&app, &session, &order_id, &late_sig).await;
    assert_eq!(
        (status, body["reason_code"].clone()),
        (StatusCode::OK, json!("too_late")),
        "{body}"
    );
    // The expiry job closed an order whose payment was mined in time (the client lost the
    // answer): the signature arriving afterwards is verified and fulfilled.
    let (_, order) = quote(&app, &session, "fert_premium", 1).await;
    let expired_id = order["order_id"].as_str().unwrap().to_owned();
    sqlx::query("UPDATE cf.orders SET status = 'expired' WHERE id = $1")
        .bind(Uuid::parse_str(&expired_id).unwrap())
        .execute(&pool)
        .await
        .unwrap();
    let sig = signature();
    chain
        .txs
        .lock()
        .unwrap()
        .insert(sig.clone(), good_tx(&session, &order));
    let (status, body) = submit(&app, &session, &expired_id, &sig).await;
    assert_eq!(
        (status, body["status"].clone()),
        (StatusCode::OK, json!("fulfilled")),
        "{body}"
    );
    // One order, one signature: a different one is refused, the first stays the only one on record.
    let (_, order) = quote(&app, &session, "fert_premium", 1).await;
    let bound_id = order["order_id"].as_str().unwrap().to_owned();
    let first = signature();
    let (status, _) = submit(&app, &session, &bound_id, &first).await;
    assert_eq!(status, StatusCode::ACCEPTED);
    let (status, body) = submit(&app, &session, &bound_id, &signature()).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["error"]["code"], "wrong_status");
    let (status, _) = submit(&app, &session, &bound_id, &first).await;
    assert_eq!(
        status,
        StatusCode::ACCEPTED,
        "the client keeps polling with the first"
    );
    let recorded: Vec<String> = sqlx::query_scalar(
        "SELECT props->>'signature' FROM cf.events WHERE name = 'order_submitted' AND props->>'order_id' = $1",
    )
    .bind(&bound_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(recorded, vec![first], "recorded once, never the second");
    let orphans: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM cf.events WHERE name = 'orphan_signature' AND props->>'order_id' = $1",
    )
    .bind(&bound_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        orphans, 1,
        "the refused second payment is still on record for support"
    );
    // Pending: unknown signature → 202, later visible → fulfilled.
    let (_, order) = quote(&app, &session, "fert_premium", 1).await;
    let order_id = order["order_id"].as_str().unwrap().to_owned();
    let sig = signature();
    let (status, body) = submit(&app, &session, &order_id, &sig).await;
    assert_eq!(status, StatusCode::ACCEPTED, "{body}");
    assert_eq!(body["status"], "submitted");
    chain.txs.lock().unwrap().insert(
        sig.clone(),
        payment_tx(
            &wallet_of(&session),
            order["burn_amount"].as_i64().unwrap(),
            order["transfer_amount"].as_i64().unwrap(),
            TREASURY_ATA,
            &format!("cf:{order_id}"),
            Value::Null,
        ),
    );
    let (status, body) = submit(&app, &session, &order_id, &sig).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["status"], "fulfilled");
    let (status, body) = submit(&app, &session, &order_id, "not-a-signature").await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
}
