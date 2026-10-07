//! System: versions, the database clock, table sizes and every background job — its
//! heartbeat and failures from Redis (`cf_common::jobs`) and what it reported to
//! `audit_log` as `job:<name>` when something needed a human.

use std::collections::HashMap;

use axum::extract::State;
use axum::Json;
use cf_common::jobs::{failures_key, heartbeat_key, last_error_key, SCHEDULE};
use serde::Serialize;
use sqlx::Row;
use time::OffsetDateTime;

use crate::api::{ApiAdmin, ApiError};
use crate::error::AdminError;
use crate::state::AdminState;

/// One scheduled job (`cf_common::jobs::SCHEDULE`): the runner's heartbeat and failure
/// counter from Redis, plus what it wrote to `audit_log` when a human was needed.
#[derive(Debug, Serialize)]
pub struct JobRow {
    pub name: String,
    pub every_secs: u64,
    /// Unix seconds of the last completed run; `None` = not seen in this Redis.
    pub last_run_at: Option<i64>,
    pub failures_24h: i64,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub last_at: Option<OffsetDateTime>,
    pub alerts_24h: i64,
}

#[derive(Debug, Serialize)]
pub struct Counts {
    pub users: i64,
    pub islands: i64,
    pub events: i64,
    pub ledger: i64,
    pub orders: i64,
    pub sessions_active: i64,
}

#[derive(Debug, Serialize)]
pub struct SystemView {
    pub version: &'static str,
    pub env: String,
    #[serde(with = "time::serde::rfc3339")]
    pub db_now: OffsetDateTime,
    pub settings_cache_secs: i64,
    pub counts: Counts,
    pub jobs: Vec<JobRow>,
}

/// `GET /api/system`
pub async fn get(State(state): State<AdminState>, _admin: ApiAdmin) -> Result<Json<SystemView>, ApiError> {
    let row = sqlx::query(
        "SELECT now() AS db_now,
                (SELECT count(*) FROM cf.users) AS users,
                (SELECT count(*) FROM cf.islands) AS islands,
                (SELECT count(*) FROM cf.events) AS events,
                (SELECT count(*) FROM cf.ledger) AS ledger,
                (SELECT count(*) FROM cf.orders) AS orders,
                (SELECT count(*) FROM cf.sessions WHERE revoked_at IS NULL AND expires_at > now()) AS sessions_active",
    )
    .fetch_one(&state.pool)
    .await?;
    let jobs = jobs_view(&state).await?;
    Ok(Json(SystemView {
        version: env!("CARGO_PKG_VERSION"),
        env: format!("{:?}", state.config.env).to_lowercase(),
        db_now: row.try_get("db_now")?,
        settings_cache_secs: 30,
        counts: Counts {
            users: row.try_get("users")?,
            islands: row.try_get("islands")?,
            events: row.try_get("events")?,
            ledger: row.try_get("ledger")?,
            orders: row.try_get("orders")?,
            sessions_active: row.try_get("sessions_active")?,
        },
        jobs,
    }))
}

async fn jobs_view(state: &AdminState) -> Result<Vec<JobRow>, ApiError> {
    let audit = sqlx::query(
        "SELECT actor, max(created_at) AS last_at,
                (array_agg(action ORDER BY created_at DESC))[1] AS last_action,
                count(*) FILTER (WHERE created_at > now() - interval '24 hours') AS alerts_24h
         FROM cf.audit_log WHERE actor LIKE 'job:%' GROUP BY actor",
    )
    .fetch_all(&state.pool)
    .await?;
    let mut reported: Reported = HashMap::new();
    for j in &audit {
        let actor: String = j.try_get("actor")?;
        reported.insert(
            actor,
            (
                j.try_get("last_action")?,
                j.try_get("last_at")?,
                j.try_get("alerts_24h")?,
            ),
        );
    }
    let mut con = state.redis.clone();
    let mut out = Vec::with_capacity(SCHEDULE.len());
    for (name, every) in SCHEDULE {
        let (last_run_at, failures, last_error): (Option<i64>, Option<i64>, Option<String>) =
            redis::cmd("MGET")
                .arg(heartbeat_key(name))
                .arg(failures_key(name))
                .arg(last_error_key(name))
                .query_async(&mut con)
                .await
                .map_err(AdminError::from)?;
        let (last_action, last_at, alerts_24h) = reported_by(&reported, &audit_actor(name));
        out.push(JobRow {
            name: (*name).to_owned(),
            every_secs: *every,
            last_run_at,
            failures_24h: failures.unwrap_or(0),
            last_error,
            last_action,
            last_at,
            alerts_24h,
        });
    }
    // The oracle guard polls every 5 s and is not on the schedule: audit rows only.
    let (last_action, last_at, alerts_24h) = reported_by(&reported, "job:oracle");
    out.push(JobRow {
        name: "oracle_guard".to_owned(),
        every_secs: 0,
        last_run_at: None,
        failures_24h: 0,
        last_error: None,
        last_action,
        last_at,
        alerts_24h,
    });
    Ok(out)
}

type Reported = HashMap<String, (String, OffsetDateTime, i64)>;

fn reported_by(reported: &Reported, actor: &str) -> (Option<String>, Option<OffsetDateTime>, i64) {
    match reported.get(actor) {
        Some((action, at, n)) => (Some(action.clone()), Some(*at), *n),
        None => (None, None, 0),
    }
}

/// `audit_log` actors were named before the schedule (jobs/reconcile.rs, jobs/finality.rs).
fn audit_actor(name: &str) -> String {
    match name {
        "reconcile_ledger" => "job:reconcile".to_owned(),
        "finalize_burns" => "job:finality".to_owned(),
        other => format!("job:{other}"),
    }
}
