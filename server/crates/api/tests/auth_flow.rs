//! Integration tests against local Postgres + Memurai (infra/db/README.md).
//! Test names follow docs/SECURITY.md.
//!
//! Env (optional): CF_TEST_DATABASE_URL, CF_TEST_REDIS_URL.

#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::HashMap;

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use axum::Router;
use cf_api::{build_app, AppState};
use cf_common::Config;
use ed25519_dalek::{Signer, SigningKey};
use http_body_util::BodyExt;
use k256::ecdsa::SigningKey as EcdsaSigningKey;
use serde_json::Value;
use sqlx::postgres::PgPoolOptions;
use tower::ServiceExt;

const DOMAIN: &str = "localhost";

async fn app() -> Router {
    let database_url = std::env::var("CF_TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://migrator@127.0.0.1:5432/cf_test".to_owned());
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
    let pool = PgPoolOptions::new()
        .max_connections(4)
        .connect(&database_url)
        .await
        .expect("postgres");
    sqlx::migrate!("../../migrations")
        .run(&pool)
        .await
        .expect("migrations");
    let client = redis::Client::open(redis_url).expect("redis url");
    let redis = client.get_connection_manager().await.expect("redis");
    build_app(AppState::from_parts(config, pool, redis))
}

type SignFn = Box<dyn Fn(&[u8]) -> Vec<u8>>;

struct Wallet {
    tag: String,
    sign: SignFn,
}

fn solana_wallet() -> Wallet {
    let key = SigningKey::from_bytes(&rand_bytes());
    let tag = format!(
        "sol:{}",
        bs58::encode(key.verifying_key().to_bytes()).into_string()
    );
    Wallet {
        tag,
        sign: Box::new(move |m| key.sign(m).to_bytes().to_vec()),
    }
}

fn evm_wallet() -> Wallet {
    let key = EcdsaSigningKey::from_slice(&rand_bytes()).expect("key");
    let tag = format!(
        "evm:{}",
        cf_auth::verify::address_from_verifying_key(key.verifying_key())
    );
    Wallet {
        tag,
        sign: Box::new(move |m| {
            let (sig, recid) = key.sign_prehash_recoverable(&cf_auth::verify::eip191_hash(m));
            let mut bytes = sig.to_bytes().to_vec();
            bytes.push(recid.to_byte() + 27);
            bytes
        }),
    }
}

fn rand_bytes() -> [u8; 32] {
    use rand::Rng;
    let mut b = [0u8; 32];
    rand::rng().fill_bytes(&mut b);
    b
}

/// Unique client IP per request so the per-IP auth limit never trips across parallel tests.
fn fresh_ip() -> String {
    let b = rand_bytes();
    format!("10.{}.{}.{}", b[0], b[1], b[2])
}

async fn json(response: axum::response::Response) -> (StatusCode, Value, Vec<String>) {
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

fn cookie_value<'a>(cookies: &'a [String], name: &str) -> Option<&'a str> {
    cookies.iter().find_map(|c| {
        c.strip_prefix(&format!("{name}="))
            .and_then(|rest| rest.split(';').next())
    })
}

async fn get_nonce(app: &Router, wallet: &str, chain: Option<u64>, ip: &str) -> (StatusCode, Value) {
    let uri = match chain {
        Some(c) => format!("/api/v1/auth/nonce?wallet={wallet}&chain_id={c}"),
        None => format!("/api/v1/auth/nonce?wallet={wallet}"),
    };
    let req = Request::builder()
        .uri(uri)
        .header("x-forwarded-for", ip)
        .body(Body::empty())
        .expect("req");
    let (status, body, _) = json(app.clone().oneshot(req).await.expect("resp")).await;
    (status, body)
}

async fn post_verify(
    app: &Router,
    wallet: &str,
    signature: &[u8],
    message: &str,
) -> (StatusCode, Value, Vec<String>) {
    let payload =
        serde_json::json!({ "wallet": wallet, "signature": hex::encode(signature), "message": message });
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/auth/verify")
        .header("x-forwarded-for", fresh_ip())
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(payload.to_string()))
        .expect("req");
    json(app.clone().oneshot(req).await.expect("resp")).await
}

/// Full happy path; returns the Set-Cookie values.
async fn login(app: &Router, wallet: &Wallet, chain: Option<u64>) -> Vec<String> {
    let (status, body) = get_nonce(app, &wallet.tag, chain, &fresh_ip()).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let message = body["message"].as_str().expect("message").to_owned();
    let (status, body, cookies) =
        post_verify(app, &wallet.tag, &(wallet.sign)(message.as_bytes()), &message).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["user"]["wallet"], wallet.tag);
    cookies
}

