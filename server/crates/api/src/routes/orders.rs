//! CLASH orders (docs/API.md §Payments). The server only quotes and verifies;
//! the transaction is built and signed in the player's wallet.

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::Json;
use cf_payments::{orders, SubmitOutcome};
use serde::Deserialize;
use sqlx::Row;
use uuid::Uuid;

use crate::error::ApiError;
use crate::idempotency::{self, IdemKey};
use crate::middleware::now_ms;
use crate::middleware::session::AuthUser;
use crate::routes::world::limit_world;
use crate::state::AppState;

/// Each submit may cost up to 2 providers x 3 retries of paid RPC: keep it well under the world limit.
const ORDERS_LIMIT_PER_MIN: u64 = 30;

async fn limit_orders(state: &AppState, user: &AuthUser, headers: &HeaderMap) -> Result<(), ApiError> {
    limit_world(state, user, headers).await?;
    state
        .limiter
        .hit(
            "orders_user",
            &user.user_id.to_string(),
            ORDERS_LIMIT_PER_MIN,
            60,
            now_ms(),
        )
        .await?;
    Ok(())
}

#[derive(Debug, Deserialize)]
pub struct CreateBody {
    pub item_id: String,
    pub qty: i32,
}

#[derive(Deserialize)]
pub struct SubmitBody {
    pub signature: String,
}

impl std::fmt::Debug for SubmitBody {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SubmitBody").finish_non_exhaustive()
    }
}

/// `POST /orders { item_id, qty }` → quote.
pub async fn create(
    State(state): State<AppState>,
    headers: HeaderMap,
    user: AuthUser,
    key: IdemKey,
    Json(body): Json<CreateBody>,
) -> Result<Response, ApiError> {
    limit_orders(&state, &user, &headers).await?;
    let runtime = state.payments.clone().ok_or_else(payments_disabled)?;
    let params = state.params.get(&state.pool).await?;
    let pool = state.pool.clone();
    let wallet: Option<String> = sqlx::query("SELECT wallet FROM cf.users WHERE id = $1")
        .bind(user.user_id)
        .fetch_one(&pool)
        .await
        .and_then(|r| r.try_get("wallet"))
        .map_err(cf_auth::AuthError::from)?;
    // A guest (0028) has no wallet to pay from: link one first (POST /auth/link).
    let wallet = wallet.ok_or_else(|| {
        ApiError::new(
            StatusCode::FORBIDDEN,
            "wallet_required",
            "link a wallet to pay with CLASH",
        )
    })?;
    let scope = format!("order:{}:{}", body.item_id, body.qty);
    idempotency::run(&state, user.user_id, key, &scope, || async move {
        Ok(orders::create(
            &pool,
            &runtime,
            user.user_id,
            &wallet,
            &body.item_id,
            body.qty,
            params,
        )
        .await?)
    })
    .await
}

/// `POST /orders/{id}/submit { signature }` → 200 with the final status, 202 while pending.
pub async fn submit(
    State(state): State<AppState>,
    headers: HeaderMap,
    user: AuthUser,
    Path(order_id): Path<Uuid>,
    Json(body): Json<SubmitBody>,
) -> Result<Response, ApiError> {
    limit_orders(&state, &user, &headers).await?;
    let runtime = state.payments.clone().ok_or_else(payments_disabled)?;
    let params = state.params.get(&state.pool).await?;
    let outcome = cf_payments::submit::submit(
        &state.pool,
        &runtime,
        user.user_id,
        order_id,
        &body.signature,
        params,
    )
    .await?;
    let status = match outcome {
        SubmitOutcome::Submitted { .. } => StatusCode::ACCEPTED,
        _ => StatusCode::OK,
    };
    Ok((status, Json(outcome)).into_response())
}

/// `GET /orders/{id}`
pub async fn status(
    State(state): State<AppState>,
    user: AuthUser,
    Path(order_id): Path<Uuid>,
) -> Result<Json<orders::OrderStatusView>, ApiError> {
    Ok(Json(orders::status(&state.pool, user.user_id, order_id).await?))
}

fn payments_disabled() -> ApiError {
    ApiError::new(StatusCode::CONFLICT, "payments_disabled", "payments are disabled")
}
