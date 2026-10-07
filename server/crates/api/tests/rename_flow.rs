//! Paid nickname change (0030, ADR-039): `POST /me/nickname` spends one `rename_ticket` (CLASH,
//! $5) and takes the new name under the creation rules; the old name stays with its owner for 30
//! days (nobody else can create or rename into it), the change is in an append-only history.
//! Refusals spend nothing, a replay spends nothing more, races have one winner and the loser keeps
//! the ticket. Against local Postgres + Memurai.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use axum::http::StatusCode;
use common::{app, app_with, get, login, post, Session};
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

const TICKET: &str = "rename_ticket";

fn look() -> Value {
    json!({ "skin": 1, "shirt": 2, "pants": 3, "shoes": 4 })
}

fn unique(prefix: &str) -> String {
    format!("{prefix}{}", &Uuid::new_v4().simple().to_string()[..8])
}

/// A player with a character called `name` and `tickets` rename tickets.
async fn player(app: &axum::Router, pool: &PgPool, name: &str, tickets: i32) -> Session {
    let s = login(app).await;
    let (status, body) = post(
        app,
        &s,
        "/me/character",
        Some(json!({ "nickname": name, "look": look() })),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    give(pool, &s, tickets).await;
    s
}

async fn give(pool: &PgPool, s: &Session, qty: i32) {
    if qty < 1 {
        return;
    }
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

async fn name_of(app: &axum::Router, s: &Session) -> Value {
    get(app, s, "/me").await.1["user"]["nickname"].clone()
}

async fn rename(app: &axum::Router, s: &Session, name: &str, key: Option<&str>) -> (StatusCode, Value) {
    post(app, s, "/me/nickname", Some(json!({ "nickname": name })), key).await
}

fn code(reply: &(StatusCode, Value)) -> (StatusCode, Value) {
    (reply.0, reply.1["error"]["code"].clone())
}

#[tokio::test]
async fn test_a_ticket_buys_a_new_name_and_the_old_one_stays_held() {
    let (app, pool) = app().await;
    let old = unique("Old");
    let a = player(&app, &pool, &old, 2).await;
    let new = unique("New");
    let (status, body) = rename(&app, &a, &new, None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        (body["nickname"].clone(), name_of(&app, &a).await),
        (json!(new), json!(new))
    );
    assert_eq!(tickets(&pool, &a).await, 1);
    let history: (String, String) =
        sqlx::query_as("SELECT old_nickname, new_nickname FROM cf.nickname_history WHERE user_id = $1::uuid")
            .bind(&a.user_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(history, (old.clone(), new.clone()));
    // Nobody else takes the old name: not a new character, not a rename, not even a lookalike.
    let fresh = login(&app).await;
    let made = post(
        &app,
        &fresh,
        "/me/character",
        Some(json!({ "nickname": old.to_uppercase(), "look": look() })),
        None,
    )
    .await;
    assert_eq!(code(&made), (StatusCode::CONFLICT, json!("nickname_taken")));
    let b = player(&app, &pool, &unique("Bee"), 1).await;
    assert_eq!(
        code(&rename(&app, &b, &old, None).await),
        (StatusCode::CONFLICT, json!("nickname_taken"))
    );
    assert_eq!(tickets(&pool, &b).await, 1, "a refused rename keeps the ticket");
    let (_, checked) = get(&app, &b, &format!("/me/nickname-check?nick={old}")).await;
    assert_eq!(checked["reason"], "taken");
    // The former owner can come back to it.
    let (status, body) = rename(&app, &a, &old, None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(tickets(&pool, &a).await, 0);
}

#[tokio::test]
async fn test_the_hold_ends_after_thirty_days_and_the_history_is_append_only() {
    let (app, pool) = app().await;
    let old = unique("Gone");
    let a = player(&app, &pool, &old, 1).await;
    assert_eq!(rename(&app, &a, &unique("Next"), None).await.0, StatusCode::OK);
    // History cannot be rewritten (not even to age a hold)…
    let edit = sqlx::query(
        "UPDATE cf.nickname_history SET changed_at = now() - interval '31 days' WHERE user_id = $1::uuid",
    )
    .bind(&a.user_id)
    .execute(&pool)
    .await;
    assert!(edit.is_err(), "the history is append-only");
    // …so the test ages it the only way the trigger allows: drop the trigger for one statement in a transaction.
    let mut tx = pool.begin().await.unwrap();
    sqlx::query("ALTER TABLE cf.nickname_history DISABLE TRIGGER nickname_history_append_only")
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query(
        "UPDATE cf.nickname_history SET changed_at = now() - interval '31 days' WHERE user_id = $1::uuid",
    )
    .bind(&a.user_id)
    .execute(&mut *tx)
    .await
    .unwrap();
    sqlx::query("ALTER TABLE cf.nickname_history ENABLE TRIGGER nickname_history_append_only")
        .execute(&mut *tx)
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let fresh = login(&app).await;
    let made = post(
        &app,
        &fresh,
        "/me/character",
        Some(json!({ "nickname": old, "look": look() })),
        None,
    )
    .await;
    assert_eq!(made.0, StatusCode::OK, "{}", made.1);
}

#[tokio::test]
async fn test_refused_renames_spend_nothing() {
    let (app, pool) = app().await;
    let name = unique("Keep");
    let a = player(&app, &pool, &name, 1).await;
    let other = unique("Taken");
    player(&app, &pool, &other, 0).await;
    assert_eq!(
        code(&rename(&app, &a, &name, None).await),
        (StatusCode::CONFLICT, json!("same_nickname"))
    );
    assert_eq!(
        code(&rename(&app, &a, "x", None).await),
        (StatusCode::BAD_REQUEST, json!("invalid_nickname"))
    );
    assert_eq!(
        code(&rename(&app, &a, "AdminBob", None).await),
        (StatusCode::BAD_REQUEST, json!("nickname_not_allowed"))
    );
    assert_eq!(
        code(&rename(&app, &a, &other.to_lowercase(), None).await),
        (StatusCode::CONFLICT, json!("nickname_taken"))
    );
    assert_eq!(
        (tickets(&pool, &a).await, name_of(&app, &a).await),
        (1, json!(name))
    );
    // No ticket, no character: refused too.
    let poor = player(&app, &pool, &unique("Poor"), 0).await;
    assert_eq!(
        code(&rename(&app, &poor, &unique("Rich"), None).await),
        (StatusCode::CONFLICT, json!("no_ticket"))
    );
    let fresh = login(&app).await;
    give(&pool, &fresh, 1).await;
    assert_eq!(
        code(&rename(&app, &fresh, &unique("Nobody"), None).await),
        (StatusCode::CONFLICT, json!("no_character"))
    );
    assert_eq!(tickets(&pool, &fresh).await, 1);
    // Another capitalisation of one's own name is a rename (charged), not a conflict.
    let (status, body) = rename(&app, &a, &name.to_lowercase(), None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
}

#[tokio::test]
async fn test_a_replay_spends_one_ticket_and_races_have_one_winner() {
    let (app, pool) = app().await;
    let a = player(&app, &pool, &unique("Rep"), 3).await;
    let key = Uuid::new_v4().to_string();
    let new = unique("Once");
    let (first, x) = rename(&app, &a, &new, Some(&key)).await;
    let (again, y) = rename(&app, &a, &new, Some(&key)).await;
    assert_eq!((first, again, x == y), (StatusCode::OK, StatusCode::OK, true));
    assert_eq!(tickets(&pool, &a).await, 2);
    // Two players, one free name: one gets it, the other keeps the ticket and the old name.
    let b_name = unique("Bee");
    let b = player(&app, &pool, &b_name, 1).await;
    let wanted = unique("Prize");
    let (ra, rb) = tokio::join!(rename(&app, &a, &wanted, None), rename(&app, &b, &wanted, None));
    let wins = [ra.0, rb.0].iter().filter(|s| **s == StatusCode::OK).count();
    assert_eq!(wins, 1, "{:?} {:?}", ra, rb);
    let loser = if ra.0 == StatusCode::OK {
        (&b, 1, json!(b_name))
    } else {
        (&a, 2, json!(new))
    };
    assert_eq!(
        (tickets(&pool, loser.0).await, name_of(&app, loser.0).await),
        (loser.1, loser.2)
    );
    // One ticket, two renames at once: one.
    let c = player(&app, &pool, &unique("Cee"), 1).await;
    let (one, two) = (unique("One"), unique("Two"));
    let (r1, r2) = tokio::join!(rename(&app, &c, &one, None), rename(&app, &c, &two, None));
    assert_eq!([r1.0, r2.0].iter().filter(|s| **s == StatusCode::OK).count(), 1);
    assert_eq!(tickets(&pool, &c).await, 0);
}

#[tokio::test]
async fn test_bought_for_clash_only_and_never_credits() {
    let (app, pool) = app_with(true).await;
    let a = player(&app, &pool, &unique("Buy"), 0).await;
    get(&app, &a, "/island").await;
    let sold = post(
        &app,
        &a,
        "/market/sell",
        Some(json!({ "item_id": TICKET, "qty": 1 })),
        None,
    )
    .await;
    assert!(sold.0.is_client_error());
    let (status, quote) = post(
        &app,
        &a,
        "/orders",
        Some(json!({ "item_id": TICKET, "qty": 1 })),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{quote}");
    // $5.00 at the fake $0.001 per CLASH = 5 000 CLASH (6 decimals), 70 % burned.
    assert_eq!(
        (quote["amount_clash"].clone(), quote["burn_amount"].clone()),
        (json!(5_000_000_000_i64), json!(3_500_000_000_i64))
    );
    let order = quote["order_id"].as_str().unwrap().to_owned();
    let mut sig = Vec::with_capacity(64);
    for _ in 0..4 {
        sig.extend_from_slice(Uuid::new_v4().as_bytes());
    }
    let path = format!("/orders/{order}/submit");
    let (_, done) = post(
        &app,
        &a,
        &path,
        Some(json!({ "signature": bs58::encode(sig).into_string() })),
        None,
    )
    .await;
    assert_eq!(done["status"], "fulfilled", "{done}");
    let new = unique("Paid");
    assert_eq!(rename(&app, &a, &new, None).await.0, StatusCode::OK);
    assert_eq!(
        (tickets(&pool, &a).await, name_of(&app, &a).await),
        (0, json!(new))
    );
    let price: (Option<i32>, Option<i64>) =
        sqlx::query_as("SELECT price_usd_cents, price_credits FROM cf.items WHERE id = $1")
            .bind(TICKET)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(price, (Some(500), None));
}

/// The hold holds under a race, deterministically: a transaction gives name K up exactly as a
/// rename does (key lock, new name, history row) and stays open while a new character asks for K;
/// once it commits, the creation — which waited on the key lock — sees the hold. Without the lock
/// in `character::create` the creation would check the hold too early and take K right after the
/// commit (the `UNIQUE` index only waits for the release, then lets it through).
#[tokio::test]
async fn test_the_hold_cannot_be_raced() {
    let (app, pool) = app().await;
    let k = unique("Held");
    let a = player(&app, &pool, &k, 0).await;
    let away = unique("Away");
    let mut tx = pool.begin().await.unwrap();
    let key = cf_world::nickname::key_of(&k);
    sqlx::query("SELECT pg_advisory_xact_lock(39, hashtext($1))")
        .bind(&key)
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("UPDATE cf.users SET nickname = $2, nickname_key = $3 WHERE id = $1::uuid")
        .bind(&a.user_id)
        .bind(&away)
        .bind(cf_world::nickname::key_of(&away))
        .execute(&mut *tx)
        .await
        .unwrap();
    sqlx::query("INSERT INTO cf.nickname_history (user_id, old_nickname, old_key, new_nickname, new_key) VALUES ($1::uuid, $2, $3, $4, $5)")
        .bind(&a.user_id)
        .bind(&k)
        .bind(&key)
        .bind(&away)
        .bind(cf_world::nickname::key_of(&away))
        .execute(&mut *tx)
        .await
        .unwrap();
    let fresh = login(&app).await;
    let body = json!({ "nickname": k.clone(), "look": look() });
    let creation = tokio::spawn({
        let app = app.clone();
        async move { post(&app, &fresh, "/me/character", Some(body), None).await }
    });
    tokio::time::sleep(std::time::Duration::from_millis(300)).await;
    tx.commit().await.unwrap();
    let made = creation.await.unwrap();
    assert_eq!(
        code(&made),
        (StatusCode::CONFLICT, json!("nickname_taken")),
        "{}",
        made.1
    );
}

/// Swapping names at once: each name is either still taken or just held — both are refused
/// cleanly (409, no deadlock turned into a 500), both keep their tickets.
#[tokio::test]
async fn test_a_simultaneous_swap_is_refused_without_a_deadlock() {
    let (app, pool) = app().await;
    let (x, y) = (unique("Ex"), unique("Why"));
    let a = player(&app, &pool, &x, 1).await;
    let b = player(&app, &pool, &y, 1).await;
    let (ra, rb) = tokio::join!(rename(&app, &a, &y, None), rename(&app, &b, &x, None));
    assert_eq!(
        (ra.0, rb.0),
        (StatusCode::CONFLICT, StatusCode::CONFLICT),
        "{:?} {:?}",
        ra,
        rb
    );
    assert_eq!((tickets(&pool, &a).await, tickets(&pool, &b).await), (1, 1));
}

/// Spending a rename ticket while another one is being delivered: both go through (ticket row,
/// then user row, on both sides — the order is never parked in the review queue by a deadlock).
#[tokio::test]
async fn test_renaming_while_buying_another_ticket_never_blocks_the_order() {
    let (app, pool) = app_with(true).await;
    let s = player(&app, &pool, &unique("Twice"), 1).await;
    get(&app, &s, "/island").await;
    for _ in 0..3 {
        let (_, quote) = post(
            &app,
            &s,
            "/orders",
            Some(json!({ "item_id": TICKET, "qty": 1 })),
            None,
        )
        .await;
        let order = quote["order_id"].as_str().unwrap().to_owned();
        let mut sig = Vec::with_capacity(64);
        for _ in 0..4 {
            sig.extend_from_slice(Uuid::new_v4().as_bytes());
        }
        let path = format!("/orders/{order}/submit");
        let next = unique("Step");
        let submit = post(
            &app,
            &s,
            &path,
            Some(json!({ "signature": bs58::encode(sig).into_string() })),
            None,
        );
        let ((paid, done), (renamed, body)) = tokio::join!(submit, rename(&app, &s, &next, None));
        assert_eq!(
            (paid, done["status"].clone()),
            (StatusCode::OK, json!("fulfilled")),
            "{done}"
        );
        assert_eq!(renamed, StatusCode::OK, "{body}");
    }
    assert_eq!(
        tickets(&pool, &s).await,
        1,
        "one owned + three bought - three spent"
    );
}
