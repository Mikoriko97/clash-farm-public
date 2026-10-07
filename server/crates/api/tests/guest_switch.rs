//! `guests_enabled` (0028) turns new guests away. A file of its own: the switch is global, and cargo runs test
//! binaries one after another, so no guest test of guest_flow.rs runs while it is off. One test, so the two
//! cases never overlap either.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use axum::http::StatusCode;
use common::{app, fresh_ip, guest};

const ON: &str = "INSERT INTO cf.settings (key, value) VALUES ('guests_enabled', 'true'::jsonb)
                  ON CONFLICT (key) DO UPDATE SET value = 'true'::jsonb";

#[tokio::test]
async fn test_guest_switch_off_or_missing_turns_new_guests_away() {
    let (app, pool) = app().await;
    sqlx::query("UPDATE cf.settings SET value = 'false'::jsonb WHERE key = 'guests_enabled'")
        .execute(&pool)
        .await
        .unwrap();
    let (off_status, off_body, _) = guest(&app, &fresh_ip()).await;
    // Fail-closed: a database without 0028's row (or with it deleted by hand) refuses guests too.
    sqlx::query("DELETE FROM cf.settings WHERE key = 'guests_enabled'")
        .execute(&pool)
        .await
        .unwrap();
    let (gone_status, gone_body, _) = guest(&app, &fresh_ip()).await;
    // Restore before asserting: the test database is shared by the whole suite.
    sqlx::query(ON).execute(&pool).await.unwrap();
    assert_eq!(off_status, StatusCode::SERVICE_UNAVAILABLE, "{off_body}");
    assert_eq!(off_body["error"]["code"], "guests_disabled");
    assert_eq!(gone_status, StatusCode::SERVICE_UNAVAILABLE, "{gone_body}");
    assert_eq!(gone_body["error"]["code"], "guests_disabled");
    let (status, body, _) = guest(&app, &fresh_ip()).await;
    assert_eq!(status, StatusCode::OK, "back on: {body}");
}
