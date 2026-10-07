//! Makes an oracle pause durable (docs/SECURITY.md §4 «Оракул», RUNBOOK §7).
//! The oracle pauses itself in memory on a stale or deviating price; that flag
//! dies with the process and is invisible to a second instance. This task
//! copies it into `settings.payments_enabled = false` (audited). An operator's
//! "enable" in the admin (a row updated by a wallet, not by `oracle`) is honoured
//! exactly once per flip: the oracle resumes; if the price is still bad it pauses
//! again and the switch goes off again with a fresh audit row. Restarts and extra
//! instances follow the same rule, so nothing depends on process memory.

use std::time::Duration;

use serde_json::{json, Value};
use sqlx::{PgPool, Row};
use time::OffsetDateTime;

use crate::state::AppState;

const POLL: Duration = Duration::from_secs(5);

#[derive(Debug, Clone)]
struct Switch {
    enabled: bool,
    updated_by: String,
    updated_at: OffsetDateTime,
}

pub async fn watch(state: AppState) {
    let Some(runtime) = state.payments.clone() else {
        return;
    };
    // The operator flip we already honoured; a newer row is a new decision.
    let mut honoured: Option<OffsetDateTime> = None;
    let mut tick = tokio::time::interval(POLL);
    tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
    loop {
        tick.tick().await;
        let Some(reason) = runtime.oracle.paused() else {
            continue;
        };
        let switch = match read_switch(&state.pool).await {
            Ok(Some(s)) => s,
            Ok(None) => continue,
            Err(err) => {
                tracing::error!(error = %err, "oracle guard: settings lookup failed");
                continue;
            }
        };
        if !switch.enabled {
            continue;
        }
        let operator_flip = switch.updated_by != "oracle" && switch.updated_by != "migration";
        if operator_flip && honoured != Some(switch.updated_at) {
            honoured = Some(switch.updated_at);
            runtime.oracle.resume();
            tracing::warn!(by = %switch.updated_by, "oracle guard: payments re-enabled by operator, oracle resumed once");
            continue;
        }
        if let Err(err) = disable_payments(&state.pool, &reason).await {
            tracing::error!(error = %err, "oracle guard: could not persist the pause");
        }
    }
}

async fn read_switch(pool: &PgPool) -> Result<Option<Switch>, sqlx::Error> {
    let row =
        sqlx::query("SELECT value, updated_by, updated_at FROM cf.settings WHERE key = 'payments_enabled'")
            .fetch_optional(pool)
            .await?;
    row.map(|r| {
        Ok(Switch {
            enabled: r.try_get::<Value, _>("value")?.as_bool().unwrap_or(false),
            updated_by: r.try_get("updated_by")?,
            updated_at: r.try_get("updated_at")?,
        })
    })
    .transpose()
}

/// `payments_enabled=false` + audit row, one transaction.
pub async fn disable_payments(pool: &PgPool, reason: &str) -> Result<(), sqlx::Error> {
    let mut tx = pool.begin().await?;
    sqlx::query(
        "UPDATE cf.settings SET value = 'false'::jsonb, updated_at = now(), updated_by = 'oracle' WHERE key = 'payments_enabled'",
    )
    .execute(&mut *tx)
    .await?;
    sqlx::query(
        "INSERT INTO cf.audit_log (actor, action, target, before, after)
         VALUES ('job:oracle', 'oracle_pause', 'payments_enabled', 'true'::jsonb, $1)",
    )
    .bind(json!({ "value": false, "reason": reason }))
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    tracing::error!(
        reason,
        "oracle_pause: payments disabled until an operator re-enables them"
    );
    Ok(())
}
