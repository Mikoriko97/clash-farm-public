//! Second factor (ADR-023): a wallet with a TOTP secret gets a pending cookie after its
//! signature; only a fresh, correct code opens the session, once; guesses are limited.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use axum::Router;
use cf_admin::totp::TotpSecret;
use common::*;
use serde_json::{json, Value};
use tower::ServiceExt;

/// RFC 6238 test key; any authenticator app accepts it, so the flow is checked end to end.
const SECRET_B32: &str = "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ";

async fn post_code(app: &Router, cookie: &str, code: &str) -> (StatusCode, Value, Vec<String>) {
    let req = Request::builder()
        .method(Method::POST)
        .uri("/login/totp")
        .header(header::COOKIE, cookie)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(json!({ "code": code }).to_string()))
        .unwrap();
    let (s, b, c) = read(app.clone().oneshot(req).await.unwrap()).await;
    (s, serde_json::from_str(&b).unwrap_or(Value::Null), c)
}

fn cookie_named<'a>(cookies: &'a [String], name: &str) -> Option<&'a String> {
    cookies.iter().find(|c| c.starts_with(&format!("{name}=")))
}

fn header_value(cookie: &str) -> String {
    cookie.split(';').next().unwrap().to_owned()
}

#[tokio::test]
async fn test_totp_gates_the_session_once_and_limits_guesses() {
    let h = harness_with_vars(None, &|address| {
        vec![(
            "CF_ADMIN_TOTP_SECRETS".to_owned(),
            format!("sol:{address}={SECRET_B32}"),
        )]
    })
    .await;
    let (status, body, cookies) = sign_in(&h, &h.key, &h.wallet).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["totp"], true);
    assert!(body["csrf"].is_null(), "no session before the code: {body}");
    assert!(
        cookie_named(&cookies, "cf_admin").is_none(),
        "no session cookie yet"
    );
    let pending = cookie_named(&cookies, "cf_admin_2fa").expect("pending cookie");
    assert!(
        pending.contains("HttpOnly") && pending.contains("Path=/login"),
        "{pending}"
    );
    let pending = header_value(pending);
    let (status, _) = get(&h.app, &pending, "/api/dashboard").await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "the pending cookie opens nothing"
    );

    let secret = TotpSecret::from_base32(SECRET_B32).unwrap();
    let now = time::OffsetDateTime::now_utc().unix_timestamp();
    let code = secret.code(u64::try_from(now / 30).unwrap()).unwrap();
    let right = format!("{code:06}");
    let wrong = format!("{:06}", (code + 1) % 1_000_000);
    let (status, _, _) = post_code(&h.app, &pending, &wrong).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, body, cookies) = post_code(&h.app, &pending, &right).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let session = header_value(cookie_named(&cookies, "cf_admin").expect("session cookie"));
    assert!(body["csrf"].is_string());
    let (status, me) = get_json(&h.app, &session, "/api/me").await;
    assert_eq!(status, StatusCode::OK, "{me}");
    assert_eq!(me["totp"], true);
    let audited: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM cf.audit_log WHERE actor = $1 AND action = 'admin_login' AND after->'factors' = $2",
    )
    .bind(&h.wallet)
    .bind(json!(["wallet", "totp"]))
    .fetch_one(&h.pool)
    .await
    .unwrap();
    assert_eq!(audited, 1, "the audit row names both factors");

    // The pending cookie is spent, and the step is spent even for a fresh sign-in.
    let (status, _, _) = post_code(&h.app, &pending, &right).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "pending cookie consumed");
    let (_, _, cookies) = sign_in(&h, &h.key, &h.wallet).await;
    let pending2 = header_value(cookie_named(&cookies, "cf_admin_2fa").unwrap());
    let (status, _, _) = post_code(&h.app, &pending2, &right).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "one time step is accepted once");

    // Guessing is limited per wallet: the sixth try in a minute is refused outright.
    let mut last = StatusCode::OK;
    for _ in 0..6 {
        last = post_code(&h.app, &pending2, &wrong).await.0;
    }
    assert_eq!(last, StatusCode::TOO_MANY_REQUESTS);
    let refused: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM cf.audit_log WHERE actor = $1 AND action = 'admin_totp_rejected'",
    )
    .bind(&h.wallet)
    .fetch_one(&h.pool)
    .await
    .unwrap();
    assert!(refused >= 2, "wrong code and replay are audited: {refused}");
}

#[tokio::test]
async fn test_wallets_without_a_secret_sign_in_with_one_factor_unless_required() {
    let h = harness().await;
    let (cookie, _) = login(&h, &h.key, &h.wallet).await.unwrap();
    let (status, me) = get_json(&h.app, &cookie, "/api/me").await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(me["totp"], false);
    let audited: i64 = sqlx::query_scalar(
        "SELECT count(*) FROM cf.audit_log WHERE actor = $1 AND action = 'admin_login' AND after->'factors' = $2",
    )
    .bind(&h.wallet)
    .bind(json!(["wallet"]))
    .fetch_one(&h.pool)
    .await
    .unwrap();
    assert_eq!(audited, 1);
    // A code without a pending sign-in is refused, not redirected.
    let (status, _, _) = post_code(&h.app, "", "123456").await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}
