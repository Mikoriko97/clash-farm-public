//! `/healthz` (process alive, no dependencies) and `/readyz` (Postgres + Redis).

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde_json::{json, Value};

use crate::state::AppState;

pub async fn healthz() -> &'static str {
    "ok"
}

pub async fn readyz(State(state): State<AppState>) -> (StatusCode, Json<Value>) {
    let db = sqlx::query("SELECT 1").execute(&state.pool).await.is_ok();
    let mut con = state.redis.clone();
    let redis = redis::cmd("PING").query_async::<String>(&mut con).await.is_ok();
    let status = if db && redis {
        StatusCode::OK
    } else {
        StatusCode::SERVICE_UNAVAILABLE
    };
    (status, Json(json!({ "db": db, "redis": redis })))
}
