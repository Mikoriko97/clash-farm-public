//! Tools (migration 0013, docs/API.md §Tools): what the player holds and how
//! worn it is, and a repair for materials.

use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use axum::Json;
use cf_world::tools;

use crate::error::ApiError;
use crate::idempotency::{self, IdemKey};
use crate::middleware::session::AuthUser;
use crate::routes::world::limit_world;
use crate::state::AppState;

/// `GET /tools` → `{ tools: [{ item_id, qty, wear, durability, repair: [{ item_id, qty }] }] }`.
pub async fn list(State(state): State<AppState>, user: AuthUser) -> Result<Response, ApiError> {
    let params = state.params.get(&state.pool).await?;
    let view = tools::list(&state.pool, user.user_id, params).await?;
    Ok(Json(serde_json::json!({ "tools": view })).into_response())
}

/// `POST /tools/{id}/repair` → `{ tool }`; `404` without the tool, `409 wrong_state` when it is not worn.
pub async fn repair(
    State(state): State<AppState>,
    headers: HeaderMap,
    user: AuthUser,
    key: IdemKey,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    limit_world(&state, &user, &headers).await?;
    if !tools::TOOLS.contains(&id.as_str()) {
        return Err(ApiError::not_found("tool not found"));
    }
    let params = state.params.get(&state.pool).await?;
    let pool = state.pool.clone();
    let scope = format!("tool_repair:{id}");
    idempotency::run(&state, user.user_id, key, &scope, || async move {
        let tool = tools::repair(&pool, user.user_id, &id, params).await?;
        Ok(serde_json::json!({ "tool": tool }))
    })
    .await
}
