//! The alerts job (ADR-023) against local Postgres + Memurai: rules open rows in
//! `cf.alerts`, the notifier hears about each once, resolution is reported, mutes and
//! acknowledgements keep the channel quiet, and "no channel" claims no delivery.
//! One test function: the job is global, so sections must not interleave.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::sync::Mutex;

use cf_api::jobs::alerts::telegram::NoChannel;
use cf_api::jobs::alerts::{self, Notifier};
use cf_common::jobs::{failures_key, heartbeat_key, last_error_key};
use common::{app, login};
use redis::aio::ConnectionManager;
use serde_json::{json, Value};
use sqlx::PgPool;
use uuid::Uuid;

#[derive(Default)]
struct Recorder(Mutex<Vec<String>>);

impl Notifier for Recorder {
    async fn send(&self, text: &str) -> Result<bool, String> {
        self.0.lock().unwrap().push(text.to_owned());
        Ok(true)
    }
}

impl Recorder {
    fn take(&self) -> Vec<String> {
        std::mem::take(&mut *self.0.lock().unwrap())
    }
}

const RULES: [&str; 8] = [
    "maintenance_on",
    "job_failed.retention",
    "job_stale.expire_quotes",
    "review_queue",
    "job_failed.finalize_burns",
    "oracle_paused",
    "signup_spike",
    "totp_rejected",
];
/// Rules the first section opens (all but the one the mute section uses).
const OPENED: [&str; 7] = [
    "maintenance_on",
    "job_failed.retention",
    "job_stale.expire_quotes",
    "review_queue",
    "oracle_paused",
    "signup_spike",
    "totp_rejected",
];
/// Rules whose cause the second section removes (`totp_rejected` sits in append-only audit rows).
const RESOLVED: [&str; 5] = [
    "maintenance_on",
    "job_failed.retention",
    "job_stale.expire_quotes",
    "oracle_paused",
    "signup_spike",
];

async fn redis() -> ConnectionManager {
    let url = std::env::var("CF_TEST_REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379/1".to_owned());
    redis::Client::open(url)
        .unwrap()
        .get_connection_manager()
        .await
        .unwrap()
}

async fn set(redis: &mut ConnectionManager, key: String, value: impl redis::ToRedisArgs) {
    let _: () = redis::cmd("SET")
        .arg(key)
        .arg(value)
        .query_async(redis)
        .await
        .unwrap();
}

async fn del(redis: &mut ConnectionManager, key: String) {
    let _: () = redis::cmd("DEL").arg(key).query_async(redis).await.unwrap();
}

async fn setting(pool: &PgPool, key: &str, value: Value) {
    sqlx::query(
        "INSERT INTO cf.settings (key, value, updated_by) VALUES ($1, $2, 'sol:tester')
         ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value, updated_at = now(), updated_by = 'sol:tester'",
    )
    .bind(key)
    .bind(value)
    .execute(pool)
    .await
    .unwrap();
}

async fn open_rules(pool: &PgPool) -> Vec<String> {
    sqlx::query_scalar("SELECT rule FROM cf.alerts WHERE resolved_at IS NULL ORDER BY rule")
        .fetch_all(pool)
        .await
        .unwrap()
}

async fn notified(pool: &PgPool, rule: &str) -> bool {
    sqlx::query_scalar(
        "SELECT notified_at IS NOT NULL FROM cf.alerts WHERE rule = $1 AND resolved_at IS NULL",
    )
    .bind(rule)
    .fetch_one(pool)
    .await
    .unwrap()
}

/// Rows of our rules from earlier runs must not leak in (the table survives between runs).
async fn clean(pool: &PgPool, redis: &mut ConnectionManager) {
    sqlx::query("DELETE FROM cf.alerts WHERE rule = ANY($1)")
        .bind(RULES.map(str::to_owned).to_vec())
        .execute(pool)
        .await
        .unwrap();
    sqlx::query("DELETE FROM cf.alert_mutes WHERE rule = ANY($1)")
        .bind(RULES.map(str::to_owned).to_vec())
        .execute(pool)
        .await
        .unwrap();
    for name in ["retention", "finalize_burns"] {
        del(redis, failures_key(name)).await;
        del(redis, last_error_key(name)).await;
    }
    del(redis, heartbeat_key("expire_quotes")).await;
    setting(pool, "maintenance", json!(false)).await;
    setting(pool, "payments_enabled", json!(true)).await;
    setting(pool, "alerts.signups_per_hour_max", json!(200)).await;
}

