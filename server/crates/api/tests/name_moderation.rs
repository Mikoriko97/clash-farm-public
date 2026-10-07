//! Nickname moderation (0031, ADR-040), the game's side: a moderator's reset gives the player a
//! `Farmer_` placeholder and one free rename; the old name — and every lookalike of it — is
//! blocked for everybody (creation, rename, live check) until an admin lifts the block; players
//! cannot pick the placeholder prefix; a block is never deleted; an online player's socket learns
//! the new name. Races: `name_moderation_races.rs`; the admin endpoints: cf-admin's
//! `admin_names.rs`. Against local Postgres + Memurai.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::net::SocketAddr;
use std::time::{Duration, Instant};

use axum::http::StatusCode;
use cf_world::moderation;
use common::{app, get, login, post, Session};
use futures_util::StreamExt;
use serde_json::{json, Value};
use sqlx::PgPool;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Message;
use uuid::Uuid;

const TICKET: &str = "rename_ticket";

fn unique(prefix: &str) -> String {
    format!("{prefix}{}", &Uuid::new_v4().simple().to_string()[..8])
}

fn uid(s: &Session) -> Uuid {
    Uuid::parse_str(&s.user_id).unwrap()
}

fn code(reply: &(StatusCode, Value)) -> (StatusCode, Value) {
    (reply.0, reply.1["error"]["code"].clone())
}

async fn create(app: &axum::Router, s: &Session, name: &str) -> (StatusCode, Value) {
    let look = json!({ "skin": 1, "shirt": 2, "pants": 3, "shoes": 4 });
    post(
        app,
        s,
        "/me/character",
        Some(json!({ "nickname": name, "look": look })),
        None,
    )
    .await
}

/// A player with a character called `name` and `tickets` rename tickets.
async fn player(app: &axum::Router, pool: &PgPool, name: &str, tickets: i32) -> Session {
    let s = login(app).await;
    let made = create(app, &s, name).await;
    assert_eq!(made.0, StatusCode::OK, "{}", made.1);
    if tickets > 0 {
        sqlx::query("INSERT INTO cf.inventory (user_id, item_id, qty) VALUES ($1, $2, $3)")
            .bind(uid(&s))
            .bind(TICKET)
            .bind(tickets)
            .execute(pool)
            .await
            .unwrap();
    }
    s
}

