//! Players: a searchable list and one player's full picture (island, plots, inventory,
//! balances, ledger, orders, events, sessions, today's caps). The only write is the
//! admin flag on `users.flags`, audited with the old and new value.

use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::postgres::PgRow;
use sqlx::Row;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::api::{like_pattern, ApiAdmin, ApiError, PageQuery};
use crate::audit;
use crate::error::AdminError;
use crate::state::AdminState;

/// `page`/`per_page` sit here directly: serde's `flatten` cannot parse numbers from a query string.
#[derive(Debug, Deserialize, Default)]
pub struct ListQuery {
    pub q: Option<String>,
    pub sort: Option<String>,
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct PlayerRow {
    pub id: Uuid,
    /// `None` for a guest island (0028) that has not linked a wallet.
    pub wallet: Option<String>,
    pub nickname: Option<String>,
    pub level: i32,
    pub xp: i64,
    pub credits: i64,
    pub plots: i64,
    pub orders: i64,
    pub reputation: i32,
    pub flags: Value,
    #[serde(with = "time::serde::rfc3339::option")]
    pub banned_at: Option<OffsetDateTime>,
    pub ban_reason: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub last_seen_at: OffsetDateTime,
}

fn player_row(r: &PgRow) -> Result<PlayerRow, sqlx::Error> {
    Ok(PlayerRow {
        id: r.try_get("id")?,
        wallet: r.try_get("wallet")?,
        nickname: r.try_get("nickname")?,
        level: r.try_get("level")?,
        xp: r.try_get("xp")?,
        credits: r.try_get("credits")?,
        plots: r.try_get("plots")?,
        orders: r.try_get("orders")?,
        reputation: r.try_get("reputation")?,
        flags: r.try_get("flags")?,
        banned_at: r.try_get("banned_at")?,
        ban_reason: r.try_get("ban_reason")?,
        created_at: r.try_get("created_at")?,
        last_seen_at: r.try_get("last_seen_at")?,
    })
}

const LIST_SQL: &str =
    "SELECT u.id, u.wallet, u.nickname, u.created_at, u.last_seen_at, u.reputation, u.flags, u.banned_at, u.ban_reason,
        coalesce(i.level, 1) AS level, coalesce(i.xp, 0)::bigint AS xp,
        coalesce(b.amount, 0)::bigint AS credits,
        (SELECT count(*) FROM cf.plots p WHERE p.island_id = i.id) AS plots,
        (SELECT count(*) FROM cf.orders o WHERE o.user_id = u.id AND o.status = 'fulfilled') AS orders,
        count(*) OVER () AS total
 FROM cf.users u
 LEFT JOIN cf.islands i ON i.user_id = u.id
 LEFT JOIN cf.balances b ON b.user_id = u.id AND b.account = 'credits'
 WHERE ($1 = '' OR coalesce(u.wallet, '') ILIKE $2 OR coalesce(u.nickname, '') ILIKE $2 OR u.id::text = $1
        OR EXISTS (SELECT 1 FROM cf.nickname_history h WHERE h.user_id = u.id AND h.old_nickname ILIKE $2))";

/// Sort keys are a fixed table: the request picks a name, never SQL.
fn order_by(sort: Option<&str>) -> &'static str {
    match sort.unwrap_or("") {
        "created" => " ORDER BY u.created_at DESC",
        "level" => " ORDER BY level DESC, xp DESC",
        "credits" => " ORDER BY credits DESC",
        "orders" => " ORDER BY orders DESC, u.last_seen_at DESC",
        "banned" => " ORDER BY u.banned_at DESC NULLS LAST, u.last_seen_at DESC",
        _ => " ORDER BY u.last_seen_at DESC",
    }
}

/// `GET /api/players?q=&sort=&page=&per_page=` → `{ total, rows }`.
pub async fn list(
    State(state): State<AdminState>,
    _admin: ApiAdmin,
    Query(q): Query<ListQuery>,
) -> Result<Json<Value>, ApiError> {
    let needle: String = q.q.unwrap_or_default().trim().chars().take(64).collect();
    let (limit, offset) = PageQuery {
        page: q.page,
        per_page: q.per_page,
    }
    .bounds();
    // Only the fixed ORDER BY clause is spliced in; every user value is a bind parameter.
    let sql = format!("{LIST_SQL}{} LIMIT $3 OFFSET $4", order_by(q.sort.as_deref()));
    let rows = sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind(&needle)
        .bind(like_pattern(&needle))
        .bind(limit)
        .bind(offset)
        .fetch_all(&state.pool)
        .await?;
    let total: i64 = rows.first().map(|r| r.try_get("total")).transpose()?.unwrap_or(0);
    let players = rows.iter().map(player_row).collect::<Result<Vec<_>, _>>()?;
    Ok(Json(serde_json::json!({ "total": total, "rows": players })))
}

