//! `/api/alerts`: what the alerts job (cf-api, ADR-023) opened, resolved and delivered;
//! acknowledge one (stops reminders), mute a rule for a while (stops delivery), unmute.
//! Reads never send anything; every write is audited.

use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::Json;
use cf_common::jobs::heartbeat_key;
use serde::Deserialize;
use serde_json::{json, Value};
use time::OffsetDateTime;

use crate::api::{ApiAdmin, ApiError, PageQuery};
use crate::audit;
use crate::error::AdminError;
use crate::state::AdminState;

/// Rules the job raises; `switch_off.<key>`, `job_failed.<job>`, `job_stale.<job>` carry a suffix.
pub const RULES: [&str; 6] = [
    "ledger_mismatch",
    "review_queue",
    "maintenance_on",
    "oracle_paused",
    "signup_spike",
    "totp_rejected",
];
const RULE_PREFIXES: [&str; 3] = ["switch_off.", "job_failed.", "job_stale."];
const MAX_MUTE_HOURS: i64 = 168;

#[derive(Debug, Deserialize)]
pub struct ListQuery {
    /// `open` (default) | `resolved` | `all`
    pub state: Option<String>,
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

/// `GET /api/alerts?state=open|resolved|all&page=&per_page=`
pub async fn list(
    State(state): State<AdminState>,
    _admin: ApiAdmin,
    Query(q): Query<ListQuery>,
) -> Result<Json<Value>, ApiError> {
    let (limit, offset) = PageQuery {
        page: q.page,
        per_page: q.per_page,
    }
    .bounds();
    let filter = match q.state.as_deref().unwrap_or("open") {
        "open" => "resolved_at IS NULL",
        "resolved" => "resolved_at IS NOT NULL",
        "all" => "TRUE",
        _ => {
            return Err(ApiError(AdminError::BadRequest(
                "state: open | resolved | all".to_owned(),
            )))
        }
    };
    let sql = format!(
        "SELECT coalesce(jsonb_agg(to_jsonb(a)), '[]'::jsonb) FROM (
             SELECT id, rule, level, title, detail, opened_at, last_seen_at, resolved_at, notified_at,
                    acked_by, acked_at, ack_note, count(*) OVER () AS total
             FROM cf.alerts WHERE {filter}
             ORDER BY resolved_at IS NOT NULL, (level = 'crit') DESC, acked_at IS NOT NULL, opened_at DESC
             LIMIT $1 OFFSET $2) a"
    );
    let rows: Value = sqlx::query_scalar(sqlx::AssertSqlSafe(sql))
        .bind(limit)
        .bind(offset)
        .fetch_one(&state.pool)
        .await?;
    let total = rows
        .as_array()
        .and_then(|a| a.first())
        .and_then(|r| r["total"].as_i64())
        .unwrap_or(0);
    let mutes: Value = sqlx::query_scalar(
        "SELECT coalesce(jsonb_agg(to_jsonb(m) ORDER BY m.until), '[]'::jsonb)
         FROM (SELECT rule, until, muted_by, note FROM cf.alert_mutes WHERE until > now()) m",
    )
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(json!({
        "rows": rows,
        "total": total,
        "per_page": limit,
        "mutes": mutes,
        "job_last_run_at": job_last_run(&state).await,
        "rules": RULES,
        "rule_prefixes": RULE_PREFIXES,
    })))
}

/// Unix seconds of the alerts job's last heartbeat (`None` = never seen in this Redis).
pub async fn job_last_run(state: &AdminState) -> Option<i64> {
    let mut con = state.redis.clone();
    redis::cmd("GET")
        .arg(heartbeat_key("alerts"))
        .query_async::<Option<i64>>(&mut con)
        .await
        .ok()
        .flatten()
}

#[derive(Debug, Deserialize)]
pub struct AckBody {
    pub note: Option<String>,
}

/// `POST /api/alerts/{id}/ack { note? }`: the operator has seen it; reminders stop.
pub async fn ack(
    State(state): State<AdminState>,
    admin: ApiAdmin,
    headers: HeaderMap,
    Path(id): Path<i64>,
    Json(body): Json<AckBody>,
) -> Result<Json<Value>, ApiError> {
    admin.guard(&state, &headers).await?;
    let note: Option<String> = body
        .note
        .map(|n| n.trim().chars().take(200).collect::<String>())
        .filter(|n| !n.is_empty());
    let mut tx = state.pool.begin().await?;
    let row: Option<(String, String, bool)> = sqlx::query_as(
        "SELECT rule, title, acked_at IS NOT NULL FROM cf.alerts WHERE id = $1 AND resolved_at IS NULL FOR UPDATE",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?;
    let Some((rule, title, acked)) = row else {
        return Err(ApiError(AdminError::NotFound));
    };
    if acked {
        return Err(ApiError(AdminError::BadRequest("уже підтверджено".to_owned())));
    }
    sqlx::query("UPDATE cf.alerts SET acked_by = $2, acked_at = now(), ack_note = $3 WHERE id = $1")
        .bind(id)
        .bind(admin.wallet())
        .bind(&note)
        .execute(&mut *tx)
        .await?;
    audit::record(
        &mut tx,
        admin.wallet(),
        "alert_ack",
        Some(&id.to_string()),
        json!({ "rule": rule, "title": title }),
        json!({ "note": note }),
        None,
    )
    .await?;
    tx.commit().await?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Debug, Deserialize)]