async fn parked_order(pool: &PgPool, user_id: Uuid) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO cf.orders (user_id, item_id, qty, price_usd_cents, clash_per_usd, amount_clash, burn_amount,
                                transfer_amount, quote_expires_at, status, created_at)
         VALUES ($1, 'decor_lantern', 1, 90, 1000, 90000, 63000, 27000, now(), 'needs_review', now() - interval '2 hours')
         RETURNING id",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await
    .unwrap()
}

#[tokio::test]
async fn test_alerts_open_notify_once_resolve_mute_ack_and_no_channel() {
    let (app, pool) = app().await;
    let mut redis = redis().await;
    clean(&pool, &mut redis).await;
    let session = login(&app).await;
    let order = parked_order(&pool, Uuid::parse_str(&session.user_id).unwrap()).await;
    let rec = Recorder::default();

    opens_and_notifies_once(&pool, &mut redis, &rec).await;
    resolves_and_reports(&pool, &mut redis, &rec, order).await;
    mute_and_no_channel(&pool, &mut redis, &rec).await;
    ack_and_reminder(&pool, &mut redis, &rec).await;
    clean(&pool, &mut redis).await;
}

async fn opens_and_notifies_once(pool: &PgPool, redis: &mut ConnectionManager, rec: &Recorder) {
    setting(pool, "maintenance", json!(true)).await;
    sqlx::query("UPDATE cf.settings SET updated_at = now() - interval '2 hours' WHERE key = 'maintenance'")
        .execute(pool)
        .await
        .unwrap();
    set(redis, failures_key("retention"), 3).await;
    set(redis, last_error_key("retention"), "database error").await;
    set(
        redis,
        heartbeat_key("expire_quotes"),
        time::OffsetDateTime::now_utc().unix_timestamp() - 3600,
    )
    .await;
    // The oracle's own pause fires at once; this test's login is the one signup that
    // trips a threshold of 1; five refused codes in an hour mean guessing.
    setting(pool, "payments_enabled", json!(false)).await;
    sqlx::query("UPDATE cf.settings SET updated_by = 'oracle' WHERE key = 'payments_enabled'")
        .execute(pool)
        .await
        .unwrap();
    setting(pool, "alerts.signups_per_hour_max", json!(1)).await;
    for _ in 0..5 {
        sqlx::query("INSERT INTO cf.audit_log (actor, action, after) VALUES ('sol:tester_totp', 'admin_totp_rejected', '{\"reason\": \"code\"}')")
            .execute(pool)
            .await
            .unwrap();
    }
    let summary = alerts::run_with(pool, redis, rec, "test").await.unwrap();
    let sent = rec.take();
    alerts::run_with(pool, redis, rec, "test").await.unwrap();
    let repeat = rec.take();
    // Switches go back before any assertion: a failed check must not leave the shared
    // test database in maintenance for every test binary that follows.
    setting(pool, "maintenance", json!(false)).await;
    setting(pool, "payments_enabled", json!(true)).await;
    setting(pool, "alerts.signups_per_hour_max", json!(200)).await;

    let open = open_rules(pool).await;
    for rule in &OPENED {
        assert!(
            open.contains(&(*rule).to_owned()),
            "{rule} missing: {open:?} ({summary})"
        );
    }
    assert!(
        sent.iter()
            .any(|m| m.starts_with("🔴") && m.contains("Maintenance увімкнено")),
        "{sent:?}"
    );
    assert!(
        sent.iter()
            .any(|m| m.contains("retention") && m.contains("database error")),
        "{sent:?}"
    );
    assert!(
        sent.iter().any(|m| m.contains("expire_quotes мовчить")),
        "{sent:?}"
    );
    assert!(sent.iter().any(|m| m.contains("ручної перевірки")), "{sent:?}");
    assert!(
        sent.iter()
            .any(|m| m.starts_with("🔴") && m.contains("Оракул зупинив платежі")),
        "{sent:?}"
    );
    assert!(
        sent.iter().any(|m| m.contains("нових акаунтів за годину")),
        "{sent:?}"
    );
    // Audit rows are append-only, so earlier runs add to the count: at least this run's five.
    assert!(
        sent.iter().any(|m| m.starts_with("🔴")
            && m.contains("відхилених кодів TOTP")
            && m.contains("sol:tester_totp ×")),
        "{sent:?}"
    );
    assert!(
        sent.iter().all(|m| m.ends_with("<i>Clash Farm · test</i>")),
        "{sent:?}"
    );
    assert!(repeat.is_empty(), "nothing repeats while a signal stays open");
    for rule in &OPENED {
        assert!(notified(pool, rule).await, "{rule} marked as delivered");
    }
}

