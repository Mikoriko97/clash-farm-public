//! Nickname moderation (0031, ADR-040) under races: a reset against a paid rename, two free renames
//! from one reset, a paid rename overtaken by a reset (the ticket stays), a block against a
//! creation of the same name. The deterministic ones hold one side's transaction open while the
//! other waits on its locks. Against local Postgres + Memurai.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::time::Duration;

use axum::http::StatusCode;
use cf_world::moderation;
use common::{app, get, login, post, Session};
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

const TICKET: &str = "rename_ticket";

fn unique(prefix: &str) -> String {
    format!("{prefix}{}", &Uuid::new_v4().simple().to_string()[..8])
}

fn uid(s: &Session) -> Uuid {
    Uuid::parse_str(&s.user_id).unwrap()
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

async fn rename_free(app: &axum::Router, s: &Session) -> Value {
    get(app, s, "/me").await.1["user"]["rename_free"].clone()
}

async fn reset(pool: &PgPool, user: Uuid, expected: &str) -> Result<moderation::Reset, cf_world::WorldError> {
    let mut tx = pool.begin().await.unwrap();
    let done = moderation::reset_nickname(&mut tx, user, expected, "offensive name", "test-admin").await?;
    tx.commit().await.unwrap();
    Ok(done)
}

/// A reset and a paid rename at once: no deadlock; the rename always succeeds, the reset either
/// lands first or finds the name already changed (`WrongState`), and the player never ends up
/// carrying a blocked name.
#[tokio::test]
async fn test_a_reset_racing_a_rename_ends_consistent() {
    let (app, pool) = app().await;
    for _ in 0..4 {
        let first = unique("Race");
        let a = player(&app, &pool, &first, 1).await;
        let next = unique("Next");
        let renaming = tokio::spawn({
            let (app, a) = (app.clone(), a.clone());
            async move { rename(&app, &a, &next).await }
        });
        let resetting = tokio::spawn({
            let (pool, user) = (pool.clone(), uid(&a));
            async move { reset(&pool, user, &first).await }
        });
        let (renamed, reset_done) = tokio::time::timeout(Duration::from_secs(10), async {
            (renaming.await.unwrap(), resetting.await.unwrap())
        })
        .await
        .expect("no deadlock");
        assert_eq!(renamed.0, StatusCode::OK, "{}", renamed.1);
        assert!(
            matches!(reset_done, Ok(_) | Err(cf_world::WorldError::WrongState)),
            "{reset_done:?}"
        );
        let (key, free): (String, bool) = sqlx::query_as(
            "SELECT nickname_key, coalesce((SELECT kind = 'moderation' FROM cf.nickname_history h WHERE h.user_id = u.id ORDER BY h.id DESC LIMIT 1), false) FROM cf.users u WHERE id = $1",
        )
        .bind(uid(&a))
        .fetch_one(&pool)
        .await
        .unwrap();
        let blocked: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM cf.nickname_blocks WHERE key = $1 AND lifted_at IS NULL)",
        )
        .bind(&key)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert!(!blocked, "the current name is never blocked");
        assert_eq!(rename_free(&app, &a).await, json!(free));
    }
}

/// One reset buys one free rename: two at once (no ticket) → exactly one wins.
#[tokio::test]
async fn test_two_free_renames_from_one_reset_one_wins() {
    let (app, pool) = app().await;
    for _ in 0..3 {
        let bad = unique("Twin");
        let a = player(&app, &pool, &bad, 0).await;
        reset(&pool, uid(&a), &bad).await.unwrap();
        let both: Vec<_> = (0..2)
            .map(|_| {
                let (app, a, name) = (app.clone(), a.clone(), unique("Free"));
                tokio::spawn(async move { rename(&app, &a, &name).await })
            })
            .collect();
        let mut codes = Vec::new();
        for task in both {
            let (status, body) = task.await.unwrap();
            codes.push((status, body["error"]["code"].clone()));
        }
        codes.sort_by_key(|c| c.0.as_u16());
        assert_eq!(
            codes,
            [
                (StatusCode::OK, Value::Null),
                (StatusCode::CONFLICT, json!("no_ticket"))
            ],
            "one free rename per reset"
        );
        assert_eq!(rename_free(&app, &a).await, json!(false));
    }
}

/// A paid rename that started before a reset committed (it took the ticket, then waited on the
/// player's row): under the lock it sees the rename is free now, rolls the ticket back and renames
/// for free.
#[tokio::test]
async fn test_a_rename_overtaken_by_a_reset_keeps_the_ticket() {
    let (app, pool) = app().await;
    let bad = unique("Slow");
    let a = player(&app, &pool, &bad, 1).await;
    let mut held = pool.begin().await.unwrap();
    moderation::reset_nickname(&mut held, uid(&a), &bad, "offensive name", "test-admin")
        .await
        .unwrap();
    let name = unique("Fresh");
    let renaming = tokio::spawn({
        let (app, a, name) = (app.clone(), a.clone(), name.clone());
        async move { rename(&app, &a, &name).await }
    });
    tokio::time::sleep(Duration::from_millis(300)).await;
    held.commit().await.unwrap();
    let (status, body) = renaming.await.unwrap();
    assert_eq!(
        (status, body["nickname"].clone()),
        (StatusCode::OK, json!(name)),
        "{body}"
    );
    assert_eq!(
        tickets(&pool, &a).await,
        1,
        "the free rename was used, not the ticket"
    );
    assert_eq!(rename_free(&app, &a).await, json!(false));
}

/// A block committed while a creation of that name waits on the key lock: the creation sees it.
#[tokio::test]
async fn test_a_block_wins_over_a_creation_waiting_on_it() {
    let (app, pool) = app().await;
    let name = unique("Held");
    let mut held = pool.begin().await.unwrap();
    moderation::block_name(&mut held, &name, "impersonation", "test-admin")
        .await
        .unwrap()
        .unwrap();
    let s = login(&app).await;
    let creating = tokio::spawn({
        let (app, s, name) = (app.clone(), s.clone(), name.clone());
        async move { create(&app, &s, &name).await }
    });
    tokio::time::sleep(Duration::from_millis(300)).await;
    held.commit().await.unwrap();
    let (status, body) = creating.await.unwrap();
    assert_eq!(
        (status, body["error"]["code"].clone()),
        (StatusCode::BAD_REQUEST, json!("nickname_not_allowed")),
        "{body}"
    );
}
