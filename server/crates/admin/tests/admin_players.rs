//! Players: search, flags and bans (sessions revoked, everything audited).
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use axum::http::StatusCode;
use cf_auth::session::ClientFingerprint;
use common::*;
use serde_json::{json, Value};
use uuid::Uuid;

#[tokio::test]
async fn test_players_are_searchable_and_flags_are_audited() {
    let h = harness().await;
    let (cookie, csrf) = login(&h, &h.key, &h.wallet).await.unwrap();
    let (user_id, _) = parked_order(&h.pool, None).await;
    let wallet: String = sqlx::query_scalar("SELECT wallet FROM cf.users WHERE id = $1")
        .bind(user_id)
        .fetch_one(&h.pool)
        .await
        .unwrap();
    let needle = &wallet[4..12];
    let (status, body) = get_json(
        &h.app,
        &cookie,
        &format!("/api/players?q={needle}&sort=created&page=1&per_page=25"),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["total"], 1);
    assert_eq!(body["rows"][0]["id"], user_id.to_string());
    let (status, body) = get_json(&h.app, &cookie, "/api/players?q=%25%25%25").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["total"], 0, "wildcards are literal: {body}");
    let (status, body) = get_json(&h.app, &cookie, &format!("/api/players/{user_id}")).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["user"]["wallet"], wallet);
    assert_eq!(body["orders"].as_array().unwrap().len(), 1);
    assert!(
        body["island"].is_null(),
        "no island until the player opens the game"
    );
    // Earlier names (paid rename, 0030): none yet; a history row shows up in the card.
    assert_eq!(body["names"], json!([]));
    sqlx::query("INSERT INTO cf.nickname_history (user_id, old_nickname, old_key, new_nickname, new_key) VALUES ($1, 'Before', 'before', 'After', 'after')")
        .bind(user_id)
        .execute(&h.pool)
        .await
        .unwrap();
    let (_, body) = get_json(&h.app, &cookie, &format!("/api/players/{user_id}")).await;
    assert_eq!(body["names"][0]["old_nickname"], "Before");
    assert_eq!(body["names"][0]["new_nickname"], "After");
    // A complaint about the old name still finds the player.
    let (_, found) = get_json(
        &h.app,
        &cookie,
        "/api/players?q=Before&sort=created&page=1&per_page=25",
    )
    .await;
    assert!(
        found["rows"]
            .as_array()
            .unwrap()
            .iter()
            .any(|r| r["id"] == user_id.to_string()),
        "{found}"
    );
    let (status, _) = get_json(&h.app, &cookie, &format!("/api/players/{}", Uuid::new_v4())).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    // Flagging needs CSRF, merges into flags and lands in the audit log.
    let (status, _) = post_json(
        &h.app,
        &cookie,
        "bad",
        &format!("/api/players/{user_id}/flag"),
        json!({ "suspect": true, "note": "farm" }),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, body) = post_json(
        &h.app,
        &cookie,
        &csrf,
        &format!("/api/players/{user_id}/flag"),
        json!({ "suspect": true, "note": "farm" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["flags"]["suspect"], true);
    assert_eq!(body["flags"]["by"], h.wallet);
    assert_eq!(audit_count(&h.pool, "player_flag", user_id).await, 1);
    let (_, body) = get_json(&h.app, &cookie, "/api/audit?q=player_flag").await;
    assert!(
        body.as_array()
            .unwrap()
            .iter()
            .any(|r| r["target"] == user_id.to_string()),
        "{body}"
    );
    let (_, body) = get_json(&h.app, &cookie, "/api/audit?q=%25%25%25").await;
    assert!(
        body.as_array().unwrap().is_empty(),
        "wildcards must not match everything: {body}"
    );
    let (status, body) = get_json(&h.app, &cookie, "/api/system").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body["counts"]["users"].as_i64().unwrap() >= 1);
}

#[tokio::test]
async fn test_ban_revokes_sessions_and_is_audited() {
    let h = harness().await;
    let (cookie, csrf) = login(&h, &h.key, &h.wallet).await.unwrap();
    let (user_id, _) = parked_order(&h.pool, None).await;
    // A live session of the player, exactly as cf-api issues it (refresh row + access token).
    let sessions = player_sessions(&h).await;
    let fp = ClientFingerprint {
        ip_hash: None,
        ua_hash: None,
        ua_hash_prev: None,
    };
    let issued = sessions.login(user_id, &fp).await.unwrap();
    assert!(sessions
        .authenticate(&issued.access_token)
        .await
        .unwrap()
        .is_some());
    let (status, body) = post_json(
        &h.app,
        &cookie,
        &csrf,
        &format!("/api/players/{user_id}/ban"),
        json!({ "reason": " " }),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "a reason is required: {body}");
    let (status, body) = post_json(
        &h.app,
        &cookie,
        &csrf,
        &format!("/api/players/{user_id}/ban"),
        json!({ "reason": "bot farm" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["sessions_revoked"], 1);
    let (banned, reason): (bool, Option<String>) =
        sqlx::query_as("SELECT banned_at IS NOT NULL, ban_reason FROM cf.users WHERE id = $1")
            .bind(user_id)
            .fetch_one(&h.pool)
            .await
            .unwrap();
    assert!(banned);
    assert_eq!(reason.as_deref(), Some("bot farm"));
    let live: i64 =
        sqlx::query_scalar("SELECT count(*) FROM cf.sessions WHERE user_id = $1 AND revoked_at IS NULL")
            .bind(user_id)
            .fetch_one(&h.pool)
            .await
            .unwrap();
    assert_eq!(live, 0, "every session revoked");
    assert!(
        sessions
            .authenticate(&issued.access_token)
            .await
            .unwrap()
            .is_none(),
        "the access token dies with the ban, not at its own expiry"
    );
    assert!(
        sessions.refresh(&issued.refresh_token, &fp).await.is_err(),
        "the refresh token is dead too"
    );
    assert_eq!(audit_count(&h.pool, "player_ban", user_id).await, 1);
    // A repeat (retry after a Redis hiccup) is idempotent: first reason kept, no second
    // audit row, sessions revoked again. Then the list shows the ban and unban restores.
    let (status, body) = post_json(
        &h.app,
        &cookie,
        &csrf,
        &format!("/api/players/{user_id}/ban"),
        json!({ "reason": "again" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["sessions_revoked"], 0);
    let reason: Option<String> = sqlx::query_scalar("SELECT ban_reason FROM cf.users WHERE id = $1")
        .bind(user_id)
        .fetch_one(&h.pool)
        .await
        .unwrap();
    assert_eq!(reason.as_deref(), Some("bot farm"), "the first reason stays");
    assert_eq!(audit_count(&h.pool, "player_ban", user_id).await, 1);
    let (_, list) = get_json(&h.app, &cookie, "/api/players?sort=banned").await;
    assert_eq!(list["rows"][0]["id"], user_id.to_string(), "{list}");
    assert!(list["rows"][0]["banned_at"].is_string());
    let (status, body) = post_json(
        &h.app,
        &cookie,
        &csrf,
        &format!("/api/players/{user_id}/unban"),
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(audit_count(&h.pool, "player_unban", user_id).await, 1);
    let (status, _) = post_json(
        &h.app,
        &cookie,
        &csrf,
        &format!("/api/players/{user_id}/unban"),
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "nothing to unban");
}
