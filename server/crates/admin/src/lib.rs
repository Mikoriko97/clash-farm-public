//! Clash Farm `cf-admin`: a separate binary on a loopback port (docs/SECURITY.md
//! §8, docs/RUNBOOK.md §7). Sign in with an allowlisted admin wallet, flip kill
//! switches, tune `settings`, review parked orders, browse players and the
//! audit log. The UI is the built admin app (ADR-022, `web/src/admin`), talking to
//! the JSON API under `/api`; strict CSP, session-bound CSRF, every mutation audited.
//! Second factor: TOTP per wallet from the environment (ADR-023, `totp`).
#![forbid(unsafe_code)]

pub mod api;
pub mod audit;
pub mod config;
pub mod error;
pub mod html;
pub mod metrics;
pub mod routes;
pub mod session;
pub mod spa;
pub mod spec;
pub mod state;
pub mod totp;

use axum::http::header::{
    CACHE_CONTROL, CONTENT_SECURITY_POLICY, REFERRER_POLICY, X_CONTENT_TYPE_OPTIONS, X_FRAME_OPTIONS,
};
use axum::http::{HeaderValue, StatusCode};
use axum::routing::{get, post};
use axum::Router;
use serde::Deserialize;
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::services::ServeDir;
use tower_http::set_header::SetResponseHeaderLayer;
use tower_http::trace::TraceLayer;

pub use config::AdminConfig;
pub use state::AdminState;

pub const BODY_LIMIT_BYTES: usize = 16 * 1024;
const CSP: &str = "default-src 'none'; script-src 'self'; style-src 'self'; connect-src 'self'; \
form-action 'self'; base-uri 'none'; frame-ancestors 'none'";

/// Forms that only need the CSRF token (logout).
#[derive(Debug, Deserialize)]
pub struct CsrfForm {
    pub csrf: String,
}

pub fn build_app(state: AdminState) -> Router {
    let mut app = Router::new()
        .route("/healthz", get(|| async { "ok" }))
        // Browsers ask for it on every page; nothing to show and nothing to redirect.
        .route("/favicon.ico", get(|| async { StatusCode::NO_CONTENT }))
        .route("/admin.css", get(routes::auth::css))
        .route("/login", get(routes::auth::login_page))
        .route("/login.js", get(routes::auth::login_js))
        .route("/login/nonce", get(routes::auth::nonce))
        .route("/login/verify", post(routes::auth::verify))
        .route("/login/totp", post(routes::auth::totp))
        .route("/logout", post(routes::auth::logout))
        .nest("/api", api::router());
    // Hashed build files of the admin app (ADR-022); everything else in the app is `index.html`.
    if let Some(dir) = state.config.static_dir.as_ref() {
        app = app.nest_service("/assets", ServeDir::new(dir.join("assets")));
    }
    app.fallback(spa::index)
        .layer(RequestBodyLimitLayer::new(BODY_LIMIT_BYTES))
        .layer(SetResponseHeaderLayer::overriding(
            CACHE_CONTROL,
            HeaderValue::from_static("no-store"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            CONTENT_SECURITY_POLICY,
            HeaderValue::from_static(CSP),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            X_FRAME_OPTIONS,
            HeaderValue::from_static("DENY"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            REFERRER_POLICY,
            HeaderValue::from_static("no-referrer"),
        ))
        .layer(SetResponseHeaderLayer::overriding(
            X_CONTENT_TYPE_OPTIONS,
            HeaderValue::from_static("nosniff"),
        ))
        .layer(TraceLayer::new_for_http())
        .with_state(state)
}
