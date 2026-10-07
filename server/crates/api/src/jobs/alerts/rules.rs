//! What the alerts job looks at (ADR-023). Every rule reads state the game already
//! keeps — settings, orders, the reconcile job's audit rows, the jobs' heartbeats in
//! Redis — and never touches anything. Thresholds live in `settings` under `alerts.*`
//! so the operator tunes them from the admin.

use std::sync::OnceLock;

use cf_common::jobs::{every_text, failures_key, heartbeat_key, last_error_key, SCHEDULE};
use redis::aio::ConnectionManager;
use serde_json::{json, Value};
use sqlx::{PgPool, Row};
use time::OffsetDateTime;

use super::{Level, Signal};
use crate::jobs::JobError;

#[derive(Debug, Clone)]
pub struct Thresholds {
    pub review_queue_max: i64,
    pub review_age_max_min: i64,
    pub switch_off_max_min: i64,
    pub signups_per_hour_max: i64,
}

impl Default for Thresholds {
    fn default() -> Self {
        Self {
            review_queue_max: 5,
            review_age_max_min: 60,
            switch_off_max_min: 30,
            signups_per_hour_max: 200,
        }
    }
}

impl Thresholds {
    pub async fn load(pool: &PgPool) -> Result<Self, sqlx::Error> {
        let rows = sqlx::query("SELECT key, value FROM cf.settings WHERE key LIKE 'alerts.%'")
            .fetch_all(pool)
            .await?;
        let mut t = Self::default();
        for row in rows {
            let key: String = row.try_get("key")?;
            let value: Value = row.try_get("value")?;
            let Some(n) = value.as_i64() else { continue };
            match key.as_str() {
                "alerts.review_queue_max" => t.review_queue_max = n,
                "alerts.review_age_max_min" => t.review_age_max_min = n,
                "alerts.switch_off_max_min" => t.switch_off_max_min = n,
                "alerts.signups_per_hour_max" => t.signups_per_hour_max = n,
                _ => {}
            }
        }
        Ok(t)
    }
}

pub async fn evaluate(
    pool: &PgPool,
    redis: &ConnectionManager,
    t: &Thresholds,
) -> Result<Vec<Signal>, JobError> {
    let mut out = Vec::new();
    ledger_mismatch(pool, &mut out).await?;
    review_queue(pool, t, &mut out).await?;
    switches(pool, t, &mut out).await?;
    signups(pool, t, &mut out).await?;
    totp_rejected(pool, &mut out).await?;
    jobs(redis, &mut out).await?;
    Ok(out)
}

fn signal(rule: impl Into<String>, level: Level, title: String, detail: Value) -> Signal {
    Signal {
        rule: rule.into(),
        level,
        title: title.chars().take(200).collect(),
        detail,
    }
}

/// The nightly reconcile reported a difference within the last day and a bit.
async fn ledger_mismatch(pool: &PgPool, out: &mut Vec<Signal>) -> Result<(), sqlx::Error> {
    // Parsed here, not cast in SQL: a changed `after` shape must not fail every tick.
    let after: Option<Value> = sqlx::query_scalar(
        "SELECT after FROM cf.audit_log
         WHERE actor = 'job:reconcile' AND action = 'ledger_mismatch' AND created_at > now() - interval '26 hours'
         ORDER BY created_at DESC LIMIT 1",
    )
    .fetch_optional(pool)
    .await?
    .flatten();
    let count = after.as_ref().and_then(|a| a["count"].as_i64());
    if let Some(n) = count.filter(|n| *n > 0) {
        out.push(signal(
            "ledger_mismatch",
            Level::Crit,
            format!("Ледж ≠ баланси у {n} рахунків — звірка негайно"),
            json!({ "рахунків": n }),
        ));
    }
    Ok(())
}

/// Too many orders wait for a human, or one has waited too long.
async fn review_queue(pool: &PgPool, t: &Thresholds, out: &mut Vec<Signal>) -> Result<(), sqlx::Error> {
    let row = sqlx::query(
        "SELECT count(*) AS n, COALESCE(EXTRACT(EPOCH FROM (now() - min(created_at)))::bigint, 0) AS age
         FROM cf.orders WHERE status = 'needs_review'",
    )
    .fetch_one(pool)
    .await?;
    let n: i64 = row.try_get("n")?;
    let age_min: i64 = row.try_get::<i64, _>("age")? / 60;
    if n > 0 && (n >= t.review_queue_max || age_min >= t.review_age_max_min) {
        let level = if age_min >= 24 * 60 {
            Level::Crit
        } else {
            Level::Warn
        };
        out.push(signal(
            "review_queue",
            level,
            format!("{n} замовлень чекають ручної перевірки, найстаріше {age_min} хв"),
            json!({ "замовлень": n, "найстаріше_хв": age_min }),
        ));
    }
    Ok(())
}

