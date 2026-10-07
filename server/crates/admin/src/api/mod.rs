//! JSON API of the admin app (ADR-022). Every route needs the admin session cookie;
//! every mutation also needs the `X-CSRF` header equal to the session's token, so a
//! cross-site page (which cannot read the token) can never act. Errors are
//! `{ "error": { "code", "message" } }`; internals stay in the logs.

pub mod alerts;
pub mod analytics;
pub mod audit;
pub mod dashboard;
pub mod economy;
pub mod exports;
pub mod ledger;
pub mod names;
pub mod orders;
pub mod players;
pub mod system;

use axum::extract::{FromRequestParts, State};
use axum::http::request::Parts;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use axum_extra::extract::cookie::{Cookie, CookieJar};
use serde::Deserialize;

use crate::error::AdminError;
use crate::metrics;
use crate::session::{Admin, COOKIE};
use crate::state::AdminState;

/// `AdminError` rendered as JSON instead of an HTML page or a redirect.
#[derive(Debug)]
pub struct ApiError(pub AdminError);

impl<E: Into<AdminError>> From<E> for ApiError {
    fn from(err: E) -> Self {
        Self(err.into())
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let status = self.0.status();
        let body =
            serde_json::json!({ "error": { "code": self.0.code(), "message": self.0.public_message() } });
        (status, Json(body)).into_response()
    }
}

/// The signed-in admin for API handlers: a missing or dead session is a JSON 401.
pub struct ApiAdmin(pub Admin);

impl std::fmt::Debug for ApiAdmin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_tuple("ApiAdmin").field(&self.0).finish()
    }
}

impl FromRequestParts<AdminState> for ApiAdmin {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &AdminState) -> Result<Self, Self::Rejection> {
        Admin::from_request_parts(parts, state)
            .await
            .map(ApiAdmin)
            .map_err(ApiError)
    }
}

/// Mutations per admin session per minute: an operator clicks, a script does not.
const MUTATIONS_PER_MIN: u64 = 60;
/// Full-table work (reconcile, CSV exports) per admin session per minute.
const HEAVY_PER_MIN: u64 = 5;

impl ApiAdmin {
    /// Every mutation passes here first: the `X-CSRF` header equals the session token
    /// (constant time), a present `Origin` is the admin's own, and the session stays
    /// under the mutation rate limit.
    pub async fn guard(&self, state: &AdminState, headers: &HeaderMap) -> Result<(), ApiError> {
        self.check_csrf(headers)?;
        check_origin(headers)?;
        state
            .limiter
            .hit_strict(
                "admin_mutation",
                &self.0.wallet,
                MUTATIONS_PER_MIN,
                60,
                crate::state::now_ms(),
            )
            .await
            .map_err(AdminError::from)?;
        Ok(())
    }

    /// Work that scans whole tables gets a limit of its own, so one tab cannot lean on
    /// the game's database.
    pub async fn heavy(&self, state: &AdminState) -> Result<(), ApiError> {
        state
            .limiter
            .hit_strict(
                "admin_heavy",
                &self.0.wallet,
                HEAVY_PER_MIN,
                60,
                crate::state::now_ms(),
            )
            .await
            .map_err(AdminError::from)?;
        Ok(())
    }

    /// Mutations carry the session token in `X-CSRF`; compared in constant time.
    pub fn check_csrf(&self, headers: &HeaderMap) -> Result<(), ApiError> {
        let given = headers.get("x-csrf").and_then(|v| v.to_str().ok()).unwrap_or("");
        self.0.check_csrf(given).map_err(ApiError)
    }

    pub fn wallet(&self) -> &str {
        &self.0.wallet
    }
}

/// Defence in depth next to the token: a browser that sends `Origin` must send ours
/// (`Host` is what the tunnel/loopback client typed; the scheme is not compared).
fn check_origin(headers: &HeaderMap) -> Result<(), ApiError> {
    let Some(origin) = headers.get("origin").and_then(|v| v.to_str().ok()) else {
        return Ok(());
    };
    let host = headers.get("host").and_then(|v| v.to_str().ok()).unwrap_or("");
    let origin_host = origin.split_once("://").map(|(_, h)| h).unwrap_or(origin);
    if origin_host.eq_ignore_ascii_case(host) {
        Ok(())
    } else {
        Err(ApiError(AdminError::Forbidden("origin mismatch")))
    }
}

