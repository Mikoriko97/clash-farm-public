//! Nickname moderation in the admin app (0031, ADR-040): reset a player's name with a reason,
//! block names ahead of time, lift blocks. Admin session + CSRF on every mutation, every change
//! audited once (a repeat reset audits nothing), unknown targets are 404. The game-side rules
//! (refusals, free rename, races, the socket) are in cf-api's `name_moderation.rs`.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use axum::http::StatusCode;
use common::*;
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

fn unique(prefix: &str) -> String {
    format!("{prefix}{}", &Uuid::new_v4().simple().to_string()[..8])
}

/// A player with a character called `name`, straight in the database.
async fn player(pool: &PgPool, name: &str) -> Uuid {
    let wallet = format!("sol:{}", bs58::encode(rand_bytes()).into_string());
    sqlx::query_scalar(
        "INSERT INTO cf.users (wallet, nickname, nickname_key, look) VALUES ($1, $2, $3, $4) RETURNING id",
    )
    .bind(&wallet)
    .bind(name)
    .bind(cf_world::nickname::key_of(name))
    .bind(json!({ "skin": 1, "shirt": 2, "pants": 3, "shoes": 4 }))
    .fetch_one(pool)
    .await
    .unwrap()
}

async fn audits(pool: &PgPool, action: &str, target: &str) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM cf.audit_log WHERE action = $1 AND target = $2")
        .bind(action)
        .bind(target)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn test_a_reset_needs_a_session_csrf_and_a_reason_and_is_audited_once() {
    let h = harness().await;
    let (cookie, csrf) = login(&h, &h.key, &h.wallet).await.unwrap();
    let bad = unique("Rude");
    let id = player(&h.pool, &bad).await;
    let path = format!("/api/players/{id}/nickname-reset");
    let body = json!({ "nickname": bad, "reason": "offensive" });
    assert_eq!(
        post_json(&h.app, "", "x", &path, body.clone()).await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        post_json(&h.app, &cookie, "bad", &path, body.clone()).await.0,
        StatusCode::FORBIDDEN
    );
    let (status, _) = post_json(
        &h.app,
        &cookie,
        &csrf,
        &path,
        json!({ "nickname": bad, "reason": "  " }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "the reason is required");
    let ghost = format!("/api/players/{}/nickname-reset", Uuid::new_v4());
    assert_eq!(
        post_json(&h.app, &cookie, &csrf, &ghost, body.clone()).await.0,
        StatusCode::NOT_FOUND
    );
    let (status, done) = post_json(&h.app, &cookie, &csrf, &path, body.clone()).await;
    assert_eq!(status, StatusCode::OK, "{done}");
    assert_eq!(
        (done["old"].clone(), done["repeated"].clone()),
        (json!(bad), json!(false))
    );
    let new = done["new"].as_str().unwrap().to_owned();
    assert!(new.starts_with("Farmer_"), "{new}");
    let now: String = sqlx::query_scalar("SELECT nickname FROM cf.users WHERE id = $1")
        .bind(id)
        .fetch_one(&h.pool)
        .await
        .unwrap();
    assert_eq!(now, new);
    // A double click: the same answer, no second audit row, the placeholder not blocked.
    let (status, again) = post_json(&h.app, &cookie, &csrf, &path, body).await;
    assert_eq!(
        (status, again["new"].clone(), again["repeated"].clone()),
        (StatusCode::OK, json!(new), json!(true))
    );
    assert_eq!(audits(&h.pool, "player_nickname_reset", &id.to_string()).await, 1);
    // The player has used the free rename; a late second click (the first answer got lost) names
    // the old name: 409, the clean new name is neither reset nor blocked.
    let clean = unique("Clean");
    cf_world::rename::rename(&h.pool, id, &clean).await.unwrap();
    let (status, _) = post_json(
        &h.app,
        &cookie,
        &csrf,
        &path,
        json!({ "nickname": bad, "reason": "offensive" }),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::CONFLICT,
        "the name changed since the card was loaded"
    );
    let now: String = sqlx::query_scalar("SELECT nickname FROM cf.users WHERE id = $1")
        .bind(id)
        .fetch_one(&h.pool)
        .await
        .unwrap();
    assert_eq!(now, clean);
    assert_eq!(audits(&h.pool, "player_nickname_reset", &id.to_string()).await, 1);
    // The card shows the reset; the blocks list has the old name with its reason.
    let (_, card) = get_json(&h.app, &cookie, &format!("/api/players/{id}")).await;
    let names: Vec<(Value, Value)> = card["names"]
        .as_array()
        .unwrap()
        .iter()
        .map(|n| (n["kind"].clone(), n["old_nickname"].clone()))
        .collect();
    assert_eq!(
        names,
        [(json!("rename"), json!(new)), (json!("moderation"), json!(bad))]
    );
    let (status, list) = get_json(&h.app, &cookie, &format!("/api/name-blocks?q={bad}")).await;
    assert_eq!(status, StatusCode::OK, "{list}");
    let rows = list["rows"].as_array().unwrap();
    assert_eq!(rows.len(), 1, "{list}");
    assert_eq!(
        (rows[0]["reason"].clone(), rows[0]["blocked_by"].clone()),
        (json!("offensive"), json!(h.wallet))
    );
    // No character yet: nothing to reset.
    let bare: Uuid = sqlx::query_scalar("INSERT INTO cf.users (wallet) VALUES ($1) RETURNING id")
        .bind(format!("sol:{}", bs58::encode(rand_bytes()).into_string()))
        .fetch_one(&h.pool)
        .await
        .unwrap();
    let (status, _) = post_json(
        &h.app,
        &cookie,
        &csrf,
        &format!("/api/players/{bare}/nickname-reset"),
        json!({ "nickname": "Nobody", "reason": "r" }),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT);
}

#[tokio::test]
async fn test_names_are_blocked_ahead_and_lifted_once_with_audit() {
    let h = harness().await;
    let (cookie, csrf) = login(&h, &h.key, &h.wallet).await.unwrap();
    let name = unique("Fake");
    let holder = player(&h.pool, &name).await;
    let make = json!({ "nickname": name, "reason": "impersonation" });
    assert_eq!(
        get_json(&h.app, "", "/api/name-blocks").await.0,
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        post_json(&h.app, &cookie, "bad", "/api/name-blocks", make.clone())
            .await
            .0,
        StatusCode::FORBIDDEN
    );
    let (status, made) = post_json(&h.app, &cookie, &csrf, "/api/name-blocks", make.clone()).await;
    assert_eq!(status, StatusCode::OK, "{made}");
    assert_eq!(
        made["holder"],
        json!(holder),
        "the admin learns who carries the name"
    );
    let block_id = made["id"].as_i64().unwrap();
    let (status, _) = post_json(&h.app, &cookie, &csrf, "/api/name-blocks", make).await;
    assert_eq!(status, StatusCode::CONFLICT, "one live block per name");
    assert_eq!(audits(&h.pool, "nickname_block", &name).await, 1);
    // Rule-refused and malformed names: nothing to block.
    for (bad, want) in [
        ("AdminTom", StatusCode::CONFLICT),
        ("a b", StatusCode::BAD_REQUEST),
    ] {
        let (status, body) = post_json(
            &h.app,
            &cookie,
            &csrf,
            "/api/name-blocks",
            json!({ "nickname": bad, "reason": "r" }),
        )
        .await;
        assert_eq!(status, want, "{bad}: {body}");
    }
    let lift = format!("/api/name-blocks/{block_id}/lift");
    assert_eq!(
        post_json(&h.app, &cookie, "bad", &lift, Value::Null).await.0,
        StatusCode::FORBIDDEN
    );
    let (status, body) = post_json(&h.app, &cookie, &csrf, &lift, Value::Null).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        post_json(&h.app, &cookie, &csrf, &lift, Value::Null).await.0,
        StatusCode::NOT_FOUND
    );
    let ghost = format!("/api/name-blocks/{}/lift", i64::MAX);
    assert_eq!(
        post_json(&h.app, &cookie, &csrf, &ghost, Value::Null).await.0,
        StatusCode::NOT_FOUND
    );
    assert_eq!(audits(&h.pool, "nickname_unblock", &name).await, 1);
    let (_, list) = get_json(&h.app, &cookie, &format!("/api/name-blocks?q={name}")).await;
    assert_eq!(list["rows"][0]["lifted_by"], json!(h.wallet), "{list}");
}
