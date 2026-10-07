//! Paid look change (0029, ADR-038): after the character exists, `POST /me/look` spends one
//! `restyle_ticket` (bought for CLASH through the ordinary order flow) and stores the new look; the
//! nickname stays. Without a ticket, with the same or an invalid look, or before the character
//! exists nothing is spent; a replay with the same key spends nothing more; two parallel changes
//! with one ticket have one winner; the ticket cannot be sold for credits; another player's ticket
//! is never touched. Against local Postgres + Memurai.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use axum::http::StatusCode;
use common::{app, app_with, get, login, post, Session};
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

const TICKET: &str = "restyle_ticket";

fn look(shirt: u8) -> Value {
    json!({ "skin": 1, "shirt": shirt, "pants": 3, "shoes": 4, "hair_color": 5, "hair": 2, "face": 1, "beard": 3, "hat": 4, "glasses": 1, "outfit": 3, "outfit_color": 6, "body": 1, "build": 2 })
}

/// A player with a character (shirt 2) and `tickets` restyle tickets.
async fn player(app: &axum::Router, pool: &PgPool, tickets: i32) -> Session {
    let s = login(app).await;
    let nick = format!("Rs{}", &Uuid::new_v4().simple().to_string()[..8]);
    let (status, body) = post(
        app,
        &s,
        "/me/character",
        Some(json!({ "nickname": nick, "look": look(2) })),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    if tickets > 0 {
        give(pool, &s, tickets).await;
    }
    s
}

async fn give(pool: &PgPool, s: &Session, qty: i32) {
    sqlx::query(
        "INSERT INTO cf.inventory (user_id, item_id, qty) VALUES ($1, $2, $3)
         ON CONFLICT (user_id, item_id) DO UPDATE SET qty = cf.inventory.qty + EXCLUDED.qty",
    )
    .bind(Uuid::parse_str(&s.user_id).unwrap())
    .bind(TICKET)
    .bind(qty)
    .execute(pool)
    .await
    .unwrap();
}

async fn tickets(pool: &PgPool, s: &Session) -> i32 {
    sqlx::query_scalar(
        "SELECT COALESCE((SELECT qty FROM cf.inventory WHERE user_id = $1 AND item_id = $2), 0)",
    )
    .bind(Uuid::parse_str(&s.user_id).unwrap())
    .bind(TICKET)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn shirt(app: &axum::Router, s: &Session) -> Value {
    get(app, s, "/me").await.1["user"]["look"]["shirt"].clone()
}

async fn restyle(app: &axum::Router, s: &Session, shirt: u8, key: Option<&str>) -> (StatusCode, Value) {
    post(app, s, "/me/look", Some(json!({ "look": look(shirt) })), key).await
}

#[tokio::test]
async fn test_a_ticket_buys_one_new_look_and_the_name_stays() {
    let (app, pool) = app().await;
    let s = player(&app, &pool, 1).await;
    let name = get(&app, &s, "/me").await.1["user"]["nickname"].clone();
    let (status, body) = restyle(&app, &s, 7, None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["look"]["shirt"], 7);
    assert_eq!(body["nickname"], name);
    assert_eq!(shirt(&app, &s).await, 7);
    assert_eq!(tickets(&pool, &s).await, 0);
    // The next change needs another ticket.
    let (status, body) = restyle(&app, &s, 8, None).await;
    assert_eq!(
        (status, body["error"]["code"].clone()),
        (StatusCode::CONFLICT, json!("no_ticket"))
    );
    assert_eq!(shirt(&app, &s).await, 7);
}

#[tokio::test]
async fn test_refused_changes_spend_nothing() {
    let (app, pool) = app().await;
    let s = player(&app, &pool, 1).await;
    // The same look again.
    let (status, body) = restyle(&app, &s, 2, None).await;
    assert_eq!(
        (status, body["error"]["code"].clone()),
        (StatusCode::CONFLICT, json!("same_look"))
    );
    // A look outside the tables, and a field the contract does not know.
    let (status, body) = restyle(&app, &s, 250, None).await;
    assert_eq!(
        (status, body["error"]["code"].clone()),
        (StatusCode::BAD_REQUEST, json!("invalid_look"))
    );
    let mut odd = look(5);
    odd["wings"] = json!(1);
    let (status, _) = post(&app, &s, "/me/look", Some(json!({ "look": odd })), None).await;
    assert!(status.is_client_error());
    assert_eq!(tickets(&pool, &s).await, 1);
    assert_eq!(shirt(&app, &s).await, 2);
    // No character yet: nothing to change, the ticket stays.
    let fresh = login(&app).await;
    give(&pool, &fresh, 1).await;
    let (status, body) = restyle(&app, &fresh, 5, None).await;
    assert_eq!(
        (status, body["error"]["code"].clone()),
        (StatusCode::CONFLICT, json!("no_character"))
    );
    assert_eq!(tickets(&pool, &fresh).await, 1);
}

#[tokio::test]
async fn test_a_replay_with_the_same_key_spends_one_ticket() {
    let (app, pool) = app().await;
    let s = player(&app, &pool, 2).await;
    let key = Uuid::new_v4().to_string();
    let (first, a) = restyle(&app, &s, 6, Some(&key)).await;
    let (again, b) = restyle(&app, &s, 6, Some(&key)).await;
    assert_eq!((first, again), (StatusCode::OK, StatusCode::OK));
    assert_eq!(a, b);
    assert_eq!(tickets(&pool, &s).await, 1);
}

#[tokio::test]
async fn test_two_parallel_changes_with_one_ticket_have_one_winner() {
    let (app, pool) = app().await;
    let s = player(&app, &pool, 1).await;
    let ((a, _), (b, _)) = tokio::join!(restyle(&app, &s, 6, None), restyle(&app, &s, 9, None));
    let won = [a, b].iter().filter(|st| **st == StatusCode::OK).count();
    assert_eq!(won, 1, "{a} {b}");
    assert!([a, b].contains(&StatusCode::CONFLICT));
    assert_eq!(tickets(&pool, &s).await, 0);
}

#[tokio::test]
async fn test_a_ticket_is_not_credits_and_not_someone_elses() {
    let (app, pool) = app().await;
    let a = player(&app, &pool, 1).await;
    let b = player(&app, &pool, 1).await;
    get(&app, &a, "/island").await;
    let (status, body) = post(
        &app,
        &a,
        "/market/sell",
        Some(json!({ "item_id": TICKET, "qty": 1 })),
        None,
    )
    .await;
    assert!(status.is_client_error(), "{body}");
    assert_eq!(tickets(&pool, &a).await, 1);
    // A changes the look: A's ticket goes, B's stays, B's look is untouched.
    let (status, _) = restyle(&app, &a, 6, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!((tickets(&pool, &a).await, tickets(&pool, &b).await), (0, 1));
    assert_eq!(shirt(&app, &b).await, 2);
    // The catalog sells it for CLASH only, at the owner's price.
    let price: (Option<i32>, Option<i64>) =
        sqlx::query_as("SELECT price_usd_cents, price_credits FROM cf.items WHERE id = $1")
            .bind(TICKET)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(price, (Some(99), None));
}

/// The whole way a player takes: a CLASH order for a ticket (fake chain, real bookkeeping), the
/// ticket in the inventory, then the new look.
#[tokio::test]
async fn test_bought_for_clash_then_spent_on_a_look() {
    let (app, pool) = app_with(true).await;
    let s = player(&app, &pool, 0).await;
    get(&app, &s, "/island").await;
    let (status, quote) = post(
        &app,
        &s,
        "/orders",
        Some(json!({ "item_id": TICKET, "qty": 1 })),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{quote}");
    // $0.99 at the fake $0.001 per CLASH = 990 CLASH (6 decimals), 70 % burned.
    assert_eq!(quote["amount_clash"], 990_000_000_i64);
    assert_eq!(quote["burn_amount"], 693_000_000_i64);
    let order = quote["order_id"].as_str().unwrap().to_owned();
    let mut sig = Vec::with_capacity(64);
    for _ in 0..4 {
        sig.extend_from_slice(Uuid::new_v4().as_bytes());
    }
    let (status, done) = post(
        &app,
        &s,
        &format!("/orders/{order}/submit"),
        Some(json!({ "signature": bs58::encode(sig).into_string() })),
        None,
    )
    .await;
    assert_eq!(
        (status, done["status"].clone()),
        (StatusCode::OK, json!("fulfilled")),
        "{done}"
    );
    assert_eq!(tickets(&pool, &s).await, 1);
    let (status, body) = restyle(&app, &s, 11, None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!((tickets(&pool, &s).await, shirt(&app, &s).await), (0, json!(11)));
}

fn fake_signature() -> String {
    let mut sig = Vec::with_capacity(64);
    for _ in 0..4 {
        sig.extend_from_slice(Uuid::new_v4().as_bytes());
    }
    bs58::encode(sig).into_string()
}

/// Spending a ticket while another one is being delivered: both go through (the ticket row is
/// locked before the user row on both sides, as in the order fulfilment — no deadlock that would
/// park a paid order in the review queue).
#[tokio::test]
async fn test_spending_a_ticket_while_buying_another_never_blocks_the_order() {
    let (app, pool) = app_with(true).await;
    let s = player(&app, &pool, 1).await;
    get(&app, &s, "/island").await;
    for shirt in [6_u8, 7, 8] {
        let (_, quote) = post(
            &app,
            &s,
            "/orders",
            Some(json!({ "item_id": TICKET, "qty": 1 })),
            None,
        )
        .await;
        let order = quote["order_id"].as_str().unwrap().to_owned();
        let path = format!("/orders/{order}/submit");
        let submit = post(
            &app,
            &s,
            &path,
            Some(json!({ "signature": fake_signature() })),
            None,
        );
        let ((paid, done), (changed, body)) = tokio::join!(submit, restyle(&app, &s, shirt, None));
        assert_eq!(
            (paid, done["status"].clone()),
            (StatusCode::OK, json!("fulfilled")),
            "{done}"
        );
        assert_eq!(changed, StatusCode::OK, "{body}");
    }
    assert_eq!(
        tickets(&pool, &s).await,
        1,
        "one owned + three bought − three spent"
    );
}
