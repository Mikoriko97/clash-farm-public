//! Credits shop and decor placement (docs/API.md). Same discipline as plots:
//! session → per-user limit → Idempotency-Key → world layer with FOR UPDATE.

use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::Response;
use axum::Json;
use cf_world::{clearings, decor, shop};
use serde::Deserialize;
use uuid::Uuid;

use crate::error::ApiError;
use crate::idempotency::{self, IdemKey};
use crate::middleware::session::AuthUser;
use crate::routes::world::limit_world;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct BuyBody {
    pub item_id: String,
    pub qty: i32,
}

#[derive(Debug, Deserialize)]
pub struct PlaceBody {
    pub item_id: String,
    pub x_dm: i16,
    pub z_dm: i16,
    #[serde(default)]
    pub yaw_deg: i16,
}

#[derive(Debug, Deserialize)]
pub struct ClearBody {
    pub x_dm: i16,
    pub z_dm: i16,
}

#[derive(Debug, Deserialize)]
pub struct RemoveBody {
    pub id: Uuid,
}

/// `POST /shop/buy-credits { item_id, qty }`
pub async fn buy_credits(
    State(state): State<AppState>,
    headers: HeaderMap,
    user: AuthUser,
    key: IdemKey,
    Json(body): Json<BuyBody>,
) -> Result<Response, ApiError> {
    limit_world(&state, &user, &headers).await?;
    let params = state.params.get(&state.pool).await?;
    let pool = state.pool.clone();
    let scope = format!("buy:{}:{}", body.item_id, body.qty);
    idempotency::run(&state, user.user_id, key, &scope, || async move {
        Ok(shop::buy_credits(&pool, user.user_id, &body.item_id, body.qty, params).await?)
    })
    .await
}

/// `POST /decor/place { item_id, x_dm, z_dm, yaw_deg }`
pub async fn place(
    State(state): State<AppState>,
    headers: HeaderMap,
    user: AuthUser,
    key: IdemKey,
    Json(body): Json<PlaceBody>,
) -> Result<Response, ApiError> {
    limit_world(&state, &user, &headers).await?;
    let params = state.params.get(&state.pool).await?;
    let pool = state.pool.clone();
    let scope = format!(
        "decor_place:{}:{}:{}:{}",
        body.item_id, body.x_dm, body.z_dm, body.yaw_deg
    );
    idempotency::run(&state, user.user_id, key, &scope, || async move {
        Ok(decor::place(
            &pool,
            user.user_id,
            &body.item_id,
            body.x_dm,
            body.z_dm,
            body.yaw_deg,
            params,
        )
        .await?)
    })
    .await
}

/// `POST /decor/remove { id }`
pub async fn remove(
    State(state): State<AppState>,
    headers: HeaderMap,
    user: AuthUser,
    key: IdemKey,
    Json(body): Json<RemoveBody>,
) -> Result<Response, ApiError> {
    limit_world(&state, &user, &headers).await?;
    let pool = state.pool.clone();
    let scope = format!("decor_remove:{}", body.id);
    idempotency::run(&state, user.user_id, key, &scope, || async move {
        Ok(decor::remove(&pool, user.user_id, body.id).await?)
    })
    .await
}

/// `POST /island/clear { x_dm, z_dm }`: the player clears a wild bush by hand — the server
/// remembers the spot (no tool, no cost, no reward) and answers with every cleared spot.
pub async fn clear(
    State(state): State<AppState>,
    headers: HeaderMap,
    user: AuthUser,
    key: IdemKey,
    Json(body): Json<ClearBody>,
) -> Result<Response, ApiError> {
    limit_world(&state, &user, &headers).await?;
    let pool = state.pool.clone();
    let scope = format!("island_clear:{}:{}", body.x_dm, body.z_dm);
    idempotency::run(&state, user.user_id, key, &scope, || async move {
        let cleared = clearings::clear(&pool, user.user_id, body.x_dm, body.z_dm).await?;
        Ok(serde_json::json!({ "cleared": cleared }))
    })
    .await
}
