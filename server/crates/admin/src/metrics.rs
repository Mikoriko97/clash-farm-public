//! Read-only numbers for the dashboard (ADR-022): players and activity, retention
//! cohorts, payments and burns, credits minted vs spent (ROR), a 14-day series and
//! the alerts an operator must see. Counts and atomic amounts stay integers; the DB
//! clock (`now()` in SQL) is the reference, never the process clock.

use cf_common::jobs::heartbeat_key;
use redis::aio::ConnectionManager;
use serde::Serialize;
use sqlx::postgres::PgRow;
use sqlx::{PgPool, Row};
use time::Date;

#[derive(Debug, Clone, Default, Serialize)]
pub struct Players {
    pub total: i64,
    pub new_24h: i64,
    pub new_7d: i64,
    pub dau: i64,
    pub wau: i64,
    pub mau: i64,
}

/// One retention figure: of `cohort` players who signed up in the window, `retained`
/// did anything on day N after signing up.
#[derive(Debug, Clone, Default, Serialize)]
pub struct Cohort {
    pub cohort: i64,
    pub retained: i64,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Retention {
    pub d1: Cohort,
    pub d7: Cohort,
    pub d30: Cohort,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Payments {
    pub orders_today: i64,
    pub review_queue: i64,
    pub submitted: i64,
    pub fulfilled_7d: i64,
    pub rejected_7d: i64,
    pub payers_30d: i64,
    pub usd_cents_30d: i64,
    /// Atomic CLASH units of verified burns.
    pub burned_total: i64,
    pub burned_24h: i64,
    pub burned_30d: i64,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Credits {
    pub minted_30d: i64,
    pub spent_30d: i64,
    /// spent / minted in basis points (10 000 = 1.0); 0 when nothing was minted.
    pub ror_bp: i64,
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Day {
    /// `YYYY-MM-DD` (the client only labels axes with it).
    pub date: Option<String>,
    pub orders: i64,
    pub fulfilled: i64,
    pub clash: i64,
    pub new_users: i64,
    pub active: i64,
    pub credits_minted: i64,
}

/// A signal on the dashboard: a row the alerts job opened (`id`), or one this page view
/// computed itself (`id` = None) — kill switches, the review queue, the job's own silence.
#[derive(Debug, Clone, Serialize)]
pub struct Alert {
    pub id: Option<i64>,
    pub rule: String,
    /// `warn` | `crit`
    pub level: String,
    pub text: String,
    pub acked: bool,
}

impl Alert {
    fn instant(rule: impl Into<String>, level: &str, text: String) -> Self {
        Self {
            id: None,
            rule: rule.into(),
            level: level.to_owned(),
            text,
            acked: false,
        }
    }
}

#[derive(Debug, Clone, Default, Serialize)]
pub struct Dashboard {
    pub players: Players,
    pub retention: Retention,
    pub payments: Payments,
    pub credits: Credits,
    pub days: Vec<Day>,
    pub alerts: Vec<Alert>,
    pub ledger_mismatches: i64,
}

pub async fn review_count(pool: &PgPool) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar("SELECT count(*) FROM cf.orders WHERE status = 'needs_review'")
        .fetch_one(pool)
        .await
}

pub async fn load(pool: &PgPool, redis: &ConnectionManager) -> Result<Dashboard, sqlx::Error> {
    let players = players(pool).await?;
    let retention = retention(pool).await?;
    let payments = payments(pool).await?;
    let credits = credits(pool).await?;
    let days = last_days(pool).await?;
    let ledger_mismatches = ledger_mismatches(pool).await?;
    let mut d = Dashboard {
        players,
        retention,
        payments,
        credits,
        days,
        alerts: Vec::new(),
        ledger_mismatches,
    };
    d.alerts = alerts(pool, redis, &d).await?;
    Ok(d)
}

async fn players(pool: &PgPool) -> Result<Players, sqlx::Error> {
    let r = sqlx::query(
        "SELECT count(*) AS total,
                count(*) FILTER (WHERE created_at > now() - interval '1 day') AS new_24h,
                count(*) FILTER (WHERE created_at > now() - interval '7 days') AS new_7d,
                count(*) FILTER (WHERE last_seen_at > now() - interval '1 day') AS dau,
                count(*) FILTER (WHERE last_seen_at > now() - interval '7 days') AS wau,
                count(*) FILTER (WHERE last_seen_at > now() - interval '30 days') AS mau
         FROM cf.users",
    )
    .fetch_one(pool)
    .await?;
    Ok(Players {
        total: r.try_get("total")?,
        new_24h: r.try_get("new_24h")?,
        new_7d: r.try_get("new_7d")?,
        dau: r.try_get("dau")?,
        wau: r.try_get("wau")?,
        mau: r.try_get("mau")?,
    })
}

/// Day-N retention over the last 14 sign-up days that are old enough to have a day N.
async fn cohort(pool: &PgPool, day: i32) -> Result<Cohort, sqlx::Error> {
    let r = sqlx::query(
        "SELECT count(*) AS cohort,
                count(*) FILTER (WHERE EXISTS (
                    SELECT 1 FROM cf.events e WHERE e.user_id = u.id
                      AND e.created_at >= u.created_at + make_interval(days => $1)
                      AND e.created_at <  u.created_at + make_interval(days => $1 + 1))) AS retained
         FROM cf.users u
         WHERE u.created_at < now() - make_interval(days => $1 + 1)
           AND u.created_at > now() - make_interval(days => $1 + 15)",
    )
    .bind(day)
    .fetch_one(pool)
    .await?;
    Ok(Cohort {
        cohort: r.try_get("cohort")?,
        retained: r.try_get("retained")?,
    })
}

async fn retention(pool: &PgPool) -> Result<Retention, sqlx::Error> {
    Ok(Retention {
        d1: cohort(pool, 1).await?,
        d7: cohort(pool, 7).await?,
        d30: cohort(pool, 30).await?,
    })
}

async fn payments(pool: &PgPool) -> Result<Payments, sqlx::Error> {
    let o = sqlx::query(
        "SELECT count(*) FILTER (WHERE created_at >= date_trunc('day', now())) AS today,
                count(*) FILTER (WHERE status = 'needs_review') AS review,
                count(*) FILTER (WHERE status = 'submitted') AS submitted,
                count(*) FILTER (WHERE status = 'fulfilled' AND updated_at > now() - interval '7 days') AS fulfilled_7d,
                count(*) FILTER (WHERE status = 'rejected' AND updated_at > now() - interval '7 days') AS rejected_7d,
                count(DISTINCT user_id) FILTER (WHERE status = 'fulfilled' AND updated_at > now() - interval '30 days') AS payers_30d,
                coalesce(sum(price_usd_cents::bigint * qty) FILTER (WHERE status = 'fulfilled' AND updated_at > now() - interval '30 days'), 0)::bigint AS usd_30d
         FROM cf.orders",
    )
    .fetch_one(pool)
    .await?;
    let b = sqlx::query(
        "SELECT coalesce(sum(amount), 0)::bigint AS total,
                coalesce(sum(amount) FILTER (WHERE verified_at > now() - interval '1 day'), 0)::bigint AS d1,
                coalesce(sum(amount) FILTER (WHERE verified_at > now() - interval '30 days'), 0)::bigint AS d30
         FROM cf.burns",
    )
    .fetch_one(pool)
    .await?;
    Ok(Payments {
        orders_today: o.try_get("today")?,
        review_queue: o.try_get("review")?,
        submitted: o.try_get("submitted")?,
        fulfilled_7d: o.try_get("fulfilled_7d")?,
        rejected_7d: o.try_get("rejected_7d")?,
        payers_30d: o.try_get("payers_30d")?,
        usd_cents_30d: o.try_get("usd_30d")?,
        burned_total: b.try_get("total")?,
        burned_24h: b.try_get("d1")?,
        burned_30d: b.try_get("d30")?,
    })
}

/// Minted = what `system:mint` gave away (negative deltas), spent = what `system:sink` took.
async fn credits(pool: &PgPool) -> Result<Credits, sqlx::Error> {
    let r = sqlx::query(
        "SELECT coalesce(-sum(delta) FILTER (WHERE account = 'system:mint'), 0)::bigint AS minted,
                coalesce(sum(delta) FILTER (WHERE account = 'system:sink'), 0)::bigint AS spent
         FROM cf.ledger WHERE currency = 'CREDITS' AND user_id IS NULL AND created_at > now() - interval '30 days'",
    )
    .fetch_one(pool)
    .await?;
    let minted: i64 = r.try_get("minted")?;
    let spent: i64 = r.try_get("spent")?;
    let ror_bp = if minted > 0 {
        spent.saturating_mul(10_000) / minted
    } else {
        0
    };
    Ok(Credits {
        minted_30d: minted,
        spent_30d: spent,
        ror_bp,
    })
}

fn day_row(r: &PgRow) -> Result<Day, sqlx::Error> {
    Ok(Day {
        date: r.try_get::<Option<Date>, _>("day")?.map(|d| d.to_string()),
        orders: r.try_get("orders")?,
        fulfilled: r.try_get("fulfilled")?,
        clash: r.try_get("clash")?,
        new_users: r.try_get("new_users")?,
        active: r.try_get("active")?,
        credits_minted: r.try_get("credits_minted")?,
    })
}

/// Fourteen calendar days ending today (DB clock), oldest first, zeros for empty days.
async fn last_days(pool: &PgPool) -> Result<Vec<Day>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT d.day::date AS day,
                (SELECT count(*) FROM cf.orders o WHERE o.created_at >= d.day AND o.created_at < d.day + interval '1 day') AS orders,
                (SELECT count(*) FROM cf.orders o WHERE o.status = 'fulfilled' AND o.updated_at >= d.day AND o.updated_at < d.day + interval '1 day') AS fulfilled,
                (SELECT coalesce(sum(b.amount), 0)::bigint FROM cf.burns b WHERE b.verified_at >= d.day AND b.verified_at < d.day + interval '1 day') AS clash,
                (SELECT count(*) FROM cf.users u WHERE u.created_at >= d.day AND u.created_at < d.day + interval '1 day') AS new_users,
                (SELECT count(DISTINCT e.user_id) FROM cf.events e WHERE e.created_at >= d.day AND e.created_at < d.day + interval '1 day') AS active,
                (SELECT coalesce(-sum(l.delta), 0)::bigint FROM cf.ledger l WHERE l.account = 'system:mint' AND l.currency = 'CREDITS' AND l.created_at >= d.day AND l.created_at < d.day + interval '1 day') AS credits_minted
         FROM generate_series(date_trunc('day', now()) - interval '13 days', date_trunc('day', now()), interval '1 day') AS d(day)
         ORDER BY d.day",
    )
    .fetch_all(pool)
    .await?;
    rows.iter().map(day_row).collect()
}

/// Ledger ≠ balances as the nightly `reconcile` job last reported it (`audit_log`, actor
/// `job:reconcile`, action `ledger_mismatch`, `after.count`); the full scan stays in the
/// job, not on every page view. No alert within a day and a bit = the last run was clean.
async fn ledger_mismatches(pool: &PgPool) -> Result<i64, sqlx::Error> {
    let count: Option<i64> = sqlx::query_scalar(
        "SELECT coalesce((after ->> 'count')::bigint, 0) FROM cf.audit_log
         WHERE actor = 'job:reconcile' AND action = 'ledger_mismatch' AND created_at > now() - interval '26 hours'
         ORDER BY id DESC LIMIT 1",
    )
    .fetch_optional(pool)
    .await?;
    Ok(count.unwrap_or(0))
}

/// Open rows of the alerts job first (crit, unacknowledged on top), then what this page
/// view can see by itself, without duplicating a rule the job already raised.
async fn alerts(pool: &PgPool, redis: &ConnectionManager, d: &Dashboard) -> Result<Vec<Alert>, sqlx::Error> {
    let mut out = stored_alerts(pool).await?;
    if let Some(text) = alerts_job_silent(redis).await {
        out.insert(0, Alert::instant("job_stale.alerts", "crit", text));
    }
    let mut instant = instant_alerts(pool, d).await?;
    instant.retain(|a| !out.iter().any(|o| o.rule == a.rule));
    out.extend(instant);
    Ok(out)
}

async fn stored_alerts(pool: &PgPool) -> Result<Vec<Alert>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT id, rule, level, title, acked_at IS NOT NULL AS acked FROM cf.alerts
         WHERE resolved_at IS NULL
         ORDER BY (level = 'crit') DESC, acked_at IS NOT NULL, opened_at",
    )
    .fetch_all(pool)
    .await?;
    rows.iter()
        .map(|r| {
            Ok(Alert {
                id: Some(r.try_get("id")?),
                rule: r.try_get("rule")?,
                level: r.try_get("level")?,
                text: r.try_get("title")?,
                acked: r.try_get("acked")?,
            })
        })
        .collect()
}

