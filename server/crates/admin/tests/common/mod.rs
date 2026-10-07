//! Shared harness of the cf-admin integration tests: local Postgres + Memurai, an
//! allowlisted signing key, HTTP/JSON helpers and fixtures (parked orders, settings).
#![allow(dead_code, clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::HashMap;

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use axum::Router;
use cf_admin::{build_app, AdminConfig, AdminState};
use cf_auth::SessionService;
use ed25519_dalek::{Signer, SigningKey};
use http_body_util::BodyExt;
use serde_json::{json, Value};
use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Row};
use tower::ServiceExt;
use uuid::Uuid;

pub fn database_url() -> String {
    std::env::var("CF_TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://migrator@127.0.0.1:5432/cf_test".to_owned())
}

/// Login is rate-limited per IP: every attempt comes from its own fake proxy address.
pub fn fresh_ip() -> String {
    let b = rand_bytes();
    format!("10.{}.{}.{}", b[0], b[1], b[2])
}

pub fn rand_bytes() -> [u8; 32] {
    use rand::Rng;
    let mut b = [0u8; 32];
    rand::rng().fill_bytes(&mut b);
    b
}

pub struct Harness {
    pub app: Router,
    pub pool: PgPool,
    pub key: SigningKey,
    pub wallet: String,
}

pub async fn harness() -> Harness {
    harness_with(None).await
}

/// `static_dir` = a built admin app to serve (ADR-022); `None` = JSON API only.
pub async fn harness_with(static_dir: Option<&std::path::Path>) -> Harness {
    harness_with_vars(static_dir, &|_| Vec::new()).await
}

/// `extra(address)` adds variables that need the fresh admin key (TOTP secrets, ADR-023).
pub async fn harness_with_vars(
    static_dir: Option<&std::path::Path>,
    extra: &dyn Fn(&str) -> Vec<(String, String)>,
) -> Harness {
    let key = SigningKey::from_bytes(&rand_bytes());
    let address = bs58::encode(key.verifying_key().to_bytes()).into_string();
    let redis_url =
        std::env::var("CF_TEST_REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379/1".to_owned());
    let db = database_url();
    let static_dir = static_dir
        .map(|d| d.to_string_lossy().into_owned())
        .unwrap_or_default();
    let mut vars: HashMap<String, String> = [
        ("CF_DATABASE_URL", db.as_str()),
        ("CF_REDIS_URL", redis_url.as_str()),
        ("CF_COOKIE_SECRET", "test-cookie-secret-test-cookie-secret-0000"),
        ("CF_SIWS_DOMAIN", "localhost"),
        ("CF_ADMIN_WALLETS", address.as_str()),
        ("CF_ADMIN_STATIC_DIR", static_dir.as_str()),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_owned(), v.to_owned()))
    .collect();
    vars.extend(extra(&address));
    let config = AdminConfig::from_map(&vars).expect("config");
    let pool = PgPoolOptions::new()
        .max_connections(4)
        .connect(&db)
        .await
        .expect("postgres");
    sqlx::migrate!("../../migrations")
        .run(&pool)
        .await
        .expect("migrations");
    let client = redis::Client::open(redis_url).expect("redis url");
    let redis = client.get_connection_manager().await.expect("redis");
    Harness {
        app: build_app(AdminState::from_parts(config, pool.clone(), redis)),
        pool,
        key,
        wallet: format!("sol:{address}"),
    }
}

pub async fn read(response: axum::response::Response) -> (StatusCode, String, Vec<String>) {
    let status = response.status();
    let cookies = response
        .headers()
        .get_all(header::SET_COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok().map(str::to_owned))
        .collect();
    let location = response
        .headers()
        .get(header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    let mut body = String::from_utf8_lossy(&bytes).into_owned();
    if let Some(l) = location {
        body = format!("LOCATION:{l}\n{body}");
    }
    (status, body, cookies)
}

pub async fn get(app: &Router, cookie: &str, path: &str) -> (StatusCode, String) {
    let req = Request::builder()
        .uri(path)
        .header(header::COOKIE, cookie)
        .body(Body::empty())
        .unwrap();
    let (s, b, _) = read(app.clone().oneshot(req).await.unwrap()).await;
    (s, b)
}

pub async fn get_json(app: &Router, cookie: &str, path: &str) -> (StatusCode, Value) {
    let (s, b) = get(app, cookie, path).await;
    (s, serde_json::from_str(&b).unwrap_or(Value::Null))
}

/// JSON POST with the session cookie and the `X-CSRF` header (`csrf` may be wrong on purpose).
pub async fn post_json(
    app: &Router,
    cookie: &str,
    csrf: &str,
    path: &str,
    body: Value,
) -> (StatusCode, Value) {
    let req = Request::builder()
        .method(Method::POST)
        .uri(path)
        .header(header::COOKIE, cookie)
        .header("x-csrf", csrf)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(body.to_string()))
        .unwrap();
    let (s, b, _) = read(app.clone().oneshot(req).await.unwrap()).await;
    (s, serde_json::from_str(&b).unwrap_or(Value::Null))
}