#[tokio::test]
async fn test_login_solana_and_me() {
    let app = app().await;
    let wallet = solana_wallet();
    let cookies = login(&app, &wallet, None).await;
    let access = cookie_value(&cookies, "cf_access").expect("access cookie");
    assert!(cookies
        .iter()
        .any(|c| c.starts_with("cf_access=") && c.contains("HttpOnly") && c.contains("SameSite=Strict")));
    let req = Request::builder()
        .uri("/api/v1/me")
        .header(header::COOKIE, format!("cf_access={access}"))
        .body(Body::empty())
        .unwrap();
    let (status, body, _) = json(app.clone().oneshot(req).await.unwrap()).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["user"]["wallet"], wallet.tag);
}

#[tokio::test]
async fn test_login_evm_siwe() {
    let app = app().await;
    let wallet = evm_wallet();
    let cookies = login(&app, &wallet, Some(8453)).await;
    assert!(cookie_value(&cookies, "cf_refresh").is_some());
}

#[tokio::test]
async fn test_evm_chain_not_allowed() {
    let app = app().await;
    let wallet = evm_wallet();
    let (status, body) = get_nonce(&app, &wallet.tag, Some(56), "10.0.0.2").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "chain_not_allowed");
    assert!(body["error"]["request_id"].is_string());
}

