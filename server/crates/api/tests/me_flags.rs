//! `/api/v1/me` never shows the admin's marks on a player (`users.flags` from the admin
//! panel, ADR-022): a flagged player must not learn it, and the note may name the admin.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use axum::http::StatusCode;
use common::{app, get, login};
use uuid::Uuid;

#[tokio::test]
async fn test_me_hides_admin_marks_in_flags() {
    let (app, pool) = app().await;
    let session = login(&app).await;
    sqlx::query("UPDATE cf.users SET flags = flags || $2 WHERE id = $1")
        .bind(Uuid::parse_str(&session.user_id).unwrap())
        .bind(serde_json::json!({ "suspect": true, "note": "farm", "by": "sol:AdminWallet", "at": 1 }))
        .execute(&pool)
        .await
        .unwrap();
    let (status, body) = get(&app, &session, "/me").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let flags = body["user"]["flags"].as_object().unwrap();
    for key in ["suspect", "note", "by", "at"] {
        assert!(!flags.contains_key(key), "{key} leaked: {body}");
    }
    assert!(!body.to_string().contains("AdminWallet"), "{body}");
}
