//! Resource gathering (migration 0007, docs/API.md §Світ): one hit per call.
//! Same shape as `routes::world`: session → rate limit → idempotency → cf-world.

use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::response::Response;
use cf_world::resources;

use crate::error::ApiError;
use crate::idempotency::{self, IdemKey};
use crate::middleware::session::AuthUser;
use crate::routes::world::limit_world;
use crate::state::AppState;

/// `POST /resources/{idx}/gather` → `{ node, item_id, qty, xp, level }`.
pub async fn gather(
    State(state): State<AppState>,
    headers: HeaderMap,
    user: AuthUser,
    key: IdemKey,
    Path(idx): Path<i16>,
) -> Result<Response, ApiError> {
    limit_world(&state, &user, &headers).await?;
    let params = state.params.get(&state.pool).await?;
    let pool = state.pool.clone();
    let scope = format!("gather:{idx}");
    idempotency::run(&state, user.user_id, key, &scope, || async move {
        Ok(resources::gather(&pool, user.user_id, idx, params).await?)
    })
    .await
}
