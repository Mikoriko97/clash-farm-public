//! `Idempotency-Key` for mutating game endpoints (docs/API.md): the same key
//! from the same user within 24 h returns the stored response without
//! re-running the action. The key is bound to a `scope` (route + target); the
//! same key on another route answers 422. A key still in flight answers 409.
//! Failed actions are not stored, so a corrected retry with the same key works.
//! If the final Redis write fails after the DB commit, the client sees 500 and
//! the pending marker expires within 60 s; a retry then re-runs the action,
//! which the world layer rejects safely (e.g. `wrong_state`).

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::error::ApiError;
use crate::state::AppState;

const PENDING: &str = "pending";
const PENDING_TTL_SECS: u64 = 60;
const STORED_TTL_SECS: u64 = 24 * 60 * 60;

/// Extractor: required `Idempotency-Key` header holding a UUID.
#[derive(Debug, Clone)]
pub struct IdemKey(pub Uuid);

impl FromRequestParts<AppState> for IdemKey {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, _state: &AppState) -> Result<Self, Self::Rejection> {
        parts
            .headers
            .get("idempotency-key")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| Uuid::parse_str(v).ok())
            .map(IdemKey)
            .ok_or_else(|| {
                ApiError::bad_request(
                    "idempotency_key_required",
                    "Idempotency-Key header (uuid) is required",
                )
            })
    }
}

#[derive(Debug, Serialize, Deserialize)]
struct Stored {
    scope: String,
    body: serde_json::Value,
}

/// Runs `action` once per (user, key); replays the stored JSON afterwards.
pub async fn run<T, F, Fut>(
    state: &AppState,
    user_id: Uuid,
    key: IdemKey,
    scope: &str,
    action: F,
) -> Result<Response, ApiError>
where
    T: Serialize,
    F: FnOnce() -> Fut,
    Fut: std::future::Future<Output = Result<T, ApiError>>,
{
    let redis_key = format!("idem:{user_id}:{}", key.0);
    let mut con = state.redis.clone();
    let claimed: Option<String> = redis::cmd("SET")
        .arg(&redis_key)
        .arg(PENDING)
        .arg("NX")
        .arg("EX")
        .arg(PENDING_TTL_SECS)
        .query_async(&mut con)
        .await
        .map_err(cf_auth::AuthError::from)?;
    if claimed.is_none() {
        let stored: Option<String> = redis::cmd("GET")
            .arg(&redis_key)
            .query_async(&mut con)
            .await
            .map_err(cf_auth::AuthError::from)?;
        return match stored.as_deref() {
            Some(PENDING) | None => Err(ApiError::new(
                StatusCode::CONFLICT,
                "in_progress",
                "same request is still being processed",
            )),
            Some(json) => replay(json, scope),
        };
    }
    match action().await {
        Ok(value) => {
            let body = serde_json::to_value(&value).map_err(|_| ApiError::internal())?;
            let stored = Stored {
                scope: scope.to_owned(),
                body,
            };
            let json = serde_json::to_string(&stored).map_err(|_| ApiError::internal())?;
            let _: () = redis::cmd("SET")
                .arg(&redis_key)
                .arg(&json)
                .arg("EX")
                .arg(STORED_TTL_SECS)
                .query_async(&mut con)
                .await
                .map_err(cf_auth::AuthError::from)?;
            Ok(Json(stored.body).into_response())
        }
        Err(err) => {
            let _: Result<(), _> = redis::cmd("DEL")
                .arg(&redis_key)
                .query_async::<()>(&mut con)
                .await;
            Err(err)
        }
    }
}

fn replay(json: &str, scope: &str) -> Result<Response, ApiError> {
    let stored: Stored = serde_json::from_str(json).map_err(|_| ApiError::internal())?;
    if stored.scope != scope {
        return Err(ApiError::new(
            StatusCode::UNPROCESSABLE_ENTITY,
            "idempotency_mismatch",
            "Idempotency-Key was already used for a different request",
        ));
    }
    Ok(Json(stored.body).into_response())
}