async fn tickets(pool: &PgPool, s: &Session) -> i32 {
    sqlx::query_scalar(
        "SELECT COALESCE((SELECT qty FROM cf.inventory WHERE user_id = $1 AND item_id = $2), 0)",
    )
    .bind(uid(s))
    .bind(TICKET)
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn rename(app: &axum::Router, s: &Session, name: &str) -> (StatusCode, Value) {
    post(app, s, "/me/nickname", Some(json!({ "nickname": name })), None).await
}

async fn me(app: &axum::Router, s: &Session) -> Value {
    get(app, s, "/me").await.1["user"].clone()
}

/// What the admin app does, minus the session and the audit row: resets the name the player has now.
async fn reset(pool: &PgPool, user: Uuid) -> moderation::Reset {
    let current: String = sqlx::query_scalar("SELECT nickname FROM cf.users WHERE id = $1")
        .bind(user)
        .fetch_one(pool)
        .await
        .unwrap();
    let mut tx = pool.begin().await.unwrap();
    let done = moderation::reset_nickname(&mut tx, user, &current, "offensive name", "test-admin")
        .await
        .unwrap();
    tx.commit().await.unwrap();
    done
}

async fn live_blocks(pool: &PgPool, key: &str) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM cf.nickname_blocks WHERE key = $1 AND lifted_at IS NULL")
        .bind(key)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn test_a_reset_gives_a_placeholder_and_one_free_rename() {
    let (app, pool) = app().await;
    let a = player(&app, &pool, &unique("Rude"), 0).await;
    let done = reset(&pool, uid(&a)).await;
    assert!(!done.repeated);
    let view = me(&app, &a).await;
    assert_eq!(
        (view["nickname"].clone(), view["rename_free"].clone()),
        (json!(done.new), json!(true))
    );
    let new = done.new.strip_prefix("Farmer_").unwrap();
    assert!(
        new.len() == 8 && new.chars().all(|c| c.is_ascii_hexdigit()),
        "{}",
        done.new
    );
    // The free rename needs no ticket, and there is only one.
    let first = unique("Kind");
    let renamed = rename(&app, &a, &first).await;
    assert_eq!(renamed.0, StatusCode::OK, "{}", renamed.1);
    assert_eq!(me(&app, &a).await["rename_free"], json!(false));
    assert_eq!(
        code(&rename(&app, &a, &unique("More")).await),
        (StatusCode::CONFLICT, json!("no_ticket"))
    );
    let kinds: Vec<String> =
        sqlx::query_scalar("SELECT kind FROM cf.nickname_history WHERE user_id = $1 ORDER BY id")
            .bind(uid(&a))
            .fetch_all(&pool)
            .await
            .unwrap();
    assert_eq!(kinds, ["moderation", "rename"]);
}

#[tokio::test]
async fn test_the_reset_name_is_blocked_for_everyone_lookalikes_too() {
    let (app, pool) = app().await;
    let bad = unique("Bob");
    let a = player(&app, &pool, &bad, 1).await;
    reset(&pool, uid(&a)).await;
    let twin = bad.replacen('o', "0", 1);
    let b = login(&app).await;
    for name in [&bad, &twin] {
        assert_eq!(
            code(&create(&app, &b, name).await),
            (StatusCode::BAD_REQUEST, json!("nickname_not_allowed")),
            "{name}"
        );
        let (_, check) = get(&app, &b, &format!("/me/nickname-check?nick={name}")).await;
        assert_eq!(check["reason"], "not_allowed", "{name}: {check}");
    }
    // Not even the former owner gets it back — with a ticket or with the free rename.
    assert_eq!(
        code(&rename(&app, &a, &bad).await),
        (StatusCode::BAD_REQUEST, json!("nickname_not_allowed"))
    );
    assert_eq!(
        me(&app, &a).await["rename_free"],
        json!(true),
        "a refusal spends nothing"
    );
    assert_eq!(tickets(&pool, &a).await, 1);
    let c = player(&app, &pool, &unique("Cat"), 1).await;
    assert_eq!(
        code(&rename(&app, &c, &twin).await),
        (StatusCode::BAD_REQUEST, json!("nickname_not_allowed"))
    );
    assert_eq!(tickets(&pool, &c).await, 1, "a refusal keeps the ticket");
}

#[tokio::test]
async fn test_a_repeat_reset_changes_nothing() {
    let (app, pool) = app().await;
    let bad = unique("Twice");
    let a = player(&app, &pool, &bad, 0).await;
    let first = reset(&pool, uid(&a)).await;
    let again = reset(&pool, uid(&a)).await;
    assert!(again.repeated);
    assert_eq!(
        (again.old.as_str(), again.new.as_str()),
        (bad.as_str(), first.new.as_str())
    );
    let rows: i64 = sqlx::query_scalar("SELECT count(*) FROM cf.nickname_history WHERE user_id = $1")
        .bind(uid(&a))
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(rows, 1);
    let placeholder = cf_world::nickname::key_of(&first.new);
    assert_eq!(
        live_blocks(&pool, &placeholder).await,
        0,
        "the placeholder is never blocked"
    );
    // No character yet: nothing to reset.
    let fresh = login(&app).await;
    let mut tx = pool.begin().await.unwrap();
    let none = moderation::reset_nickname(&mut tx, uid(&fresh), "", "r", "test-admin").await;
    assert!(matches!(none, Err(cf_world::WorldError::NoCharacter)), "{none:?}");
}

#[tokio::test]
async fn test_a_name_blocked_ahead_is_refused_and_a_lift_frees_it() {
    let (app, pool) = app().await;
    let name = unique("Ahead");
    let mut tx = pool.begin().await.unwrap();
    let block = moderation::block_name(&mut tx, &name, "impersonation", "test-admin")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(block.holder, None);
    tx.commit().await.unwrap();
    let mut tx = pool.begin().await.unwrap();
    let twice = moderation::block_name(&mut tx, &name.to_uppercase(), "again", "test-admin")
        .await
        .unwrap();
    assert!(twice.is_none(), "one live block per key");
    drop(tx);
    let b = login(&app).await;
    assert_eq!(
        code(&create(&app, &b, &name).await),
        (StatusCode::BAD_REQUEST, json!("nickname_not_allowed"))
    );
    let mut tx = pool.begin().await.unwrap();
    assert_eq!(
        moderation::lift_block(&mut tx, block.id, "test-admin")
            .await
            .unwrap(),
        name
    );
    tx.commit().await.unwrap();
    let mut tx = pool.begin().await.unwrap();
    let lifted = moderation::lift_block(&mut tx, block.id, "test-admin").await;
    assert!(
        matches!(lifted, Err(cf_world::WorldError::NotFound)),
        "a lift happens once: {lifted:?}"
    );
    drop(tx);
    let made = create(&app, &b, &name).await;
    assert_eq!(made.0, StatusCode::OK, "{}", made.1);
    // Blocking a name somebody carries reports the holder (the admin resets it separately).
    let mut tx = pool.begin().await.unwrap();
    let held = moderation::block_name(&mut tx, &name, "later", "test-admin")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(held.holder, Some(uid(&b)));
}

#[tokio::test]
async fn test_players_cannot_take_the_placeholder_prefix() {
    let (app, _) = app().await;
    let s = login(&app).await;
    for name in ["Farmer_1a2b3c4d", "farmer_bob"] {
        assert_eq!(
            code(&create(&app, &s, name).await),
            (StatusCode::BAD_REQUEST, json!("nickname_not_allowed")),
            "{name}"
        );
    }
}

#[tokio::test]
async fn test_blocks_are_lift_only() {
    let (_, pool) = app().await;
    let mut tx = pool.begin().await.unwrap();
    let block = moderation::block_name(&mut tx, &unique("Keep"), "test", "test-admin")
        .await
        .unwrap()
        .unwrap();
    tx.commit().await.unwrap();
    for sql in [
        "DELETE FROM cf.nickname_blocks WHERE id = $1",
        "UPDATE cf.nickname_blocks SET reason = 'other' WHERE id = $1",
        "UPDATE cf.nickname_blocks SET key = 'other' WHERE id = $1",
    ] {
        assert!(
            sqlx::query(sql).bind(block.id).execute(&pool).await.is_err(),
            "{sql}"
        );
    }
    let mut tx = pool.begin().await.unwrap();
    moderation::lift_block(&mut tx, block.id, "test-admin")
        .await
        .unwrap();
    tx.commit().await.unwrap();
    let relift =
        sqlx::query("UPDATE cf.nickname_blocks SET lifted_at = now(), lifted_by = 'x' WHERE id = $1")
            .bind(block.id)
            .execute(&pool)
            .await;
    assert!(relift.is_err(), "a lifted block stays as it was lifted");
}

async fn serve(app: axum::Router) -> SocketAddr {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    addr
}

/// The socket closes with `{"t":"identity"}` within one session check (30 s) of the reset, and
/// the next socket introduces the player by the new name.
#[tokio::test]
async fn test_an_online_player_learns_the_name_changed() {
    let (app, pool) = app().await;
    let a = player(&app, &pool, &unique("Live"), 0).await;
    let addr = serve(app).await;
    let open = || async {
        let mut request = format!("ws://{addr}/api/v1/ws/presence")
            .into_client_request()
            .unwrap();
        request
            .headers_mut()
            .insert("origin", "http://localhost:5173".parse().unwrap());
        request
            .headers_mut()
            .insert("cookie", a.cookie().parse().unwrap());
        tokio_tungstenite::connect_async(request).await.unwrap().0
    };
    let mut ws = open().await;
    let done = reset(&pool, uid(&a)).await;
    let deadline = Instant::now() + Duration::from_secs(40);
    let mut told = false;
    while Instant::now() < deadline && !told {
        match tokio::time::timeout(Duration::from_secs(2), ws.next()).await {
            Ok(Some(Ok(Message::Text(t)))) => {
                told = serde_json::from_str::<Value>(&t).unwrap()["t"] == "identity"
            }
            Ok(Some(Ok(_))) | Err(_) => {}
            Ok(None | Some(Err(_))) => break,
        }
    }
    assert!(told, "the socket should announce the new identity");
    let mut again = open().await;
    let hello = loop {
        if let Some(Ok(Message::Text(t))) = again.next().await {
            break serde_json::from_str::<Value>(&t).unwrap();
        }
    };
    assert_eq!(
        (hello["t"].clone(), hello["nick"].clone()),
        (json!("hello"), json!(done.new))
    );
}
