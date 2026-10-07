//! Shared helpers for integration tests: app against local Postgres/Memurai,
//! wallet login, requests carrying session cookies + CSRF + Idempotency-Key.

#![allow(dead_code, clippy::unwrap_used, clippy::expect_used, clippy::panic)]

pub mod payments;

use std::collections::HashMap;

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use axum::Router;
use cf_api::{build_app, AppState};
use cf_common::Config;
use ed25519_dalek::{Signer, SigningKey};
use http_body_util::BodyExt;
use serde_json::Value;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use tower::ServiceExt;

pub const DOMAIN: &str = "localhost";

pub fn database_url() -> String {
    std::env::var("CF_TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://migrator@127.0.0.1:5432/cf_test".to_owned())
}

pub async fn pool() -> PgPool {
    PgPoolOptions::new()
        .max_connections(4)
        .connect(&database_url())
        .await
        .expect("postgres")
}

pub async fn app() -> (Router, PgPool) {
    app_with(false).await
}

/// `fake = true`: the local fake-payments runtime (no chain), as `CF_PAYMENTS_FAKE=1` would build.
pub async fn app_with(fake: bool) -> (Router, PgPool) {
    let database_url = database_url();
    let redis_url =
        std::env::var("CF_TEST_REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379/1".to_owned());
    let vars: HashMap<String, String> = [
        ("CF_DATABASE_URL", database_url.as_str()),
        ("CF_REDIS_URL", redis_url.as_str()),
        ("CF_COOKIE_SECRET", "test-cookie-secret-test-cookie-secret-0000"),
        ("CF_CSRF_SECRET", "test-csrf-secret-test-csrf-secret-000000"),
        ("CF_SIWS_DOMAIN", DOMAIN),
        ("CF_CLASH_MINT", "9mM1Mc4Ta9UJJ32v5qsHef91PiXi7EWyiSsqF5WXpump"),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_owned(), v.to_owned()))
    .collect();
    let config = Config::from_map(&vars).expect("test config");
    let pool = pool().await;
    sqlx::migrate!("../../migrations")
        .run(&pool)
        .await
        .expect("migrations");
    let client = redis::Client::open(redis_url).expect("redis url");
    let redis = client.get_connection_manager().await.expect("redis");
    let payments = if fake {
        Some(std::sync::Arc::new(
            cf_payments::PaymentsRuntime::fake(&config.clash_mint, config.clash_decimals)
                .expect("fake runtime"),
        ))
    } else {
        None
    };
    (
        build_app(AppState::from_parts_with_payments(
            config,
            pool.clone(),
            redis,
            payments,
        )),
        pool,
    )
}

pub fn rand_bytes() -> [u8; 32] {
    use rand::Rng;
    let mut b = [0u8; 32];
    rand::rng().fill_bytes(&mut b);
    b
}

pub fn fresh_ip() -> String {
    let b = rand_bytes();
    format!("10.{}.{}.{}", b[0], b[1], b[2])
}

#[derive(Debug, Clone)]
pub struct Session {
    pub user_id: String,
    pub wallet: String,
    pub access: String,
    pub csrf: String,
    /// Client address this session's requests claim, so the per-IP limits
    /// (shared Redis) count each test on its own instead of the whole suite.
    pub ip: String,
}

impl Session {
    pub fn cookie(&self) -> String {
        format!("cf_access={}; cf_csrf={}", self.access, self.csrf)
    }
}

pub async fn json(response: axum::response::Response) -> (StatusCode, Value, Vec<String>) {
    let status = response.status();
    let cookies = response
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok().map(str::to_owned))
        .collect();
    let bytes = response.into_body().collect().await.expect("body").to_bytes();
    let value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).unwrap_or(Value::Null)
    };
    (status, value, cookies)
}

pub fn cookie_value(cookies: &[String], name: &str) -> Option<String> {
    cookies.iter().find_map(|c| {
        c.strip_prefix(&format!("{name}="))
            .and_then(|rest| rest.split(';').next())
            .map(str::to_owned)
    })
}

