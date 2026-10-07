//! The `alerts` job (ADR-023): every minute the rules in `rules.rs` look at the database
//! and at the other jobs' heartbeats; `store.rs` keeps one open row per rule in
//! `cf.alerts` (opened → seen → resolved) and `telegram.rs` tells the operator — once
//! when a signal opens, once when it resolves, and every six hours while a critical one
//! stays unacknowledged. A page view never sends anything: the admin only reads,
//! acknowledges and mutes.

pub mod rules;
pub mod store;
pub mod telegram;

use std::future::Future;
use std::sync::OnceLock;

use redis::aio::ConnectionManager;
use serde_json::Value;
use sqlx::PgPool;

use crate::jobs::JobError;
use crate::state::AppState;
use telegram::{esc, NoChannel, Telegram};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Level {
    Warn,
    Crit,
}

impl Level {
    pub fn as_str(self) -> &'static str {
        match self {
            Level::Warn => "warn",
            Level::Crit => "crit",
        }
    }
}

/// One firing rule; `detail` is a flat object shown as key: value lines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Signal {
    pub rule: String,
    pub level: Level,
    pub title: String,
    pub detail: Value,
}

/// Where messages go. `Ok(true)` = delivered (the row is marked), `Ok(false)` = no channel
/// configured (nothing is claimed), `Err` = try again next tick.
pub trait Notifier: Sync {
    fn send(&self, text: &str) -> impl Future<Output = Result<bool, String>> + Send;
}

/// The Telegram client lives for the process (one connection pool), built on first use.
static CHANNEL: OnceLock<Option<Telegram>> = OnceLock::new();

pub async fn run(state: &AppState) -> Result<String, JobError> {
    let env = format!("{:?}", state.config.env).to_lowercase();
    let channel = CHANNEL.get_or_init(|| {
        state.config.alerts.as_ref().and_then(|cfg| match Telegram::new(cfg) {
            Ok(tg) => Some(tg),
            Err(err) => {
                tracing::error!(error = %err, "alerts: telegram client unavailable, signals stay in the admin only");
                None
            }
        })
    });
    match channel {
        Some(tg) => run_with(&state.pool, &state.redis, tg, &env).await,
        None => run_with(&state.pool, &state.redis, &NoChannel, &env).await,
    }
}

/// One tick: evaluate → sync rows → deliver what is due. Tests call this with a recorder.
pub async fn run_with<N: Notifier>(
    pool: &PgPool,
    redis: &ConnectionManager,
    notifier: &N,
    env: &str,
) -> Result<String, JobError> {
    let thresholds = rules::Thresholds::load(pool).await?;
    let signals = rules::evaluate(pool, redis, &thresholds).await?;
    let sync = store::sync(pool, &signals).await?;
    let mut sent = 0usize;
    for row in &sync.to_notify {
        match notifier.send(&format_open(row, env)).await {
            Ok(true) => {
                store::mark_notified(pool, row.id).await?;
                sent += 1;
            }
            Ok(false) => {}
            Err(err) => tracing::warn!(rule = %row.rule, error = %err, "alert_send_failed"),
        }
    }
    for row in sync.resolved.iter().filter(|r| r.notified) {
        // Best effort: a lost resolution notice is not worth a retry loop.
        if let Err(err) = notifier.send(&format_resolved(row, env)).await {
            tracing::warn!(rule = %row.rule, error = %err, "alert_resolve_send_failed");
        }
    }
    Ok(format!(
        "open={} new={} resolved={} sent={sent}",
        sync.open,
        sync.opened,
        sync.resolved.len()
    ))
}

fn format_open(row: &store::OpenRow, env: &str) -> String {
    let head = match (row.level.as_str(), row.reminder) {
        ("crit", true) => format!("🔴 Досі відкрито ({} год)", row.open_hours),
        ("crit", false) => "🔴 Критично".to_owned(),
        _ => "🟠 Увага".to_owned(),
    };
    let mut text = format!("{head} · <b>{}</b>", esc(&row.title));
    if let Some(detail) = row.detail.as_object() {
        for (key, value) in detail {
            let value = match value {
                Value::String(s) => s.clone(),
                other => other.to_string(),
            };
            let value: String = value.chars().take(120).collect();
            text.push_str(&format!("\n{}: {}", esc(key), esc(&value)));
        }
    }
    text.push_str(&format!("\n<i>Clash Farm · {}</i>", esc(env)));
    text
}

fn format_resolved(row: &store::Resolved, env: &str) -> String {
    format!(
        "✅ Вирішено · {}\n<i>Clash Farm · {}</i>",
        esc(&row.title),
        esc(env)
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn messages_are_escaped_and_carry_the_detail() {
        let row = store::OpenRow {
            id: 1,
            rule: "x".into(),
            level: "crit".into(),
            title: "Ледж <> баланси".into(),
            detail: json!({ "count": 3, "who": "a&b" }),
            reminder: false,
            open_hours: 0,
        };
        let text = format_open(&row, "prod");
        assert!(
            text.starts_with("🔴 Критично · <b>Ледж &lt;&gt; баланси</b>\n"),
            "{text}"
        );
        assert!(
            text.contains("\ncount: 3\n") && text.contains("who: a&amp;b"),
            "{text}"
        );
        assert!(text.ends_with("<i>Clash Farm · prod</i>"));
        let reminder = store::OpenRow {
            reminder: true,
            open_hours: 7,
            ..row
        };
        assert!(format_open(&reminder, "prod").starts_with("🔴 Досі відкрито (7 год)"));
        let done = store::Resolved {
            rule: "x".into(),
            title: "t".into(),
            notified: true,
        };
        assert_eq!(
            format_resolved(&done, "local"),
            "✅ Вирішено · t\n<i>Clash Farm · local</i>"
        );
    }
}
