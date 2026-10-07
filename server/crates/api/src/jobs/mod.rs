//! Background jobs (week 5, docs/RUNBOOK.md §Jobs). Each job is a plain async
//! function over the pool so tests call it directly; `spawn_all` wraps them in
//! periodic tasks guarded by a Redis lock (`SET NX PX`) so that several api
//! instances never run the same job at once. Jobs only ever log and write
//! `audit_log`/`events`: they never touch balances or the ledger.

pub mod alerts;
pub mod expire;
pub mod finality;
pub mod oracle_guard;
pub mod reconcile;
pub mod retention;

use std::future::Future;
use std::time::Duration;

use cf_common::jobs::{
    every_secs, failures_key, heartbeat_key, last_error_key, FAILURES_TTL_SECS, HEARTBEAT_TTL_SECS,
};

use redis::aio::ConnectionManager;
use serde_json::Value;
use sqlx::PgPool;
use thiserror::Error;

use crate::state::AppState;

#[derive(Debug, Error)]
pub enum JobError {
    #[error("database error")]
    Db(#[from] sqlx::Error),
    #[error("chain error: {0}")]
    Chain(#[from] cf_chain::ChainError),
    #[error("redis error")]
    Redis(#[from] redis::RedisError),
    #[error("world error: {0}")]
    World(#[from] cf_world::error::WorldError),
}

/// Starts every periodic job. Called once from the binary; tests call jobs directly.
pub fn spawn_all(state: AppState) {
    spawn_periodic(state.clone(), "expire_quotes", |s| async move {
        expire::run(&s.pool).await
    });
    spawn_periodic(state.clone(), "retention", |s| async move {
        retention::run(&s.pool).await
    });
    spawn_periodic(state.clone(), "reconcile_ledger", |s| async move {
        reconcile::run(&s.pool).await
    });
    spawn_periodic(state.clone(), "finalize_burns", |s| async move {
        finality::run(&s.pool, s.payments.as_deref()).await
    });
    spawn_periodic(state.clone(), "alerts", |s| async move { alerts::run(&s).await });
    spawn_periodic(state.clone(), "regrow_resources", |s| async move {
        let params = s.params.get(&s.pool).await?;
        let done = cf_world::regrow::run(&s.pool, params).await?;
        Ok(format!(
            "islands={} moved={} planted={}",
            done.islands, done.moved, done.planted
        ))
    });
    tokio::spawn(oracle_guard::watch(state));
}

/// Cadence comes from `cf_common::jobs::SCHEDULE` so the admin and the alerts job know
/// how often each job is expected; after every run the runner leaves a heartbeat and,
/// on failure, a counter and the error text in Redis (`cf_common::jobs` keys).
fn spawn_periodic<F, Fut>(state: AppState, name: &'static str, job: F)
where
    F: Fn(AppState) -> Fut + Send + 'static,
    Fut: Future<Output = Result<String, JobError>> + Send,
{
    let every = Duration::from_secs(every_secs(name).unwrap_or_else(|| {
        tracing::error!(job = name, "job missing from SCHEDULE; running every 60 s");
        60
    }));
    tokio::spawn(async move {
        let mut tick = tokio::time::interval(every);
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        loop {
            tick.tick().await;
            // The lock lives as long as the interval: one instance per period.
            if !try_lock(&state.redis, name, every).await {
                continue;
            }
            match job(state.clone()).await {
                Ok(summary) => tracing::info!(job = name, %summary, "job_done"),
                Err(err) => {
                    tracing::error!(job = name, error = %err, "job_failed");
                    record_failure(&state.redis, name, &err.to_string()).await;
                }
            }
            record_heartbeat(&state.redis, name).await;
        }
    });
}

async fn record_heartbeat(redis: &ConnectionManager, name: &str) {
    let mut con = redis.clone();
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    let set: Result<(), redis::RedisError> = redis::cmd("SET")
        .arg(heartbeat_key(name))
        .arg(now)
        .arg("EX")
        .arg(HEARTBEAT_TTL_SECS)
        .query_async(&mut con)
        .await;
    if let Err(err) = set {
        tracing::warn!(job = name, error = %err, "job heartbeat not recorded");
    }
}

async fn record_failure(redis: &ConnectionManager, name: &str, error: &str) {
    let mut con = redis.clone();
    let error: String = error.chars().take(200).collect();
    let mut pipe = redis::pipe();
    pipe.cmd("INCR").arg(failures_key(name)).ignore();
    pipe.cmd("EXPIRE")
        .arg(failures_key(name))
        .arg(FAILURES_TTL_SECS)
        .ignore();
    pipe.cmd("SET")
        .arg(last_error_key(name))
        .arg(error)
        .arg("EX")
        .arg(FAILURES_TTL_SECS)
        .ignore();
    let done: Result<(), redis::RedisError> = pipe.query_async(&mut con).await;
    if let Err(err) = done {
        tracing::warn!(job = name, error = %err, "job failure not recorded");
    }
}

/// `SET jobs:lock:<name> NX PX <ttl>`; a Redis failure counts as "not ours" (fail closed).
pub async fn try_lock(redis: &ConnectionManager, name: &str, ttl: Duration) -> bool {
    let mut con = redis.clone();
    let ttl_ms = u64::try_from(ttl.as_millis()).unwrap_or(u64::MAX).max(1000);
    let set: Result<Option<String>, redis::RedisError> = redis::cmd("SET")
        .arg(format!("jobs:lock:{name}"))
        .arg("1")
        .arg("NX")
        .arg("PX")
        .arg(ttl_ms)
        .query_async(&mut con)
        .await;
    match set {
        Ok(v) => v.is_some(),
        Err(err) => {
            tracing::error!(job = name, error = %err, "job lock unavailable");
            false
        }
    }
}

/// Alerts from jobs go to `audit_log` (visible on the admin home page) and to the log.
pub async fn alert(
    pool: &PgPool,
    job: &str,
    action: &str,
    target: Option<&str>,
    detail: Value,
) -> Result<(), sqlx::Error> {
    tracing::error!(job, action, target, %detail, "job_alert");
    sqlx::query(
        "INSERT INTO cf.audit_log (actor, action, target, before, after) VALUES ($1, $2, $3, NULL, $4)",
    )
    .bind(format!("job:{job}"))
    .bind(action)
    .bind(target)
    .bind(detail)
    .execute(pool)
    .await?;
    Ok(())
}
