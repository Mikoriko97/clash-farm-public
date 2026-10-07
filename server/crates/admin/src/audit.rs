//! `audit_log` (docs/DATA_MODEL.md): who did what, before/after, hashed IP.
//! Append-only by trigger; the admin only ever inserts and reads.

use serde_json::Value;
use sqlx::{PgConnection, PgPool, Row};
use time::OffsetDateTime;

pub async fn record(
    conn: &mut PgConnection,
    actor: &str,
    action: &str,
    target: Option<&str>,
    before: Value,
    after: Value,
    ip_hash: Option<&[u8]>,
) -> Result<(), sqlx::Error> {
    sqlx::query(
        "INSERT INTO cf.audit_log (actor, action, target, before, after, ip_hash)
         VALUES ($1, $2, $3, $4, $5, $6)",
    )
    .bind(actor)
    .bind(action)
    .bind(target)
    .bind(before)
    .bind(after)
    .bind(ip_hash)
    .execute(conn)
    .await?;
    Ok(())
}

#[derive(Debug, Clone)]
pub struct AuditRow {
    pub id: i64,
    pub actor: String,
    pub action: String,
    pub target: Option<String>,
    pub before: Option<Value>,
    pub after: Option<Value>,
    pub created_at: OffsetDateTime,
}

/// Rows whose actor, action or target contains `needle` (case-insensitive, parameterised).
pub async fn search(pool: &PgPool, needle: &str, limit: i64) -> Result<Vec<AuditRow>, sqlx::Error> {
    let pattern = format!(
        "%{}%",
        needle
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_")
    );
    let rows = sqlx::query(
        "SELECT id, actor, action, target, before, after, created_at
         FROM cf.audit_log
         WHERE actor ILIKE $1 OR action ILIKE $1 OR coalesce(target, '') ILIKE $1
         ORDER BY id DESC LIMIT $2",
    )
    .bind(pattern)
    .bind(limit.clamp(1, 500))
    .fetch_all(pool)
    .await?;
    rows.iter().map(row_from).collect()
}

pub async fn recent(pool: &PgPool, limit: i64) -> Result<Vec<AuditRow>, sqlx::Error> {
    let rows = sqlx::query(
        "SELECT id, actor, action, target, before, after, created_at
         FROM cf.audit_log ORDER BY id DESC LIMIT $1",
    )
    .bind(limit.clamp(1, 500))
    .fetch_all(pool)
    .await?;
    rows.iter().map(row_from).collect()
}

fn row_from(r: &sqlx::postgres::PgRow) -> Result<AuditRow, sqlx::Error> {
    Ok(AuditRow {
        id: r.try_get("id")?,
        actor: r.try_get("actor")?,
        action: r.try_get("action")?,
        target: r.try_get("target")?,
        before: r.try_get("before")?,
        after: r.try_get("after")?,
        created_at: r.try_get("created_at")?,
    })
}
