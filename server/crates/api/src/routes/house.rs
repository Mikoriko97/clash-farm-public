//! The island house (docs/API.md §Світ): sleeping for XP once a day.
//! Same shape as `routes::world`: session → rate limit → idempotency → cf-world.

use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::Response;
use cf_world::house;

use crate::error::ApiError;
use crate::idempotency::{self, IdemKey};
use crate::middleware::session::AuthUser;
use crate::routes::world::limit_world;
use crate::state::AppState;

/// `POST /house/sleep` → `{ xp_gain, xp, level }`; `429 daily_cap` after today's sleep.
pub async fn sleep(
    State(state): State<AppState>,
    headers: HeaderMap,
    user: AuthUser,
    key: IdemKey,
) -> Result<Response, ApiError> {
    limit_world(&state, &user, &headers).await?;
    let params = state.params.get(&state.pool).await?;
    let pool = state.pool.clone();
    idempotency::run(&state, user.user_id, key, "sleep", || async move {
        Ok(house::sleep(&pool, user.user_id, params).await?)
    })
    .await
}
