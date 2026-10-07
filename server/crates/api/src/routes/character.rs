//! Character creation at the first login (0025, ADR-025, docs/API.md): the nickname and the
//! look, chosen once; later a new look costs a restyle ticket bought for CLASH (0029, ADR-038). The account comes from the session only. Creating is idempotent per
//! `Idempotency-Key` (a retry after a lost answer returns the first answer, not
//! `character_exists`); the live name check counts against a daily cap in the world layer.

use axum::extract::{Query, State};
use axum::http::HeaderMap;
use axum::response::Response;
use axum::Json;
use cf_world::character::{self, LookRequest, NickCheck};
use cf_world::{rename, restyle};
use serde::Deserialize;

use crate::error::ApiError;
use crate::idempotency::{self, IdemKey};
use crate::middleware::session::AuthUser;
use crate::routes::world::limit_world;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CreateBody {
    pub nickname: String,
    pub look: LookRequest,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LookBody {
    pub look: LookRequest,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NicknameBody {
    pub nickname: String,
}

#[derive(Debug, Deserialize)]
pub struct CheckQuery {
    pub nick: String,
}

/// `POST /me/character { nickname, look }` → `{ nickname, look }`.
pub async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    user: AuthUser,
    key: IdemKey,
    Json(body): Json<CreateBody>,
) -> Result<Response, ApiError> {
    limit_world(&state, &user, &headers).await?;
    let pool = state.pool.clone();
    idempotency::run(&state, user.user_id, key, "character_create", || async move {
        Ok(character::create(&pool, user.user_id, &body.nickname, body.look.into()).await?)
    })
    .await
}

/// `POST /me/look { look }` → `{ nickname, look }`; spends one `restyle_ticket`. A replay with
/// the same `Idempotency-Key` returns the first answer and spends nothing more.
pub async fn change_look(
    State(state): State<AppState>,
    headers: HeaderMap,
    user: AuthUser,
    key: IdemKey,
    Json(body): Json<LookBody>,
) -> Result<Response, ApiError> {
    limit_world(&state, &user, &headers).await?;
    let pool = state.pool.clone();
    idempotency::run(&state, user.user_id, key, "restyle", || async move {
        Ok(restyle::restyle(&pool, user.user_id, body.look.into()).await?)
    })
    .await
}

/// `POST /me/nickname { nickname }` → `{ nickname, look }`; spends one `rename_ticket` (0030,
/// ADR-039). A replay with the same `Idempotency-Key` returns the first answer, spends nothing more.
pub async fn change_nickname(
    State(state): State<AppState>,
    headers: HeaderMap,
    user: AuthUser,
    key: IdemKey,
    Json(body): Json<NicknameBody>,
) -> Result<Response, ApiError> {
    limit_world(&state, &user, &headers).await?;
    let pool = state.pool.clone();
    idempotency::run(&state, user.user_id, key, "rename", || async move {
        Ok(rename::rename(&pool, user.user_id, &body.nickname).await?)
    })
    .await
}

/// `GET /me/nickname-check?nick=` → `{ available, reason?, message? }` with `reason` one of
/// `invalid` (message says why), `not_allowed`, `taken`.
pub async fn nickname_check(
    State(state): State<AppState>,
    headers: HeaderMap,
    user: AuthUser,
    Query(query): Query<CheckQuery>,
) -> Result<Json<serde_json::Value>, ApiError> {
    limit_world(&state, &user, &headers).await?;
    let verdict = rename::check(&state.pool, user.user_id, &query.nick).await?;
    let (reason, message) = match verdict {
        NickCheck::Available => {
            return Ok(Json(serde_json::json!({ "available": true })));
        }
        NickCheck::Invalid(why) => ("invalid", why),
        NickCheck::NotAllowed => ("not_allowed", "that name is not allowed"),
        NickCheck::Taken => ("taken", "that name is taken"),
    };
    Ok(Json(
        serde_json::json!({ "available": false, "reason": reason, "message": message }),
    ))
}
