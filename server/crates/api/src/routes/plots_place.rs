//! Player-placed plots and bed actions (migration 0006, docs/API.md §Світ).
//! Same shape as `routes::world`: session → rate limit → idempotency → cf-world.

use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::response::Response;
use axum::Json;
use cf_world::beds;
use cf_world::placement::{self, Cell};
use serde::Deserialize;
use uuid::Uuid;

use crate::error::ApiError;
use crate::idempotency::{self, IdemKey};
use crate::middleware::session::AuthUser;
use crate::routes::world::{limit_world, ItemBody};
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct CellBody {
    pub col: i16,
    pub row: i16,
}

/// `POST /plots/place { col, row }` → the new plot.
pub async fn place(
    State(state): State<AppState>,
    headers: HeaderMap,
    user: AuthUser,
    key: IdemKey,
    Json(body): Json<CellBody>,
) -> Result<Response, ApiError> {
    limit_world(&state, &user, &headers).await?;
    let params = state.params.get(&state.pool).await?;
    let pool = state.pool.clone();
    let cell = Cell {
        col: body.col,
        row: body.row,
    };
    let scope = format!("plot_place:{}:{}", cell.col, cell.row);
    idempotency::run(&state, user.user_id, key, &scope, || async move {
        Ok(placement::place(&pool, user.user_id, cell, params).await?)
    })
    .await
}

/// `POST /plots/{id}/remove` → `{ removed: id }`; only an empty plot.
pub async fn remove(
    State(state): State<AppState>,
    headers: HeaderMap,
    user: AuthUser,
    key: IdemKey,
    Path(plot_id): Path<Uuid>,
) -> Result<Response, ApiError> {
    limit_world(&state, &user, &headers).await?;
    let pool = state.pool.clone();
    let scope = format!("plot_remove:{plot_id}");
    idempotency::run(&state, user.user_id, key, &scope, || async move {
        placement::remove(&pool, user.user_id, plot_id).await?;
        Ok(serde_json::json!({ "removed": plot_id }))
    })
    .await
}

/// `POST /plots/{id}/bed/plant { item_id }` → `{ plots, count }`.
pub async fn bed_plant(
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
    let scope = format!("bed_plant:{plot_id}");
    idempotency::run(&state, user.user_id, key, &scope, || async move {
        Ok(beds::plant(&pool, user.user_id, plot_id, &body.item_id, params).await?)
    })
    .await
}

/// `POST /plots/{id}/bed/fertilize { item_id }` → `{ plots, count }`.
pub async fn bed_fertilize(
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
    let scope = format!("bed_fertilize:{plot_id}");
    idempotency::run(&state, user.user_id, key, &scope, || async move {
        Ok(beds::fertilize(&pool, user.user_id, plot_id, &body.item_id, params).await?)
    })
    .await
}

/// `POST /plots/{id}/bed/harvest` → `{ plots, count, xp, level, reward_credits, credits }`.
pub async fn bed_harvest(
    State(state): State<AppState>,
    headers: HeaderMap,
    user: AuthUser,
    key: IdemKey,
    Path(plot_id): Path<Uuid>,
) -> Result<Response, ApiError> {
    limit_world(&state, &user, &headers).await?;
    let params = state.params.get(&state.pool).await?;
    let pool = state.pool.clone();
    let scope = format!("bed_harvest:{plot_id}");
    idempotency::run(&state, user.user_id, key, &scope, || async move {
        Ok(beds::harvest(&pool, user.user_id, plot_id, params).await?)
    })
    .await
}