/// `?page=&per_page=` with sane bounds.
#[derive(Debug, Deserialize, Default)]
pub struct PageQuery {
    pub page: Option<i64>,
    pub per_page: Option<i64>,
}

impl PageQuery {
    /// (limit, offset), page 1-based, 1..=100 rows, at most a million pages.
    pub fn bounds(&self) -> (i64, i64) {
        let per = self.per_page.unwrap_or(25).clamp(1, 100);
        let page = self.page.unwrap_or(1).clamp(1, 1_000_000);
        (per, (page - 1).saturating_mul(per))
    }
}

/// `%`/`_`-safe ILIKE pattern for a user-typed needle.
pub fn like_pattern(needle: &str) -> String {
    format!(
        "%{}%",
        needle
            .trim()
            .replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_")
    )
}

pub fn router() -> Router<AdminState> {
    Router::new()
        .route("/me", get(me))
        .route("/logout", post(logout))
        .route("/dashboard", get(dashboard::get))
        .route("/settings", get(economy::settings).post(economy::set_setting))
        .route("/catalog", get(economy::catalog))
        .route("/catalog/{id}", post(economy::update_item))
        .route("/players", get(players::list))
        .route("/players/{id}", get(players::detail))
        .route("/players/{id}/flag", post(players::flag))
        .route("/players/{id}/ban", post(players::ban))
        .route("/players/{id}/unban", post(players::unban))
        .route("/players/{id}/nickname-reset", post(names::reset))
        .route("/name-blocks", get(names::list).post(names::block))
        .route("/name-blocks/{id}/lift", post(names::lift))
        .route("/ledger", get(ledger::search))
        .route("/ledger/summary", get(ledger::summary))
        .route("/ledger/reconcile", post(ledger::reconcile))
        .route("/analytics", get(analytics::get))
        .route("/alerts", get(alerts::list))
        .route("/alerts/mute", post(alerts::mute))
        .route("/alerts/unmute", post(alerts::unmute))
        .route("/alerts/{id}/ack", post(alerts::ack))
        .route("/export/{file}", get(exports::get))
        .route("/orders", get(orders::list))
        .route("/orders/{id}/approve", post(orders::approve))
        .route("/orders/{id}/reject", post(orders::reject))
        .route("/audit", get(audit::list))
        .route("/system", get(system::get))
        .fallback(not_found)
        .method_not_allowed_fallback(not_found)
}

/// Unknown API paths answer 404 only to a signed-in admin; strangers get the same 401 as everywhere.
async fn not_found(_admin: ApiAdmin) -> ApiError {
    ApiError(AdminError::NotFound)
}

/// `GET /api/me`: who is signed in, the CSRF token for mutations, and the badge count.
async fn me(State(state): State<AdminState>, admin: ApiAdmin) -> Result<Json<serde_json::Value>, ApiError> {
    let review_queue = metrics::review_count(&state.pool).await?;
    Ok(Json(serde_json::json!({
        "wallet": admin.wallet(),
        "csrf": admin.0.csrf(),
        "env": format!("{:?}", state.config.env).to_lowercase(),
        "version": env!("CARGO_PKG_VERSION"),
        "review_queue": review_queue,
        "open_alerts": metrics::open_alerts(&state.pool).await?,
        "totp": state.config.totp_secret(admin.wallet()).is_some(),
    })))
}

/// `POST /api/logout`: revokes the session and clears the cookie.
async fn logout(
    State(state): State<AdminState>,
    admin: ApiAdmin,
    headers: HeaderMap,
    jar: CookieJar,
) -> Result<(CookieJar, StatusCode), ApiError> {
    admin.guard(&state, &headers).await?;
    state.sessions.revoke(&admin.0.token).await?;
    let mut gone = Cookie::new(COOKIE, "");
    gone.set_path("/");
    Ok((jar.remove(gone), StatusCode::NO_CONTENT))
}
