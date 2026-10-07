//! Selling, crafting and eating (migration 0008, docs/API.md §Світ).
//! Same shape as `routes::world`: session → rate limit → idempotency → cf-world.

use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::response::Response;
use axum::Json;
use cf_world::{craft, market};
use serde::Deserialize;

use crate::error::ApiError;
use crate::idempotency::{self, IdemKey};
use crate::middleware::session::AuthUser;
use crate::routes::world::limit_world;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct SellBody {
    pub item_id: String,
    pub qty: i32,
}

#[derive(Debug, Deserialize)]
pub struct CraftBody {
    pub recipe_id: String,
    #[serde(default = "one")]
    pub times: i32,
}

fn one() -> i32 {
    1
}

/// `GET /recipes` → `{ recipes: [...] }`.
pub async fn recipes(State(state): State<AppState>) -> Result<Json<serde_json::Value>, ApiError> {
    let recipes = craft::list(&state.pool).await?;
    Ok(Json(serde_json::json!({ "recipes": recipes })))
}

/// `POST /market/sell { item_id, qty }` → `{ item_id, qty_sold, credits_earned, credits }`.
pub async fn sell(
    State(state): State<AppState>,
    headers: HeaderMap,
    user: AuthUser,
    key: IdemKey,
    Json(body): Json<SellBody>,
) -> Result<Response, ApiError> {
    limit_world(&state, &user, &headers).await?;
    let params = state.params.get(&state.pool).await?;
    let pool = state.pool.clone();
    let scope = format!("sell:{}", body.item_id);
    idempotency::run(&state, user.user_id, key, &scope, || async move {
        Ok(market::sell(&pool, user.user_id, &body.item_id, body.qty, params).await?)
    })
    .await
}

/// `POST /craft { recipe_id, times? }` → `{ recipe_id, output_item, qty, xp, level }`.
pub async fn craft_one(
    State(state): State<AppState>,
    headers: HeaderMap,
    user: AuthUser,
    key: IdemKey,
    Json(body): Json<CraftBody>,
) -> Result<Response, ApiError> {
    limit_world(&state, &user, &headers).await?;
    let params = state.params.get(&state.pool).await?;
    let pool = state.pool.clone();
    let scope = format!("craft:{}", body.recipe_id);
    idempotency::run(&state, user.user_id, key, &scope, || async move {
        Ok(craft::craft(&pool, user.user_id, &body.recipe_id, body.times, params).await?)
    })
    .await
}

/// `POST /items/{id}/eat` → `{ item_id, xp, level }`.
pub async fn eat(
    State(state): State<AppState>,
    headers: HeaderMap,
    user: AuthUser,
    key: IdemKey,
    Path(item_id): Path<String>,
) -> Result<Response, ApiError> {
    limit_world(&state, &user, &headers).await?;
    let params = state.params.get(&state.pool).await?;
    let pool = state.pool.clone();
    let scope = format!("eat:{item_id}");
    idempotency::run(&state, user.user_id, key, &scope, || async move {
        Ok(craft::eat(&pool, user.user_id, &item_id, params).await?)
    })
    .await
}
