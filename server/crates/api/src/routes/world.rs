//! Island, plots and inventory (docs/API.md §Світ). Every handler:
//! session → user_id → per-user rate limit → ownership in SQL (FOR UPDATE) → caps → action.

use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::response::Response;
use axum::Json;
use cf_world::{inventory, island, plots, stipend, IslandSnapshot};
use serde::Deserialize;
use uuid::Uuid;

use crate::error::ApiError;
use crate::idempotency::{self, IdemKey};
use crate::middleware::session::AuthUser;
use crate::middleware::{client_ip, now_ms};
use crate::state::AppState;

/// Burst protection for mutating world calls (daily caps are the economic limit).
const WORLD_LIMIT_PER_MIN: u64 = 60;

#[derive(Debug, Deserialize)]
pub struct ItemBody {
    pub item_id: String,
}

pub(crate) async fn limit_world(
    state: &AppState,
    user: &AuthUser,
    headers: &HeaderMap,
) -> Result<(), ApiError> {
    let limiter = &state.limiter;
    limiter
        .hit(
            "world_user",
            &user.user_id.to_string(),
            WORLD_LIMIT_PER_MIN,
            60,
            now_ms(),
        )
        .await?;
    limiter
        .hit(
            "world_ip",
            &client_ip(headers),
            WORLD_LIMIT_PER_MIN * 4,
            60,
            now_ms(),
        )
        .await?;
    Ok(())
}

/// `GET /island` — creates the island on first call; a stuck island gets its seed stipend
/// (0019) before the snapshot is taken, under the world rate limit like every action.
pub async fn island(
    State(state): State<AppState>,
    headers: HeaderMap,
    user: AuthUser,
) -> Result<Json<IslandSnapshot>, ApiError> {
    limit_world(&state, &user, &headers).await?;
    let params = state.params.get(&state.pool).await?;
    let mut snapshot = island::get_or_create(&state.pool, user.user_id, params).await?;
    if stipend::grant_if_stuck(&state.pool, user.user_id, params).await? > 0 {
        snapshot = island::snapshot(&state.pool, user.user_id, params).await?;
    }
    Ok(Json(snapshot))
}

/// `POST /plots/{id}/plant { item_id }`
pub async fn plant(
    State(state): State<AppState>,
    headers: HeaderMap,
    user: AuthUser,
    key: IdemKey,
    Path(plot_id): Path<Uuid>,
    Json(body): Json<ItemBody>,
) -> Result<Response, ApiError> {
    limit_world(&state, &user, &headers).await?;
    let params = state.params.get(&state.pool).await?;
    let pool = state.pool.clone();
    let scope = format!("plant:{plot_id}");
    idempotency::run(&state, user.user_id, key, &scope, || async move {
        Ok(plots::plant(&pool, user.user_id, plot_id, &body.item_id, params).await?)
    })
    .await
}

/// `POST /plots/{id}/fertilize { item_id }`
pub async fn fertilize(
    State(state): State<AppState>,
    headers: HeaderMap,
    user: AuthUser,
    key: IdemKey,
    Path(plot_id): Path<Uuid>,
    Json(body): Json<ItemBody>,
) -> Result<Response, ApiError> {
    limit_world(&state, &user, &headers).await?;
    let params = state.params.get(&state.pool).await?;
    let pool = state.pool.clone();
    let scope = format!("fertilize:{plot_id}");
    idempotency::run(&state, user.user_id, key, &scope, || async move {
        Ok(plots::fertilize(&pool, user.user_id, plot_id, &body.item_id, params).await?)
    })
    .await
}

/// `POST /plots/{id}/harvest`
pub async fn harvest(
    State(state): State<AppState>,
    headers: HeaderMap,
    user: AuthUser,
    key: IdemKey,
    Path(plot_id): Path<Uuid>,
) -> Result<Response, ApiError> {
    limit_world(&state, &user, &headers).await?;
    let params = state.params.get(&state.pool).await?;
    let pool = state.pool.clone();
    let scope = format!("harvest:{plot_id}");
    idempotency::run(&state, user.user_id, key, &scope, || async move {
        Ok(plots::harvest(&pool, user.user_id, plot_id, params).await?)
    })
    .await
}

#[derive(Debug, Deserialize)]
pub struct ExpandBody {
    pub side: String,
}

/// `POST /island/expand { side }` → `{ island, side, level, permit_used }` (ADR-015).
pub async fn expand(
    State(state): State<AppState>,
    headers: HeaderMap,
    user: AuthUser,
    key: IdemKey,
    Json(body): Json<ExpandBody>,
) -> Result<Response, ApiError> {
    limit_world(&state, &user, &headers).await?;
    let pool = state.pool.clone();
    let scope = format!("island_expand:{}", body.side);
    idempotency::run(&state, user.user_id, key, &scope, || async move {
        Ok(cf_world::expand::expand(&pool, user.user_id, &body.side).await?)
    })
    .await
}

/// `GET /inventory`
pub async fn inventory(
    State(state): State<AppState>,
    user: AuthUser,
) -> Result<Json<serde_json::Value>, ApiError> {
    let items = inventory::list(&state.pool, user.user_id).await?;
    Ok(Json(serde_json::json!({ "items": items })))
}

/// `GET /catalog`
pub async fn catalog(State(state): State<AppState>) -> Result<Json<serde_json::Value>, ApiError> {
    let items = cf_world::catalog::list_enabled(&state.pool).await?;
    let fake = state.payments.as_ref().is_some_and(|p| p.fake);
    Ok(Json(serde_json::json!({
        "items": items,
        "payments": { "available": state.payments.is_some(), "fake": fake },
    })))
}
