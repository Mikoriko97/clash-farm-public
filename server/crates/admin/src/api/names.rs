//! Nickname moderation (0031, ADR-040): reset a player's name, block names, lift blocks. The
//! rules live in `cf_world::moderation`; here — the admin session, CSRF, the mutation limit
//! (`guard`), the reason and the audit row, written in the same transaction as the change.

use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::Json;
use cf_world::moderation;
use cf_world::WorldError;
use serde::Deserialize;
use serde_json::Value;
use uuid::Uuid;

use crate::api::{like_pattern, ApiAdmin, ApiError};
use crate::audit;
use crate::error::AdminError;
use crate::state::AdminState;

/// Fresh placeholders tried before giving up (a collision is one in four billion).
const RESET_ATTEMPTS: usize = 3;

/// `nickname`: the name the admin saw on the card. Reset only if the player still carries it, so
/// a stale second click never hits the clean name the player has picked since.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResetBody {
    pub nickname: String,
    pub reason: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BlockBody {
    pub nickname: String,
    pub reason: String,
}

#[derive(Debug, Deserialize, Default)]
pub struct BlocksQuery {
    pub q: Option<String>,
}

fn reason_of(raw: &str) -> Result<String, ApiError> {
    let reason: String = raw.trim().chars().take(200).collect();
    if reason.is_empty() {
        return Err(ApiError(AdminError::BadRequest("причина обовʼязкова".to_owned())));
    }
    Ok(reason)
}

fn world(err: WorldError) -> ApiError {
    ApiError(match err {
        WorldError::NotFound => AdminError::NotFound,
        WorldError::NoCharacter => AdminError::Conflict("у гравця ще нема персонажа"),
        WorldError::WrongState => AdminError::Conflict("нік уже змінився — оновіть картку"),
        WorldError::InvalidNickname(why) => AdminError::BadRequest(format!("ім'я не підходить: {why}")),
        WorldError::NicknameNotAllowed => AdminError::Conflict("це ім'я вже забороняють правила гри"),
        WorldError::Db(e) => AdminError::Db(e),
        other => {
            tracing::error!(error = %other, "admin names: unexpected world error");
            AdminError::Conflict("не вдалося, спробуйте ще раз")
        }
    })
}

/// `POST /api/players/{id}/nickname-reset { nickname, reason }`: placeholder name, a free rename,
/// the old name blocked. A repeat answers the first reset and audits nothing.
pub async fn reset(
    State(state): State<AdminState>,
    admin: ApiAdmin,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(body): Json<ResetBody>,
) -> Result<Json<Value>, ApiError> {
    admin.guard(&state, &headers).await?;
    let reason = reason_of(&body.reason)?;
    let expected = body.nickname.trim();
    for _ in 0..RESET_ATTEMPTS {
        let mut tx = state.pool.begin().await?;
        let done = match moderation::reset_nickname(&mut tx, id, expected, &reason, admin.wallet()).await {
            Ok(done) => done,
            Err(WorldError::NicknameTaken) => continue,
            Err(e) => return Err(world(e)),
        };
        if !done.repeated {
            audit::record(
                &mut tx,
                admin.wallet(),
                "player_nickname_reset",
                Some(&id.to_string()),
                serde_json::json!({ "nickname": done.old }),
                serde_json::json!({ "nickname": done.new, "reason": reason }),
                None,
            )
            .await?;
        }
        tx.commit().await?;
        tracing::warn!(actor = %admin.wallet(), player = %id, repeated = done.repeated, "admin_nickname_reset");
        return Ok(Json(
            serde_json::json!({ "old": done.old, "new": done.new, "repeated": done.repeated }),
        ));
    }
    Err(ApiError(AdminError::Conflict(
        "не вдалося підібрати тимчасове ім'я, спробуйте ще раз",
    )))
}

/// `GET /api/name-blocks?q=`: live blocks first, then lifted ones; newest first, at most 200.
/// `holder` is the account carrying the name (or a lookalike) right now.
pub async fn list(
    State(state): State<AdminState>,
    _admin: ApiAdmin,
    Query(q): Query<BlocksQuery>,
) -> Result<Json<Value>, ApiError> {
    let needle =
        q.q.unwrap_or_default()
            .trim()
            .chars()
            .take(64)
            .collect::<String>();
    let key = cf_world::nickname::key_of(&needle);
    let rows: Option<Value> = sqlx::query_scalar(
        "SELECT coalesce(jsonb_agg(to_jsonb(t)), '[]'::jsonb) FROM (
            SELECT b.id, b.nickname, b.key, b.reason, b.blocked_by, b.created_at, b.lifted_at, b.lifted_by,
                   (SELECT u.id FROM cf.users u WHERE u.nickname_key = b.key) AS holder
            FROM cf.nickname_blocks b
            WHERE ($1 = '' OR b.nickname ILIKE $2 OR b.key = $3)
            ORDER BY b.lifted_at IS NOT NULL, b.created_at DESC
            LIMIT 200) t",
    )
    .bind(&needle)
    .bind(like_pattern(&needle))
    .bind(&key)
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(
        serde_json::json!({ "rows": rows.unwrap_or(Value::Array(Vec::new())) }),
    ))
}

/// `POST /api/name-blocks { nickname, reason }`: blocks a name (and its lookalikes) ahead of time.
pub async fn block(
    State(state): State<AdminState>,
    admin: ApiAdmin,
    headers: HeaderMap,
    Json(body): Json<BlockBody>,
) -> Result<Json<Value>, ApiError> {
    admin.guard(&state, &headers).await?;
    let reason = reason_of(&body.reason)?;
    let mut tx = state.pool.begin().await?;
    let made = moderation::block_name(&mut tx, &body.nickname, &reason, admin.wallet())
        .await
        .map_err(world)?
        .ok_or(ApiError(AdminError::Conflict("це ім'я вже заблоковане")))?;
    audit::record(
        &mut tx,
        admin.wallet(),
        "nickname_block",
        Some(&made.nickname),
        serde_json::json!({ "blocked": false }),
        serde_json::json!({ "blocked": true, "id": made.id, "key": made.key, "reason": reason }),
        None,
    )
    .await?;
    tx.commit().await?;
    tracing::warn!(actor = %admin.wallet(), block = made.id, "admin_nickname_block");
    Ok(Json(
        serde_json::json!({ "id": made.id, "nickname": made.nickname, "holder": made.holder }),
    ))
}

/// `POST /api/name-blocks/{id}/lift`: the name may be claimed again.
pub async fn lift(
    State(state): State<AdminState>,
    admin: ApiAdmin,
    headers: HeaderMap,
    Path(id): Path<i64>,
) -> Result<Json<Value>, ApiError> {
    admin.guard(&state, &headers).await?;
    let mut tx = state.pool.begin().await?;
    let nickname = moderation::lift_block(&mut tx, id, admin.wallet())
        .await
        .map_err(world)?;
    audit::record(
        &mut tx,
        admin.wallet(),
        "nickname_unblock",
        Some(&nickname),
        serde_json::json!({ "blocked": true, "id": id }),
        serde_json::json!({ "blocked": false, "id": id }),
        None,
    )
    .await?;
    tx.commit().await?;
    tracing::warn!(actor = %admin.wallet(), block = id, "admin_nickname_unblock");
    Ok(Json(serde_json::json!({ "lifted": true })))
}
