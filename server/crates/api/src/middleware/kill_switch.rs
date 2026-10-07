//! Kill switches from `settings` (RUNBOOK §7), read at most every 5 seconds:
//! `maintenance` → 503 for everything under /api; `shop_enabled=false` → 409
//! for the shop and new quotes; `payments_enabled=false` (or a paused oracle) → 409
//! for new quotes (`POST /orders`; submit and status of a signed order always pass); `renames_enabled=false` → 409 for `POST /me/nickname` (0030).

use std::time::{Duration, Instant};

use axum::extract::{Request, State};
use axum::http::StatusCode;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use sqlx::Row;

use crate::error::ApiError;
use crate::state::{AppState, SwitchCache};

const CACHE_TTL: Duration = Duration::from_secs(5);

pub async fn maintenance(State(state): State<AppState>, request: Request, next: Next) -> Response {
    let switches = match load(&state).await {
        Ok(s) => s,
        Err(err) => {
            tracing::error!(error = %err, "settings lookup failed");
            return ApiError::unavailable().into_response();
        }
    };
    if switches.maintenance {
        return ApiError::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "maintenance",
            "maintenance in progress",
        )
        .into_response();
    }
    let path = request.uri().path();
    let is_shop = path.starts_with("/shop/");
    // Only a NEW quote stops: `/orders/{id}/submit` verifies a transaction the player has already
    // signed (and burned) — refusing it would leave burned CLASH with no item and no trace — and
    // `GET /orders/{id}` only reads. Neither needs the oracle.
    let is_new_order = path == "/orders" && request.method() == axum::http::Method::POST;
    if (is_shop || is_new_order) && !switches.shop_enabled {
        return ApiError::new(StatusCode::CONFLICT, "shop_disabled", "shop is paused").into_response();
    }
    if path == "/me/nickname" && request.method() == axum::http::Method::POST && !switches.renames_enabled {
        return ApiError::new(
            StatusCode::CONFLICT,
            "renames_disabled",
            "name changes are paused",
        )
        .into_response();
    }
    if is_new_order {
        let oracle_paused = state
            .payments
            .as_ref()
            .is_some_and(|p| p.oracle.paused().is_some());
        if !switches.payments_enabled || oracle_paused {
            return ApiError::new(StatusCode::CONFLICT, "payments_disabled", "payments are paused")
                .into_response();
        }
    }
    next.run(request).await
}

async fn load(state: &AppState) -> Result<SwitchCache, sqlx::Error> {
    if let Some(cache) = *state.switches.read().await {
        if cache.checked_at.elapsed() < CACHE_TTL {
            return Ok(cache);
        }
    }
    let rows = sqlx::query(
        "SELECT key, value FROM cf.settings WHERE key IN ('maintenance', 'shop_enabled', 'payments_enabled', 'renames_enabled')",
    )
    .fetch_all(&state.pool)
    .await?;
    let mut cache = SwitchCache {
        checked_at: Instant::now(),
        maintenance: false,
        shop_enabled: true,
        payments_enabled: true,
        renames_enabled: true,
    };
    for row in rows {
        let key: String = row.try_get("key")?;
        let value = row.try_get::<serde_json::Value, _>("value")?.as_bool();
        // Garbage in `settings` fails closed: maintenance on, shop/payments off.
        match key.as_str() {
            "maintenance" => cache.maintenance = value.unwrap_or(true),
            "shop_enabled" => cache.shop_enabled = value.unwrap_or(false),
            "payments_enabled" => cache.payments_enabled = value.unwrap_or(false),
            "renames_enabled" => cache.renames_enabled = value.unwrap_or(false),
            _ => {}
        }
    }
    *state.switches.write().await = Some(cache);
    Ok(cache)
}
