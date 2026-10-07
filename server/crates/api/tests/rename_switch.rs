//! `renames_enabled` (0030, ADR-039) pauses paid nickname changes, tickets already bought
//! included. A file of its own: the switch is global, and cargo runs test binaries one after
//! another, so no rename test of rename_flow.rs runs while it is off.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use axum::http::StatusCode;
use common::{app as build, login, post};
use serde_json::json;
use uuid::Uuid;

#[tokio::test]
async fn test_rename_switch_off_pauses_renames_and_keeps_the_ticket() {
    let (app, pool) = build().await;
    // Everything that may fail happens before the switch goes off.
    let s = login(&app).await;
    let name = format!("Sw{}", &Uuid::new_v4().simple().to_string()[..8]);
    let made = post(
        &app,
        &s,
        "/me/character",
        Some(json!({ "nickname": name, "look": { "skin": 1, "shirt": 2, "pants": 3, "shoes": 4 } })),
        None,
    )
    .await;
    assert_eq!(made.0, StatusCode::OK, "{}", made.1);
    sqlx::query("INSERT INTO cf.inventory (user_id, item_id, qty) VALUES ($1::uuid, 'rename_ticket', 1)")
        .bind(&s.user_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE cf.settings SET value = 'false'::jsonb WHERE key = 'renames_enabled'")
        .execute(&pool)
        .await
        .unwrap();
    // A fresh app reads the switch on its first request (the cache lives 5 s per app); the session is shared.
    let (paused, _) = build().await;
    let (status, body) = post(
        &paused,
        &s,
        "/me/nickname",
        Some(json!({ "nickname": format!("{name}x") })),
        None,
    )
    .await;
    let left: Result<i32, _> = sqlx::query_scalar(
        "SELECT qty FROM cf.inventory WHERE user_id = $1::uuid AND item_id = 'rename_ticket'",
    )
    .bind(&s.user_id)
    .fetch_one(&pool)
    .await;
    // Back on before any assert: a failing assert must not leave the switch off for other binaries.
    sqlx::query("UPDATE cf.settings SET value = 'true'::jsonb WHERE key = 'renames_enabled'")
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(
        (status, body["error"]["code"].clone()),
        (StatusCode::CONFLICT, json!("renames_disabled")),
        "{body}"
    );
    assert_eq!(left.unwrap(), 1, "the ticket waits for the switch");
}
