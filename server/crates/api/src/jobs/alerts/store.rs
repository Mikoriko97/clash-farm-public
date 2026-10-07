//! `cf.alerts` bookkeeping for one tick: upsert the firing rules (one open row per rule),
//! resolve the rest, drop expired mutes, and pick what is due for delivery.

use serde_json::Value;
use sqlx::{PgPool, Row};

use super::Signal;

/// An open alert that should reach the operator now.
#[derive(Debug, Clone)]
pub struct OpenRow {
    pub id: i64,
    pub rule: String,
    pub level: String,
    pub title: String,
    pub detail: Value,
    /// Already announced once; this is the six-hourly reminder of an unread crit.
    pub reminder: bool,
    pub open_hours: i64,
}

#[derive(Debug, Clone)]
pub struct Resolved {
    pub rule: String,
    pub title: String,
    /// The operator heard about it, so tell them it is over.
    pub notified: bool,
}

#[derive(Debug, Default)]
pub struct Sync {
    pub open: i64,
    /// Rows created this tick.
    pub opened: usize,
    pub resolved: Vec<Resolved>,
    pub to_notify: Vec<OpenRow>,
}

pub async fn sync(pool: &PgPool, signals: &[Signal]) -> Result<Sync, sqlx::Error> {
    let mut tx = pool.begin().await?;
    let mut out = Sync::default();
    for s in signals {
        let inserted: bool = sqlx::query_scalar(
            "INSERT INTO cf.alerts (rule, level, title, detail) VALUES ($1, $2, $3, $4)
             ON CONFLICT (rule) WHERE resolved_at IS NULL DO UPDATE
                SET last_seen_at = now(), level = EXCLUDED.level, title = EXCLUDED.title, detail = EXCLUDED.detail
             RETURNING (xmax = 0)",
        )
        .bind(&s.rule)
        .bind(s.level.as_str())
        .bind(&s.title)
        .bind(&s.detail)
        .fetch_one(&mut *tx)
        .await?;
        if inserted {
            out.opened += 1;
        }
    }
    let rules: Vec<String> = signals.iter().map(|s| s.rule.clone()).collect();
    let resolved = sqlx::query(
        "UPDATE cf.alerts SET resolved_at = now()
         WHERE resolved_at IS NULL AND NOT (rule = ANY($1))
         RETURNING rule, title, notified_at IS NOT NULL AS notified",
    )
    .bind(&rules)
    .fetch_all(&mut *tx)
    .await?;
    for r in &resolved {
        out.resolved.push(Resolved {
            rule: r.try_get("rule")?,
            title: r.try_get("title")?,
            notified: r.try_get("notified")?,
        });
    }
    sqlx::query("DELETE FROM cf.alert_mutes WHERE until <= now()")
        .execute(&mut *tx)
        .await?;
    let due = sqlx::query(
        "SELECT a.id, a.rule, a.level, a.title, a.detail,
                a.notified_at IS NOT NULL AS reminder,
                (EXTRACT(EPOCH FROM (now() - a.opened_at)) / 3600)::bigint AS open_hours
         FROM cf.alerts a
         LEFT JOIN cf.alert_mutes m ON m.rule = a.rule AND m.until > now()
         WHERE a.resolved_at IS NULL AND a.acked_at IS NULL AND m.rule IS NULL
           AND (a.notified_at IS NULL OR (a.level = 'crit' AND a.notified_at < now() - interval '6 hours'))
         ORDER BY a.opened_at",
    )
    .fetch_all(&mut *tx)
    .await?;
    for r in &due {
        out.to_notify.push(OpenRow {
            id: r.try_get("id")?,
            rule: r.try_get("rule")?,
            level: r.try_get("level")?,
            title: r.try_get("title")?,
            detail: r.try_get("detail")?,
            reminder: r.try_get("reminder")?,
            open_hours: r.try_get("open_hours")?,
        });
    }
    out.open = sqlx::query_scalar("SELECT count(*) FROM cf.alerts WHERE resolved_at IS NULL")
        .fetch_one(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(out)
}

pub async fn mark_notified(pool: &PgPool, id: i64) -> Result<(), sqlx::Error> {
    sqlx::query("UPDATE cf.alerts SET notified_at = now() WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await?;
    Ok(())
}
