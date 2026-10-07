//! Shared weather and lightning (docs/API.md §Світ).
//! Same shape as `routes::world`: session → rate limit → idempotency → cf-world.

use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use axum::Json;
use cf_world::weather;

use crate::error::ApiError;
use crate::idempotency::{self, IdemKey};
use crate::middleware::session::AuthUser;
use crate::routes::world::limit_world;
use crate::state::AppState;

/// `GET /weather` → `{ state, slot, since, until }` from the database clock.
pub async fn current(State(state): State<AppState>, _user: AuthUser) -> Result<Response, ApiError> {
    let params = state.params.get(&state.pool).await?;
    let view = weather::current(&state.pool, params).await?;
    Ok(Json(view).into_response())
}

/// `POST /weather/lightning` → `{ struck, slot, resource_idx, item_id, qty }`; `409 no_storm` outside a storm.
pub async fn lightning(
    State(state): State<AppState>,
    headers: HeaderMap,
    user: AuthUser,
    key: IdemKey,
) -> Result<Response, ApiError> {
    limit_world(&state, &user, &headers).await?;
    let params = state.params.get(&state.pool).await?;
    let pool = state.pool.clone();
    idempotency::run(&state, user.user_id, key, "lightning", || async move {
        Ok(weather::lightning(&pool, user.user_id, params).await?)
    })
    .await
}