async fn resolves_and_reports(pool: &PgPool, redis: &mut ConnectionManager, rec: &Recorder, order: Uuid) {
    setting(pool, "maintenance", json!(false)).await;
    setting(pool, "payments_enabled", json!(true)).await;
    // Out of reach for the shared test database: every suite run (and each guest test) adds users within the
    // hour, so the default 200 would keep the spike open after a few runs. `reset` puts 200 back.
    setting(pool, "alerts.signups_per_hour_max", json!(1_000_000)).await;
    del(redis, failures_key("retention")).await;
    del(redis, last_error_key("retention")).await;
    set(
        redis,
        heartbeat_key("expire_quotes"),
        time::OffsetDateTime::now_utc().unix_timestamp(),
    )
    .await;
    sqlx::query("UPDATE cf.orders SET status = 'rejected' WHERE id = $1")
        .bind(order)
        .execute(pool)
        .await
        .unwrap();
    alerts::run_with(pool, redis, rec, "test").await.unwrap();
    let open = open_rules(pool).await;
    for rule in &RESOLVED {
        assert!(!open.contains(&(*rule).to_owned()), "{rule} still open: {open:?}");
    }
    let sent = rec.take();
    assert!(
        sent.iter().filter(|m| m.starts_with("✅ Вирішено")).count() >= 5,
        "{sent:?}"
    );
    let resolved: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM cf.alerts WHERE rule = 'maintenance_on' AND resolved_at IS NOT NULL AND notified_at IS NOT NULL",
    )
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(resolved, 1);
}

async fn mute_and_no_channel(pool: &PgPool, redis: &mut ConnectionManager, rec: &Recorder) {
    sqlx::query("INSERT INTO cf.alert_mutes (rule, until, muted_by) VALUES ('job_failed.finalize_burns', now() + interval '1 hour', 'sol:tester')")
        .execute(pool)
        .await
        .unwrap();
    set(redis, failures_key("finalize_burns"), 1).await;
    set(redis, last_error_key("finalize_burns"), "rpc timeout").await;
    alerts::run_with(pool, redis, rec, "test").await.unwrap();
    assert!(open_rules(pool)
        .await
        .contains(&"job_failed.finalize_burns".to_owned()));
    assert!(
        !rec.take().iter().any(|m| m.contains("finalize_burns")),
        "muted: opened but not sent"
    );
    assert!(!notified(pool, "job_failed.finalize_burns").await);
    // No channel: the row opens, nothing is claimed delivered.
    alerts::run_with(pool, redis, &NoChannel, "test").await.unwrap();
    assert!(!notified(pool, "job_failed.finalize_burns").await);
    // Mute lifted → the next tick delivers.
    sqlx::query("DELETE FROM cf.alert_mutes WHERE rule = 'job_failed.finalize_burns'")
        .execute(pool)
        .await
        .unwrap();
    alerts::run_with(pool, redis, rec, "test").await.unwrap();
    assert!(rec
        .take()
        .iter()
        .any(|m| m.contains("finalize_burns") && m.contains("rpc timeout")));
    assert!(notified(pool, "job_failed.finalize_burns").await);
}

async fn ack_and_reminder(pool: &PgPool, redis: &mut ConnectionManager, rec: &Recorder) {
    // Ten failures make it critical; seven hours since delivery would earn a reminder —
    // unless someone acknowledged it.
    set(redis, failures_key("finalize_burns"), 10).await;
    sqlx::query("UPDATE cf.alerts SET opened_at = now() - interval '7 hours', notified_at = now() - interval '7 hours', acked_by = 'sol:tester', acked_at = now() WHERE rule = 'job_failed.finalize_burns' AND resolved_at IS NULL")
        .execute(pool)
        .await
        .unwrap();
    alerts::run_with(pool, redis, rec, "test").await.unwrap();
    assert!(rec.take().is_empty(), "acknowledged: no reminder");
    sqlx::query("UPDATE cf.alerts SET acked_by = NULL, acked_at = NULL WHERE rule = 'job_failed.finalize_burns' AND resolved_at IS NULL")
        .execute(pool)
        .await
        .unwrap();
    alerts::run_with(pool, redis, rec, "test").await.unwrap();
    let sent = rec.take();
    assert!(
        sent.iter()
            .any(|m| m.starts_with("🔴 Досі відкрито (7 год)") && m.contains("10 збоїв")),
        "{sent:?}"
    );
    alerts::run_with(pool, redis, rec, "test").await.unwrap();
    assert!(rec.take().is_empty(), "reminded once per six hours");
}
