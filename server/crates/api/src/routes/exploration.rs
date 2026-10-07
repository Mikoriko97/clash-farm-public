//! Exploration and expeditions (migrations 0014/0015, docs/API.md §Mainland).
//! Every position comes from the presence socket (`mainland_position`), never
//! from the request body.

use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use axum::Json;
use cf_world::{expeditions, exploration};
use serde::Deserialize;

use crate::error::ApiError;
use crate::idempotency::{self, IdemKey};
use crate::middleware::session::AuthUser;
use crate::routes::mainland::mainland_position;
use crate::routes::world::limit_world;
use crate::state::AppState;

/// `GET /mainland/explored` → `{ mask_hex, explored, landmarks, total_landmarks }`.
pub async fn explored(State(state): State<AppState>, user: AuthUser) -> Result<Response, ApiError> {
    let view = exploration::view(&state.pool, user.user_id).await?;
    Ok(Json(view).into_response())
}

/// `POST /mainland/explore` → `{ view, revealed, xp, level }`; the chunk is the character's.
pub async fn explore(
    State(state): State<AppState>,
    headers: HeaderMap,
    user: AuthUser,
    key: IdemKey,
) -> Result<Response, ApiError> {
    limit_world(&state, &user, &headers).await?;
    let pool = state.pool.clone();
    let redis = state.redis.clone();
    idempotency::run(&state, user.user_id, key, "explore", || async move {
        let pos = mainland_position(&redis, user.user_id).await?;
        Ok(exploration::explore(&pool, user.user_id, pos.x_dm, pos.z_dm).await?)
    })
    .await
}

#[derive(Debug, Deserialize)]
pub struct DiscoverBody {
    pub landmark_id: String,
}

/// `POST /mainland/discover { landmark_id }` → `{ landmark, reward, xp, level }`.
pub async fn discover(
    State(state): State<AppState>,
    headers: HeaderMap,
    user: AuthUser,
    key: IdemKey,
    Json(body): Json<DiscoverBody>,
) -> Result<Response, ApiError> {
    limit_world(&state, &user, &headers).await?;
    if body.landmark_id.len() > 32
        || !body
            .landmark_id
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
    {
        return Err(ApiError::not_found("landmark not found"));
    }
    let pool = state.pool.clone();
    let redis = state.redis.clone();
    let scope = format!("discover:{}", body.landmark_id);
    idempotency::run(&state, user.user_id, key, &scope, || async move {
        let pos = mainland_position(&redis, user.user_id).await?;
        Ok(exploration::discover(&pool, user.user_id, &body.landmark_id, pos.x_dm, pos.z_dm).await?)
    })
    .await
}

/// `GET /mainland/expeditions` → `{ day, tasks: [{ index, kind, target, count, xp, progress, claimed }] }`.
pub async fn expeditions_today(State(state): State<AppState>, user: AuthUser) -> Result<Response, ApiError> {
    let view = expeditions::view(&state.pool, user.user_id).await?;
    Ok(Json(view).into_response())
}

/// `POST /mainland/expeditions/{n}/claim` → `{ task, xp, level }`.
pub async fn claim(
    State(state): State<AppState>,
    headers: HeaderMap,
    user: AuthUser,
    key: IdemKey,
    Path(n): Path<usize>,
) -> Result<Response, ApiError> {
    limit_world(&state, &user, &headers).await?;
    if n >= expeditions::TASKS_PER_DAY {
        return Err(ApiError::not_found("no such task"));
    }
    let pool = state.pool.clone();
    let scope = format!("expedition_claim:{n}");
    idempotency::run(&state, user.user_id, key, &scope, || async move {
        Ok(expeditions::claim(&pool, user.user_id, n).await?)
    })
    .await
}