/// Logs in a fresh Solana wallet and returns its session.
pub async fn login(app: &Router) -> Session {
    login_cookies(app).await.0
}

/// Same, plus every `Set-Cookie` of the sign-in (the refresh token among them).
pub async fn login_cookies(app: &Router) -> (Session, Vec<String>) {
    let key = SigningKey::from_bytes(&rand_bytes());
    let wallet = format!(
        "sol:{}",
        bs58::encode(key.verifying_key().to_bytes()).into_string()
    );
    let req = Request::builder()
        .uri(format!("/api/v1/auth/nonce?wallet={wallet}"))
        .header("x-forwarded-for", fresh_ip())
        .body(Body::empty())
        .unwrap();
    let (status, body, _) = json(app.clone().oneshot(req).await.unwrap()).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let message = body["message"].as_str().unwrap().to_owned();
    let signature = hex::encode(key.sign(message.as_bytes()).to_bytes());
    let payload = serde_json::json!({ "wallet": wallet, "signature": signature, "message": message });
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/auth/verify")
        .header("x-forwarded-for", fresh_ip())
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(payload.to_string()))
        .unwrap();
    let (status, body, cookies) = json(app.clone().oneshot(req).await.unwrap()).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let session = Session {
        user_id: body["user"]["id"].as_str().unwrap().to_owned(),
        wallet,
        access: cookie_value(&cookies, "cf_access").unwrap(),
        csrf: cookie_value(&cookies, "cf_csrf").unwrap(),
        ip: fresh_ip(),
    };
    (session, cookies)
}

/// `POST /auth/guest` (0028) from `ip` with no cookies; the session when it succeeded.
pub async fn guest(app: &Router, ip: &str) -> (StatusCode, Value, Option<Session>) {
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/auth/guest")
        .header("x-forwarded-for", ip)
        .body(Body::empty())
        .unwrap();
    let (status, body, cookies) = json(app.clone().oneshot(req).await.unwrap()).await;
    let session = (status == StatusCode::OK).then(|| Session {
        user_id: body["user"]["id"].as_str().unwrap().to_owned(),
        wallet: String::new(),
        access: cookie_value(&cookies, "cf_access").unwrap(),
        csrf: cookie_value(&cookies, "cf_csrf").unwrap(),
        ip: fresh_ip(),
    });
    (status, body, session)
}

pub async fn get(app: &Router, session: &Session, path: &str) -> (StatusCode, Value) {
    let req = Request::builder()
        .uri(format!("/api/v1{path}"))
        .header(header::COOKIE, session.cookie())
        .header("x-forwarded-for", &session.ip)
        .body(Body::empty())
        .unwrap();
    let (status, body, _) = json(app.clone().oneshot(req).await.unwrap()).await;
    (status, body)
}

/// POST with session cookie, CSRF header and a fresh (or given) Idempotency-Key.
pub async fn post(
    app: &Router,
    session: &Session,
    path: &str,
    body: Option<Value>,
    idem: Option<&str>,
) -> (StatusCode, Value) {
    let key = idem
        .map(str::to_owned)
        .unwrap_or_else(|| uuid::Uuid::new_v4().to_string());
    let mut builder = Request::builder()
        .method(Method::POST)
        .uri(format!("/api/v1{path}"))
        .header(header::COOKIE, session.cookie())
        .header("x-forwarded-for", &session.ip)
        .header("x-csrf-token", &session.csrf)
        .header("idempotency-key", key);
    let payload = match body {
        Some(value) => {
            builder = builder.header(header::CONTENT_TYPE, "application/json");
            Body::from(value.to_string())
        }
        None => Body::empty(),
    };
    let (status, value, _) = json(app.clone().oneshot(builder.body(payload).unwrap()).await.unwrap()).await;
    (status, value)
}