#[tokio::test]
async fn test_nonce_single_use() {
    let app = app().await;
    let wallet = solana_wallet();
    let (_, body) = get_nonce(&app, &wallet.tag, None, "10.0.0.3").await;
    let message = body["message"].as_str().unwrap().to_owned();
    let signature = (wallet.sign)(message.as_bytes());
    let (first, ..) = post_verify(&app, &wallet.tag, &signature, &message).await;
    assert_eq!(first, StatusCode::OK);
    let (second, body, _) = post_verify(&app, &wallet.tag, &signature, &message).await;
    assert_eq!(second, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"]["code"], "nonce_invalid");
}

#[tokio::test]
async fn test_wrong_domain_rejected() {
    let app = app().await;
    let wallet = solana_wallet();
    let (_, body) = get_nonce(&app, &wallet.tag, None, "10.0.0.4").await;
    let message = body["message"]
        .as_str()
        .unwrap()
        .replace("localhost", "evil.example");
    let (status, body, _) =
        post_verify(&app, &wallet.tag, &(wallet.sign)(message.as_bytes()), &message).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"]["code"], "wrong_domain");
}

#[tokio::test]
async fn test_bad_signature_and_foreign_wallet_rejected() {
    let app = app().await;
    let wallet = solana_wallet();
    let other = solana_wallet();
    let (_, body) = get_nonce(&app, &wallet.tag, None, "10.0.0.5").await;
    let message = body["message"].as_str().unwrap().to_owned();
    // Signed by a different key than the one in the message.
    let (status, body, _) = post_verify(&app, &wallet.tag, &(other.sign)(message.as_bytes()), &message).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"]["code"], "bad_signature");
    // Message issued for `wallet` cannot be used by `other` (nonce bound to wallet).
    let (_, body) = get_nonce(&app, &wallet.tag, None, "10.0.0.5").await;
    let message = body["message"].as_str().unwrap().to_owned();
    let (status, ..) = post_verify(&app, &other.tag, &(other.sign)(message.as_bytes()), &message).await;
    assert_ne!(status, StatusCode::OK);
}

#[tokio::test]
async fn test_expired_message_rejected() {
    let app = app().await;
    let wallet = solana_wallet();
    let (_, body) = get_nonce(&app, &wallet.tag, None, "10.0.0.6").await;
    let message = body["message"].as_str().unwrap().to_owned();
    let parsed = cf_auth::SignInMessage::parse(&message).unwrap();
    let stale = cf_auth::SignInMessage {
        issued_at: parsed.issued_at - time::Duration::hours(2),
        expiration_time: parsed.expiration_time - time::Duration::hours(2),
        ..parsed
    }
    .render();
    let (status, body, _) = post_verify(&app, &wallet.tag, &(wallet.sign)(stale.as_bytes()), &stale).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    assert_eq!(body["error"]["code"], "message_expired");
}

// test_refresh_reuse_revokes_family and the grace window of ADR-036 live in refresh_grace.rs.

#[tokio::test]
async fn test_csrf_missing_rejected_and_body_user_id_ignored() {
    let app = app().await;
    let wallet = solana_wallet();
    let cookies = login(&app, &wallet, None).await;
    let access = cookie_value(&cookies, "cf_access").unwrap().to_owned();
    let csrf = cookie_value(&cookies, "cf_csrf").unwrap().to_owned();
    // Logout without CSRF header → 403, session still alive.
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/auth/logout")
        .header("x-forwarded-for", fresh_ip())
        .header(header::COOKIE, format!("cf_access={access}; cf_csrf={csrf}"))
        .body(Body::empty())
        .unwrap();
    let (status, body, _) = json(app.clone().oneshot(req).await.unwrap()).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    // A foreign user_id in the body must not change whose profile /me returns.
    let req = Request::builder()
        .uri("/api/v1/me?user_id=00000000-0000-0000-0000-000000000001")
        .header(header::COOKIE, format!("cf_access={access}"))
        .body(Body::empty())
        .unwrap();
    let (status, body, _) = json(app.clone().oneshot(req).await.unwrap()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body["user"]["wallet"], wallet.tag);
    // Proper logout with CSRF → 200; then /me is 401.
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/auth/logout")
        .header("x-forwarded-for", fresh_ip())
        .header(header::COOKIE, format!("cf_access={access}; cf_csrf={csrf}"))
        .header("x-csrf-token", &csrf)
        .body(Body::empty())
        .unwrap();
    let (status, ..) = json(app.clone().oneshot(req).await.unwrap()).await;
    assert_eq!(status, StatusCode::OK);
    let req = Request::builder()
        .uri("/api/v1/me")
        .header(header::COOKIE, format!("cf_access={access}"))
        .body(Body::empty())
        .unwrap();
    let (status, ..) = json(app.clone().oneshot(req).await.unwrap()).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_rate_limit_auth() {
    let app = app().await;
    let wallet = solana_wallet();
    let ip = format!("10.9.{}.{}", rand_bytes()[0], rand_bytes()[1]);
    let mut last = StatusCode::OK;
    for _ in 0..11 {
        let (status, _) = get_nonce(&app, &wallet.tag, None, &ip).await;
        last = status;
    }
    assert_eq!(last, StatusCode::TOO_MANY_REQUESTS);
}

#[tokio::test]
async fn test_body_limit_and_invalid_wallet() {
    let app = app().await;
    let (status, body) = get_nonce(&app, "btc:1abc", None, "10.0.0.7").await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(body["error"]["code"], "invalid_wallet");
    let huge = "x".repeat(20_000);
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/auth/verify")
        .header("x-forwarded-for", fresh_ip())
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(format!("{{\"wallet\":\"{huge}\"}}")))
        .unwrap();
    let (status, ..) = json(app.clone().oneshot(req).await.unwrap()).await;
    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
}

/// Migration 0020: the admin bans a wallet — sign-in is refused, a held refresh token
/// dies on first use, and `last_seen_at` does not move on the refused attempt.
#[tokio::test]
async fn test_banned_wallet_cannot_sign_in_or_refresh() {
    let app = app().await;
    let wallet = solana_wallet();
    let cookies = login(&app, &wallet, None).await;
    let refresh = cookie_value(&cookies, "cf_refresh").unwrap().to_owned();
    let csrf = cookie_value(&cookies, "cf_csrf").unwrap().to_owned();
    let db = std::env::var("CF_TEST_DATABASE_URL")
        .unwrap_or_else(|_| "postgres://migrator@127.0.0.1:5432/cf_test".to_owned());
    let pool = PgPoolOptions::new()
        .max_connections(1)
        .connect(&db)
        .await
        .unwrap();
    sqlx::query("UPDATE cf.users SET banned_at = now(), ban_reason = 'test' WHERE wallet = $1")
        .bind(&wallet.tag)
        .execute(&pool)
        .await
        .unwrap();
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/auth/refresh")
        .header("x-csrf-token", &csrf)
        .header(header::COOKIE, format!("cf_refresh={refresh}; cf_csrf={csrf}"))
        .header("x-forwarded-for", fresh_ip())
        .body(Body::empty())
        .unwrap();
    let (status, body, _) = json(app.clone().oneshot(req).await.unwrap()).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(body["error"]["code"], "banned");
    let (status, body) = get_nonce(&app, &wallet.tag, None, &fresh_ip()).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let message = body["message"].as_str().unwrap().to_owned();
    let (status, body, _) =
        post_verify(&app, &wallet.tag, &(wallet.sign)(message.as_bytes()), &message).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(body["error"]["code"], "banned");
    let live: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM cf.sessions s JOIN cf.users u ON u.id = s.user_id WHERE u.wallet = $1 AND s.revoked_at IS NULL",
    )
    .bind(&wallet.tag)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(live, 0, "the refresh family is gone");
}