/// One parameterised SQL returning a single JSON value (or nothing) for a detail sub-list.
async fn rows_json(
    pool: &sqlx::PgPool,
    sql: &'static str,
    id: Uuid,
    empty: Value,
) -> Result<Value, ApiError> {
    let v: Option<Option<Value>> = sqlx::query_scalar(sql).bind(id).fetch_optional(pool).await?;
    Ok(v.flatten().unwrap_or(empty))
}

/// `GET /api/players/{id}`: everything the admin needs on one screen.
pub async fn detail(
    State(state): State<AdminState>,
    _admin: ApiAdmin,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, ApiError> {
    let sql = format!("{LIST_SQL} AND u.id = $3 LIMIT 1");
    let row = sqlx::query(sqlx::AssertSqlSafe(sql))
        .bind("")
        .bind("")
        .bind(id)
        .fetch_optional(&state.pool)
        .await?
        .ok_or(ApiError(AdminError::NotFound))?;
    let user = player_row(&row)?;
    let pool = &state.pool;
    let island = rows_json(pool, "SELECT to_jsonb(t) FROM (SELECT id, level, xp, expand_n, expand_e, expand_s, expand_w, created_at FROM cf.islands WHERE user_id = $1) t", id, Value::Null).await?;
    let plots = rows_json(pool, "SELECT coalesce(jsonb_agg(to_jsonb(t) ORDER BY idx), '[]'::jsonb) FROM (SELECT p.idx, p.col, p.\"row\", p.state, p.crop_id, p.ready_at, p.fertilizer_id FROM cf.plots p JOIN cf.islands i ON i.id = p.island_id WHERE i.user_id = $1) t", id, Value::Array(Vec::new())).await?;
    let inventory = rows_json(pool, "SELECT coalesce(jsonb_agg(to_jsonb(t) ORDER BY item_id), '[]'::jsonb) FROM (SELECT item_id, qty FROM cf.inventory WHERE user_id = $1 AND qty > 0) t", id, Value::Array(Vec::new())).await?;
    let balances = rows_json(pool, "SELECT coalesce(jsonb_agg(to_jsonb(t) ORDER BY account), '[]'::jsonb) FROM (SELECT account, amount, updated_at FROM cf.balances WHERE user_id = $1) t", id, Value::Array(Vec::new())).await?;
    let ledger = rows_json(pool, "SELECT coalesce(jsonb_agg(to_jsonb(t)), '[]'::jsonb) FROM (SELECT id, account, delta, currency, ref_type, ref_id, created_at FROM cf.ledger WHERE user_id = $1 ORDER BY id DESC LIMIT 50) t", id, Value::Array(Vec::new())).await?;
    let orders = rows_json(pool, "SELECT coalesce(jsonb_agg(to_jsonb(t)), '[]'::jsonb) FROM (SELECT id, item_id, qty, price_usd_cents, amount_clash, status, reason_code, created_at, updated_at FROM cf.orders WHERE user_id = $1 ORDER BY created_at DESC LIMIT 20) t", id, Value::Array(Vec::new())).await?;
    let events = rows_json(pool, "SELECT coalesce(jsonb_agg(to_jsonb(t)), '[]'::jsonb) FROM (SELECT id, name, props, created_at FROM cf.events WHERE user_id = $1 ORDER BY id DESC LIMIT 50) t", id, Value::Array(Vec::new())).await?;
    let caps = rows_json(pool, "SELECT coalesce(jsonb_agg(to_jsonb(t) ORDER BY action), '[]'::jsonb) FROM (SELECT action, count FROM cf.rate_events WHERE user_id = $1 AND day = (now() AT TIME ZONE 'UTC')::date) t", id, Value::Array(Vec::new())).await?;
    let sessions = rows_json(pool, "SELECT to_jsonb(t) FROM (SELECT count(*) AS total, count(*) FILTER (WHERE revoked_at IS NULL AND expires_at > now()) AS active, max(created_at) AS last_at FROM cf.sessions WHERE user_id = $1) t", id, Value::Null).await?;
    // Paid renames (0030, ADR-039) and moderator resets (0031, `kind`): every name the player had.
    let names = rows_json(pool, "SELECT coalesce(jsonb_agg(to_jsonb(t)), '[]'::jsonb) FROM (SELECT old_nickname, new_nickname, changed_at, kind FROM cf.nickname_history WHERE user_id = $1 ORDER BY id DESC LIMIT 20) t", id, Value::Array(Vec::new())).await?;
    Ok(Json(serde_json::json!({
        "user": user, "island": island, "plots": plots, "inventory": inventory, "balances": balances,
        "ledger": ledger, "orders": orders, "events": events, "caps_today": caps, "sessions": sessions, "names": names,
    })))
}

#[derive(Debug, Deserialize)]
pub struct FlagBody {
    pub suspect: bool,
    #[serde(default)]
    pub note: String,
}

/// `POST /api/players/{id}/flag { suspect, note }`: merges into `users.flags`, audited.
pub async fn flag(
    State(state): State<AdminState>,
    admin: ApiAdmin,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(body): Json<FlagBody>,
) -> Result<Json<Value>, ApiError> {
    admin.guard(&state, &headers).await?;
    let note: String = body.note.trim().chars().take(200).collect();
    let patch = serde_json::json!({ "suspect": body.suspect, "note": note, "by": admin.wallet(), "at": OffsetDateTime::now_utc().unix_timestamp() });
    let mut tx = state.pool.begin().await?;
    let before: Value = sqlx::query_scalar("SELECT flags FROM cf.users WHERE id = $1 FOR UPDATE")
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(ApiError(AdminError::NotFound))?;
    let after: Value =
        sqlx::query_scalar("UPDATE cf.users SET flags = flags || $2 WHERE id = $1 RETURNING flags")
            .bind(id)
            .bind(&patch)
            .fetch_one(&mut *tx)
            .await?;
    audit::record(
        &mut tx,
        admin.wallet(),
        "player_flag",
        Some(&id.to_string()),
        before,
        after.clone(),
        None,
    )
    .await?;
    tx.commit().await?;
    tracing::warn!(actor = %admin.wallet(), player = %id, suspect = body.suspect, "admin_player_flag");
    Ok(Json(serde_json::json!({ "flags": after })))
}

#[derive(Debug, Deserialize)]
pub struct BanBody {
    pub reason: String,
}

/// `POST /api/players/{id}/ban { reason }`: sets `banned_at`, revokes every live session
/// (refresh and access alike) and audits. Sign-in and refresh refuse the wallet from now on.
pub async fn ban(
    State(state): State<AdminState>,
    admin: ApiAdmin,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(body): Json<BanBody>,
) -> Result<Json<Value>, ApiError> {
    admin.guard(&state, &headers).await?;
    let reason: String = body.reason.trim().chars().take(200).collect();
    if reason.is_empty() {
        return Err(ApiError(AdminError::BadRequest("причина обовʼязкова".to_owned())));
    }
    let mut tx = state.pool.begin().await?;
    let before: Option<OffsetDateTime> =
        sqlx::query_scalar("SELECT banned_at FROM cf.users WHERE id = $1 FOR UPDATE")
            .bind(id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(ApiError(AdminError::NotFound))?;
    // Idempotent: a repeat (or a retry after a session-store hiccup) keeps the first ban's
    // time and reason, writes no second audit row, and still revokes the sessions.
    if before.is_none() {
        sqlx::query("UPDATE cf.users SET banned_at = now(), ban_reason = $2 WHERE id = $1")
            .bind(id)
            .bind(&reason)
            .execute(&mut *tx)
            .await?;
        audit::record(
            &mut tx,
            admin.wallet(),
            "player_ban",
            Some(&id.to_string()),
            serde_json::json!({ "banned": false }),
            serde_json::json!({ "banned": true, "reason": reason }),
            None,
        )
        .await?;
    }
    tx.commit().await?;
    let revoked = state.player_sessions.revoke_all(id).await?;
    tracing::warn!(actor = %admin.wallet(), player = %id, revoked, "admin_player_banned");
    Ok(Json(
        serde_json::json!({ "banned": true, "sessions_revoked": revoked }),
    ))
}

/// `POST /api/players/{id}/unban`
pub async fn unban(
    State(state): State<AdminState>,
    admin: ApiAdmin,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, ApiError> {
    admin.guard(&state, &headers).await?;
    let mut tx = state.pool.begin().await?;
    let before: Option<String> = sqlx::query_scalar(
        "SELECT ban_reason FROM cf.users WHERE id = $1 AND banned_at IS NOT NULL FOR UPDATE",
    )
    .bind(id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(ApiError(AdminError::NotFound))?;
    sqlx::query("UPDATE cf.users SET banned_at = NULL, ban_reason = NULL WHERE id = $1")
        .bind(id)
        .execute(&mut *tx)
        .await?;
    audit::record(
        &mut tx,
        admin.wallet(),
        "player_unban",
        Some(&id.to_string()),
        serde_json::json!({ "banned": true, "reason": before }),
        serde_json::json!({ "banned": false }),
        None,
    )
    .await?;
    tx.commit().await?;
    tracing::warn!(actor = %admin.wallet(), player = %id, "admin_player_unbanned");
    Ok(Json(serde_json::json!({ "banned": false })))
}