pub struct MuteBody {
    pub rule: String,
    pub hours: i64,
    pub note: Option<String>,
}

/// `POST /api/alerts/mute { rule, hours, note? }`: no delivery for this rule until then;
/// rows still open and show in the admin.
pub async fn mute(
    State(state): State<AdminState>,
    admin: ApiAdmin,
    headers: HeaderMap,
    Json(body): Json<MuteBody>,
) -> Result<Json<Value>, ApiError> {
    admin.guard(&state, &headers).await?;
    let rule = valid_rule(&body.rule)?;
    if !(1..=MAX_MUTE_HOURS).contains(&body.hours) {
        return Err(ApiError(AdminError::BadRequest("hours: 1 … 168".to_owned())));
    }
    let note: Option<String> = body
        .note
        .map(|n| n.trim().chars().take(200).collect::<String>())
        .filter(|n| !n.is_empty());
    let until = OffsetDateTime::now_utc() + time::Duration::hours(body.hours);
    let mut tx = state.pool.begin().await?;
    sqlx::query(
        "INSERT INTO cf.alert_mutes (rule, until, muted_by, note) VALUES ($1, $2, $3, $4)
         ON CONFLICT (rule) DO UPDATE SET until = EXCLUDED.until, muted_by = EXCLUDED.muted_by,
                                         note = EXCLUDED.note, created_at = now()",
    )
    .bind(&rule)
    .bind(until)
    .bind(admin.wallet())
    .bind(&note)
    .execute(&mut *tx)
    .await?;
    audit::record(
        &mut tx,
        admin.wallet(),
        "alert_mute",
        Some(&rule),
        Value::Null,
        json!({ "hours": body.hours, "until": until.unix_timestamp(), "note": note }),
        None,
    )
    .await?;
    tx.commit().await?;
    Ok(Json(json!({ "ok": true, "until": until.unix_timestamp() })))
}

#[derive(Debug, Deserialize)]
pub struct UnmuteBody {
    pub rule: String,
}

/// `POST /api/alerts/unmute { rule }`
pub async fn unmute(
    State(state): State<AdminState>,
    admin: ApiAdmin,
    headers: HeaderMap,
    Json(body): Json<UnmuteBody>,
) -> Result<Json<Value>, ApiError> {
    admin.guard(&state, &headers).await?;
    let rule = valid_rule(&body.rule)?;
    let mut tx = state.pool.begin().await?;
    let until: Option<OffsetDateTime> =
        sqlx::query_scalar("DELETE FROM cf.alert_mutes WHERE rule = $1 RETURNING until")
            .bind(&rule)
            .fetch_optional(&mut *tx)
            .await?;
    let Some(until) = until else {
        return Err(ApiError(AdminError::NotFound));
    };
    audit::record(
        &mut tx,
        admin.wallet(),
        "alert_unmute",
        Some(&rule),
        json!({ "until": until.unix_timestamp() }),
        Value::Null,
        None,
    )
    .await?;
    tx.commit().await?;
    Ok(Json(json!({ "ok": true })))
}

/// A rule the job can raise: one of `RULES`, or a prefix with a `[a-z0-9_]` suffix.
fn valid_rule(raw: &str) -> Result<String, ApiError> {
    let rule = raw.trim();
    let shape_ok = !rule.is_empty()
        && rule.len() <= 64
        && rule
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'.');
    let known = RULES.contains(&rule)
        || RULE_PREFIXES.iter().any(|p| {
            rule.strip_prefix(p)
                .is_some_and(|s| !s.is_empty() && !s.contains('.'))
        });
    if shape_ok && known {
        Ok(rule.to_owned())
    } else {
        Err(ApiError(AdminError::BadRequest(
            "невідоме правило: ledger_mismatch, review_queue, maintenance_on, oracle_paused, signup_spike, totp_rejected, switch_off.<key>, job_failed.<job>, job_stale.<job>".to_owned(),
        )))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rules_are_allowlisted() {
        assert!(valid_rule("review_queue").is_ok());
        assert!(valid_rule(" job_failed.retention ").is_ok());
        assert!(valid_rule("switch_off.shop_enabled").is_ok());
        assert!(valid_rule("job_stale.").is_err(), "empty suffix");
        assert!(valid_rule("job_stale.a.b").is_err(), "one dot");
        assert!(valid_rule("Review_Queue").is_err(), "lowercase only");
        assert!(valid_rule("evil rule").is_err());
        assert!(valid_rule("unknown_rule").is_err());
        assert!(valid_rule(&"a".repeat(65)).is_err());
    }
}