/// The one signal the job cannot raise about itself: no heartbeat for five minutes.
async fn alerts_job_silent(redis: &ConnectionManager) -> Option<String> {
    let mut con = redis.clone();
    let beat: Option<i64> = redis::cmd("GET")
        .arg(heartbeat_key("alerts"))
        .query_async::<Option<i64>>(&mut con)
        .await
        .ok()
        .flatten();
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    match beat {
        Some(t) if now - t <= 5 * 60 => None,
        Some(t) => Some(format!(
            "Джоба alerts мовчить {} хв — сигнали нижче можуть бути застарілі",
            (now - t) / 60
        )),
        None => Some(
            "Джоба alerts ще не бігала (cf-api без джоб?) — сигнали й Telegram не оновлюються".to_owned(),
        ),
    }
}

/// Kill switches, the review queue and the last reconcile, as seen right now.
async fn instant_alerts(pool: &PgPool, d: &Dashboard) -> Result<Vec<Alert>, sqlx::Error> {
    let mut out = Vec::new();
    if d.ledger_mismatches > 0 {
        out.push(Alert::instant(
            "ledger_mismatch",
            "crit",
            format!(
                "Ледж ≠ баланси у {} гравців — звірка негайно",
                d.ledger_mismatches
            ),
        ));
    }
    if d.payments.review_queue > 0 {
        out.push(Alert::instant(
            "review_queue",
            "warn",
            format!("{} замовлень чекають ручної перевірки", d.payments.review_queue),
        ));
    }
    let switches: Vec<(String, serde_json::Value)> = sqlx::query_as(
        "SELECT key, value FROM cf.settings WHERE key IN ('maintenance', 'shop_enabled', 'payments_enabled', 'registrations_enabled')",
    )
    .fetch_all(pool)
    .await?;
    for (key, value) in switches {
        let on = value.as_bool().unwrap_or(false);
        match key.as_str() {
            "maintenance" if on => out.push(Alert::instant(
                "maintenance_on",
                "crit",
                "Увімкнено maintenance: гра закрита".to_owned(),
            )),
            "shop_enabled" | "payments_enabled" | "registrations_enabled" if !on => {
                out.push(Alert::instant(
                    format!("switch_off.{key}"),
                    "warn",
                    format!("Вимкнено {key}"),
                ));
            }
            _ => {}
        }
    }
    Ok(out)
}

/// Open, unacknowledged rows: the badge next to «Сигнали».
pub async fn open_alerts(pool: &PgPool) -> Result<i64, sqlx::Error> {
    sqlx::query_scalar("SELECT count(*) FROM cf.alerts WHERE resolved_at IS NULL AND acked_at IS NULL")
        .fetch_one(pool)
        .await
}