/// Nonce → sign → verify; returns (cookie header, csrf token).
pub async fn login(h: &Harness, key: &SigningKey, wallet: &str) -> Result<(String, String), StatusCode> {
    let (status, body, cookies) = sign_in(h, key, wallet).await;
    if status != StatusCode::OK {
        return Err(status);
    }
    let cookie = cookies
        .first()
        .and_then(|c| c.split(';').next())
        .expect("session cookie")
        .to_owned();
    Ok((cookie, body["csrf"].as_str().expect("csrf").to_owned()))
}

/// Nonce → sign → verify, raw: (status, body, Set-Cookie headers).
pub async fn sign_in(h: &Harness, key: &SigningKey, wallet: &str) -> (StatusCode, Value, Vec<String>) {
    let ip = fresh_ip();
    let req = Request::builder()
        .uri(format!("/login/nonce?wallet={wallet}"))
        .header("x-forwarded-for", &ip)
        .body(Body::empty())
        .unwrap();
    let (status, body, _) = read(h.app.clone().oneshot(req).await.unwrap()).await;
    if status != StatusCode::OK {
        return (status, Value::Null, Vec::new());
    }
    let nonce: Value = serde_json::from_str(&body).unwrap();
    let message = nonce["message"].as_str().unwrap().to_owned();
    let signature = hex::encode(key.sign(message.as_bytes()).to_bytes());
    let payload = json!({ "wallet": wallet, "signature": signature, "message": message });
    let req = Request::builder()
        .method(Method::POST)
        .uri("/login/verify")
        .header("x-forwarded-for", &ip)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(payload.to_string()))
        .unwrap();
    let (status, body, cookies) = read(h.app.clone().oneshot(req).await.unwrap()).await;
    (
        status,
        serde_json::from_str(&body).unwrap_or(Value::Null),
        cookies,
    )
}

/// The players' session service as cf-api runs it, on the harness's Redis and secret.
pub async fn player_sessions(h: &Harness) -> SessionService {
    let redis_url =
        std::env::var("CF_TEST_REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379/1".to_owned());
    let client = redis::Client::open(redis_url).expect("redis url");
    let redis = client.get_connection_manager().await.expect("redis");
    SessionService::new(
        h.pool.clone(),
        redis,
        "test-cookie-secret-test-cookie-secret-0000",
    )
}

pub async fn setting(pool: &PgPool, key: &str) -> Option<Value> {
    sqlx::query_scalar("SELECT value FROM cf.settings WHERE key = $1")
        .bind(key)
        .fetch_optional(pool)
        .await
        .unwrap()
}

pub async fn set_setting(pool: &PgPool, key: &str, value: Value) {
    sqlx::query("INSERT INTO cf.settings (key, value) VALUES ($1, $2) ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value")
        .bind(key)
        .bind(value)
        .execute(pool)
        .await
        .unwrap();
}

/// A parked order; `signature` = what the player submitted (recorded in `events` like `submit` does).
pub async fn parked_order(pool: &PgPool, signature: Option<&str>) -> (Uuid, Uuid) {
    let wallet = format!("sol:{}", bs58::encode(rand_bytes()).into_string());
    let user_id: Uuid = sqlx::query_scalar("INSERT INTO cf.users (wallet) VALUES ($1) RETURNING id")
        .bind(&wallet)
        .fetch_one(pool)
        .await
        .unwrap();
    let order_id: Uuid = sqlx::query_scalar(
        "INSERT INTO cf.orders (user_id, item_id, qty, price_usd_cents, clash_per_usd, amount_clash,
                                burn_amount, transfer_amount, quote_expires_at, status, reason_code)
         VALUES ($1, 'decor_lantern', 1, 90, 1000, 90000, 63000, 27000, now() + interval '90 seconds', 'needs_review', 'rpc_error')
         RETURNING id",
    )
    .bind(user_id)
    .fetch_one(pool)
    .await
    .unwrap();
    if let Some(sig) = signature {
        sqlx::query("INSERT INTO cf.events (user_id, name, props) VALUES ($1, 'order_submitted', $2)")
            .bind(user_id)
            .bind(json!({ "order_id": order_id, "signature": sig }))
            .execute(pool)
            .await
            .unwrap();
    }
    (user_id, order_id)
}

pub fn fresh_signature() -> String {
    bs58::encode([rand_bytes(), rand_bytes()].concat()).into_string()
}

pub async fn order_row(pool: &PgPool, id: Uuid) -> (String, Option<String>) {
    let row = sqlx::query("SELECT status, reason_code FROM cf.orders WHERE id = $1")
        .bind(id)
        .fetch_one(pool)
        .await
        .unwrap();
    (
        row.try_get("status").unwrap(),
        row.try_get("reason_code").unwrap(),
    )
}

pub async fn audit_count(pool: &PgPool, action: &str, target: Uuid) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM cf.audit_log WHERE action = $1 AND target = $2")
        .bind(action)
        .bind(target.to_string())
        .fetch_one(pool)
        .await
        .unwrap()
}