/// Kill switches left flipped, and the oracle's own pause (immediate).
async fn switches(pool: &PgPool, t: &Thresholds, out: &mut Vec<Signal>) -> Result<(), sqlx::Error> {
    let rows = sqlx::query(
        "SELECT key, value, updated_by, (EXTRACT(EPOCH FROM (now() - updated_at)) / 60)::bigint AS age_min
         FROM cf.settings
         WHERE key IN ('maintenance', 'shop_enabled', 'payments_enabled', 'registrations_enabled')",
    )
    .fetch_all(pool)
    .await?;
    for row in rows {
        let key: String = row.try_get("key")?;
        let on = row.try_get::<Value, _>("value")?.as_bool().unwrap_or(false);
        let by: String = row.try_get("updated_by")?;
        let age_min: i64 = row.try_get("age_min")?;
        let detail = json!({ "хв": age_min, "ким": by });
        match key.as_str() {
            "maintenance" if on && age_min >= t.switch_off_max_min => out.push(signal(
                "maintenance_on",
                Level::Crit,
                format!("Maintenance увімкнено {age_min} хв ({by}) — гра закрита"),
                detail,
            )),
            "payments_enabled" if !on && by == "oracle" => out.push(signal(
                "oracle_paused",
                Level::Crit,
                "Оракул зупинив платежі: ціна CLASH застаріла або відхилилась".to_owned(),
                detail,
            )),
            "shop_enabled" | "payments_enabled" | "registrations_enabled"
                if !on && age_min >= t.switch_off_max_min =>
            {
                out.push(signal(
                    format!("switch_off.{key}"),
                    Level::Warn,
                    format!("{key} вимкнено {age_min} хв ({by})"),
                    detail,
                ));
            }
            _ => {}
        }
    }
    Ok(())
}

/// A wave of new accounts (guests included, 0028) looks like bots.
async fn signups(pool: &PgPool, t: &Thresholds, out: &mut Vec<Signal>) -> Result<(), sqlx::Error> {
    let n: i64 =
        sqlx::query_scalar("SELECT count(*) FROM cf.users WHERE created_at > now() - interval '1 hour'")
            .fetch_one(pool)
            .await?;
    if n >= t.signups_per_hour_max {
        out.push(signal(
            "signup_spike",
            Level::Warn,
            format!(
                "{n} нових акаунтів за годину, разом із гостями (поріг {}) — схоже на хвилю ботів",
                t.signups_per_hour_max
            ),
            json!({ "за_годину": n, "поріг": t.signups_per_hour_max }),
        ));
    }
    Ok(())
}

/// Refused second-factor codes (`admin_totp_rejected` in `audit_log`): someone holding an
/// admin's wallet key is guessing.
async fn totp_rejected(pool: &PgPool, out: &mut Vec<Signal>) -> Result<(), sqlx::Error> {
    let rows: Vec<(String, i64)> = sqlx::query_as(
        "SELECT actor, count(*) FROM cf.audit_log
         WHERE action = 'admin_totp_rejected' AND created_at > now() - interval '1 hour'
         GROUP BY actor HAVING count(*) >= 5 ORDER BY count(*) DESC LIMIT 5",
    )
    .fetch_all(pool)
    .await?;
    if rows.is_empty() {
        return Ok(());
    }
    let total: i64 = rows.iter().map(|(_, n)| n).sum();
    let who = rows
        .iter()
        .map(|(actor, n)| format!("{actor} ×{n}"))
        .collect::<Vec<_>>()
        .join(", ");
    out.push(signal(
        "totp_rejected",
        Level::Crit,
        format!("{total} відхилених кодів TOTP за годину — хтось підбирає другий фактор ({who})"),
        json!({ "за_годину": total, "гаманці": who }),
    ));
    Ok(())
}

/// When this process first evaluated: a heartbeat that never appeared is only "stale"
/// once the job had time to run.
static STARTED: OnceLock<i64> = OnceLock::new();

/// Other jobs: failed within a day, or silent for three intervals.
async fn jobs(redis: &ConnectionManager, out: &mut Vec<Signal>) -> Result<(), redis::RedisError> {
    let now = OffsetDateTime::now_utc().unix_timestamp();
    let started = *STARTED.get_or_init(|| now);
    let mut con = redis.clone();
    for (name, every) in SCHEDULE.iter().filter(|(n, _)| *n != "alerts") {
        let (heartbeat, failures, last_error): (Option<i64>, Option<i64>, Option<String>) =
            redis::cmd("MGET")
                .arg(heartbeat_key(name))
                .arg(failures_key(name))
                .arg(last_error_key(name))
                .query_async(&mut con)
                .await?;
        let stale_after = i64::try_from(every * 3 + 60).unwrap_or(i64::MAX);
        let silent_for = heartbeat.map_or(now - started, |hb| now - hb);
        if silent_for > stale_after {
            out.push(signal(
                format!("job_stale.{name}"),
                Level::Crit,
                format!(
                    "Джоба {name} мовчить {} хв (має бігати {})",
                    silent_for / 60,
                    every_text(*every)
                ),
                json!({ "хв": silent_for / 60, "heartbeat": heartbeat.is_some() }),
            ));
        }
        if let Some(n) = failures.filter(|n| *n > 0) {
            let error = last_error.unwrap_or_default();
            out.push(signal(
                format!("job_failed.{name}"),
                if n >= 10 { Level::Crit } else { Level::Warn },
                format!("Джоба {name}: {n} збоїв за добу — {error}"),
                json!({ "збоїв": n, "остання_помилка": error }),
            ));
        }
    }
    Ok(())
}
