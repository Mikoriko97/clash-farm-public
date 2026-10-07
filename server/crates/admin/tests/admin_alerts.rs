//! Alerts in the admin (ADR-023): listing what the job opened, acknowledging (audited,
//! once), muting a rule for a bounded while (audited), unmuting, the dashboard's own
//! "alerts job silent" signal, and the open-alert count next to the menu.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use axum::http::StatusCode;
use common::*;
use serde_json::{json, Value};
use uuid::Uuid;

async fn audit_rows(pool: &sqlx::PgPool, action: &str, target: &str) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM cf.audit_log WHERE action = $1 AND target = $2")
        .bind(action)
        .bind(target)
        .fetch_one(pool)
        .await
        .unwrap()
}

#[tokio::test]
async fn test_alerts_are_listed_acked_muted_and_audited() {
    let h = harness().await;
    let (cookie, csrf) = login(&h, &h.key, &h.wallet).await.unwrap();
    let rule = format!("job_failed.t{}", &Uuid::new_v4().simple().to_string()[..8]);
    let id: i64 = sqlx::query_scalar(
        "INSERT INTO cf.alerts (rule, level, title, detail) VALUES ($1, 'crit', 'test alert', '{\"n\": 1}') RETURNING id",
    )
    .bind(&rule)
    .fetch_one(&h.pool)
    .await
    .unwrap();

    let (status, me) = get_json(&h.app, &cookie, "/api/me").await;
    assert_eq!(status, StatusCode::OK);
    assert!(me["open_alerts"].as_i64().unwrap() >= 1, "{me}");
    let (status, list) = get_json(&h.app, &cookie, "/api/alerts").await;
    assert_eq!(status, StatusCode::OK, "{list}");
    let row = list["rows"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["id"] == id)
        .expect("our row is open")
        .clone();
    assert_eq!(row["rule"], rule);
    assert!(row["acked_by"].is_null() && row["notified_at"].is_null());
    assert!(list["total"].as_i64().unwrap() >= 1);
    assert!(list["rules"]
        .as_array()
        .unwrap()
        .iter()
        .any(|r| r == "review_queue"));
    let (status, _) = get_json(&h.app, &cookie, "/api/alerts?state=nope").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    // The dashboard shows the job's silence (no heartbeat in the test Redis) as its own crit.
    let (_, dash) = get_json(&h.app, &cookie, "/api/dashboard").await;
    let alerts = dash["alerts"].as_array().unwrap();
    assert!(
        alerts
            .iter()
            .any(|a| a["rule"] == "job_stale.alerts" && a["level"] == "crit"),
        "{dash}"
    );
    assert!(
        alerts.iter().any(|a| a["id"] == id && a["text"] == "test alert"),
        "{dash}"
    );

    // Acknowledge once. Audit rows are counted relative to now: `audit_log` is append-only
    // and alert ids restart whenever the test database is rebuilt.
    let ack = format!("/api/alerts/{id}/ack");
    let acks_before = audit_rows(&h.pool, "alert_ack", &id.to_string()).await;
    let (status, body) = post_json(&h.app, &cookie, &csrf, &ack, json!({ "note": "  looking  " })).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (by, note): (Option<String>, Option<String>) =
        sqlx::query_as("SELECT acked_by, ack_note FROM cf.alerts WHERE id = $1")
            .bind(id)
            .fetch_one(&h.pool)
            .await
            .unwrap();
    assert_eq!(by.as_deref(), Some(h.wallet.as_str()));
    assert_eq!(note.as_deref(), Some("looking"));
    assert_eq!(
        audit_rows(&h.pool, "alert_ack", &id.to_string()).await,
        acks_before + 1
    );
    let (status, _) = post_json(&h.app, &cookie, &csrf, &ack, json!({})).await;
    assert_eq!(status, StatusCode::BAD_REQUEST, "already acknowledged");
    let (status, _) = post_json(&h.app, &cookie, &csrf, "/api/alerts/999999999/ack", json!({})).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = post_json(&h.app, &cookie, "bad", &ack, json!({})).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "csrf");

    mute_flow(&h, &cookie, &csrf, &rule).await;

    // Resolved rows leave the open list and show under state=resolved.
    sqlx::query("UPDATE cf.alerts SET resolved_at = now() WHERE id = $1")
        .bind(id)
        .execute(&h.pool)
        .await
        .unwrap();
    let (_, open) = get_json(&h.app, &cookie, "/api/alerts").await;
    assert!(!open["rows"].as_array().unwrap().iter().any(|r| r["id"] == id));
    let (_, resolved) = get_json(&h.app, &cookie, "/api/alerts?state=resolved").await;
    assert!(resolved["rows"].as_array().unwrap().iter().any(|r| r["id"] == id));
    sqlx::query("DELETE FROM cf.alerts WHERE id = $1")
        .bind(id)
        .execute(&h.pool)
        .await
        .unwrap();
}

async fn mute_flow(h: &Harness, cookie: &str, csrf: &str, rule: &str) {
    let mute = |body: Value| post_json(&h.app, cookie, csrf, "/api/alerts/mute", body);
    let (status, body) = mute(json!({ "rule": rule, "hours": 2, "note": "night" })).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body["until"].as_i64().unwrap() > time::OffsetDateTime::now_utc().unix_timestamp() + 7000);
    let (_, list) = get_json(&h.app, cookie, "/api/alerts").await;
    let muted = list["mutes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|m| m["rule"] == rule)
        .expect("muted");
    assert_eq!(muted["muted_by"], h.wallet);
    assert_eq!(muted["note"], "night");
    assert_eq!(audit_rows(&h.pool, "alert_mute", rule).await, 1);
    for bad in [
        json!({ "rule": rule, "hours": 0 }),
        json!({ "rule": rule, "hours": 200 }),
        json!({ "rule": "evil rule", "hours": 1 }),
        json!({ "rule": "unknown_rule", "hours": 1 }),
        json!({ "rule": "job_stale.", "hours": 1 }),
    ] {
        assert_eq!(mute(bad.clone()).await.0, StatusCode::BAD_REQUEST, "{bad}");
    }
    let (status, _) = post_json(
        &h.app,
        cookie,
        csrf,
        "/api/alerts/unmute",
        json!({ "rule": rule }),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(audit_rows(&h.pool, "alert_unmute", rule).await, 1);
    let (status, _) = post_json(
        &h.app,
        cookie,
        csrf,
        "/api/alerts/unmute",
        json!({ "rule": rule }),
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND, "nothing to unmute");
}
