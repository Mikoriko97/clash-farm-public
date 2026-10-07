//! Refresh rotation (docs/SECURITY.md §3, ADR-036): reuse of a rotated token revokes the family,
//! except when the same browser repeats it within `REFRESH_REUSE_GRACE` — a lost response or a
//! second tab — which rotates from the family's live token instead. Logout, another browser or a
//! later replay are never forgiven. `family_alive` is what the presence socket re-checks.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use axum::Router;
use cf_auth::session::SessionService;
use common::{app, cookie_value, fresh_ip, json, login_cookies};
use serde_json::Value;
use sqlx::PgPool;
use tower::ServiceExt;

/// A browser's cookie jar for one session: the newest access, refresh and CSRF tokens.
#[derive(Clone)]
struct Jar {
    access: String,
    refresh: String,
    csrf: String,
}

impl Jar {
    fn from(cookies: &[String]) -> Self {
        Self {
            access: cookie_value(cookies, "cf_access").unwrap(),
            refresh: cookie_value(cookies, "cf_refresh").unwrap(),
            csrf: cookie_value(cookies, "cf_csrf").unwrap(),
        }
    }
}

async fn signed_in(app: &Router) -> (String, Jar) {
    let (session, cookies) = login_cookies(app).await;
    (session.user_id, Jar::from(&cookies))
}

/// `POST /auth/refresh` with `refresh` from a browser whose user agent is `ua` (none = the sign-in's).
async fn refresh(
    app: &Router,
    refresh: &str,
    csrf: &str,
    ua: Option<&str>,
) -> (StatusCode, Value, Vec<String>) {
    let mut req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/auth/refresh")
        .header("x-forwarded-for", fresh_ip())
        .header(header::COOKIE, format!("cf_refresh={refresh}; cf_csrf={csrf}"))
        .header("x-csrf-token", csrf);
    if let Some(ua) = ua {
        req = req.header(header::USER_AGENT, ua);
    }
    json(
        app.clone()
            .oneshot(req.body(Body::empty()).unwrap())
            .await
            .unwrap(),
    )
    .await
}

async fn me(app: &Router, jar: &Jar) -> StatusCode {
    let req = Request::builder()
        .uri("/api/v1/me")
        .header("x-forwarded-for", fresh_ip())
        .header(
            header::COOKIE,
            format!("cf_access={}; cf_csrf={}", jar.access, jar.csrf),
        )
        .body(Body::empty())
        .unwrap();
    app.clone().oneshot(req).await.unwrap().status()
}

/// `POST /auth/logout` with the access token and, if given, a refresh cookie (maybe a stale one).
async fn logout(app: &Router, jar: &Jar, refresh: Option<&str>) -> StatusCode {
    let mut cookie = format!("cf_access={}; cf_csrf={}", jar.access, jar.csrf);
    if let Some(r) = refresh {
        cookie.push_str(&format!("; cf_refresh={r}"));
    }
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/auth/logout")
        .header("x-forwarded-for", fresh_ip())
        .header(header::COOKIE, cookie)
        .header("x-csrf-token", &jar.csrf)
        .body(Body::empty())
        .unwrap();
    app.clone().oneshot(req).await.unwrap().status()
}

async fn sessions(pool: &PgPool) -> SessionService {
    let redis_url =
        std::env::var("CF_TEST_REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379/1".to_owned());
    let redis = redis::Client::open(redis_url)
        .unwrap()
        .get_connection_manager()
        .await
        .unwrap();
    // Any key: `family_alive` hashes nothing.
    SessionService::new(pool.clone(), redis, "unused-in-family-alive-0000000000000000")
}

async fn family_of(pool: &PgPool, user_id: &str) -> uuid::Uuid {
    sqlx::query_scalar("SELECT DISTINCT family FROM cf.sessions WHERE user_id = $1::uuid")
        .bind(user_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

async fn live_tokens(pool: &PgPool, user_id: &str) -> i64 {
    sqlx::query_scalar("SELECT count(*) FROM cf.sessions WHERE user_id = $1::uuid AND revoked_at IS NULL")
        .bind(user_id)
        .fetch_one(pool)
        .await
        .unwrap()
}

/// Moves the rotation of every revoked token of the user out of the grace window.
async fn age_rotations(pool: &PgPool, user_id: &str) {
    sqlx::query("UPDATE cf.sessions SET revoked_at = now() - interval '5 minutes' WHERE user_id = $1::uuid AND revoked_at IS NOT NULL")
        .bind(user_id)
        .execute(pool)
        .await
        .unwrap();
}

#[tokio::test]
async fn test_refresh_reuse_revokes_family() {
    let (app, pool) = app().await;
    let (user, jar1) = signed_in(&app).await;
    let (status, _, cookies) = refresh(&app, &jar1.refresh, &jar1.csrf, None).await;
    assert_eq!(status, StatusCode::OK);
    let jar2 = Jar::from(&cookies);
    assert_ne!(jar1.refresh, jar2.refresh);
    // Replayed after the grace window: a stolen token. The family is revoked.
    age_rotations(&pool, &user).await;
    let (status, body, _) = refresh(&app, &jar1.refresh, &jar1.csrf, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"]["code"], "session_revoked");
    // The newest token and its access token are dead too.
    let (status, ..) = refresh(&app, &jar2.refresh, &jar2.csrf, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(me(&app, &jar2).await, StatusCode::UNAUTHORIZED);
    assert_eq!(live_tokens(&pool, &user).await, 0);
}

#[tokio::test]
async fn test_refresh_replay_within_grace_rotates_from_the_live_token() {
    let (app, pool) = app().await;
    let (user, jar1) = signed_in(&app).await;
    let (status, _, cookies) = refresh(&app, &jar1.refresh, &jar1.csrf, None).await;
    assert_eq!(status, StatusCode::OK);
    let jar2 = Jar::from(&cookies);
    // The second tab (or a retry after a lost response) presents the old token at once: a new session.
    let (status, body, cookies) = refresh(&app, &jar1.refresh, &jar1.csrf, None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let jar3 = Jar::from(&cookies);
    assert!(jar3.refresh != jar1.refresh && jar3.refresh != jar2.refresh);
    // Still one session: the token it replaced (jar2) and its access token are gone, the newest works.
    assert_eq!(live_tokens(&pool, &user).await, 1);
    assert_eq!(me(&app, &jar2).await, StatusCode::UNAUTHORIZED);
    assert_eq!(me(&app, &jar3).await, StatusCode::OK);
    let (status, _, cookies) = refresh(&app, &jar3.refresh, &jar3.csrf, None).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(me(&app, &Jar::from(&cookies)).await, StatusCode::OK);
}

#[tokio::test]
async fn test_refresh_replay_from_another_browser_revokes_family() {
    let (app, pool) = app().await;
    let (user, jar1) = signed_in(&app).await;
    let (status, _, cookies) = refresh(&app, &jar1.refresh, &jar1.csrf, None).await;
    assert_eq!(status, StatusCode::OK);
    let jar2 = Jar::from(&cookies);
    let (status, body, _) = refresh(&app, &jar1.refresh, &jar1.csrf, Some("Mozilla/5.0 (thief)")).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert_eq!(body["error"]["code"], "session_revoked");
    assert_eq!(me(&app, &jar2).await, StatusCode::UNAUTHORIZED);
    assert_eq!(live_tokens(&pool, &user).await, 0);
}

#[tokio::test]
async fn test_refresh_replay_after_logout_is_not_revived() {
    let (app, pool) = app().await;
    let svc = sessions(&pool).await;
    // The browser lost the answer of its last refresh: it still holds the old refresh cookie when
    // it signs out (or none at all). The whole family ends anyway, and nothing comes back.
    for stale in [true, false] {
        let (user, jar1) = signed_in(&app).await;
        let family = family_of(&pool, &user).await;
        let (_, _, cookies) = refresh(&app, &jar1.refresh, &jar1.csrf, None).await;
        let jar2 = Jar::from(&cookies);
        let cookie = stale.then_some(jar1.refresh.as_str());
        assert!(logout(&app, &jar2, cookie).await.is_success());
        assert_eq!(live_tokens(&pool, &user).await, 0, "stale cookie: {stale}");
        assert!(!svc.family_alive(family).await.unwrap());
        let (status, ..) = refresh(&app, &jar1.refresh, &jar1.csrf, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        let (status, ..) = refresh(&app, &jar2.refresh, &jar2.csrf, None).await;
        assert_eq!(status, StatusCode::UNAUTHORIZED);
        assert_eq!(live_tokens(&pool, &user).await, 0);
    }
}

#[tokio::test]
async fn test_concurrent_rotation_and_revocation_leave_no_live_token() {
    let (app, pool) = app().await;
    for _ in 0..6 {
        let (user, jar1) = signed_in(&app).await;
        let (_, _, cookies) = refresh(&app, &jar1.refresh, &jar1.csrf, None).await;
        let jar2 = Jar::from(&cookies);
        // A refresh and a logout of the same family at once: whichever runs first, the family ends.
        let (_, out) = tokio::join!(
            refresh(&app, &jar2.refresh, &jar2.csrf, None),
            logout(&app, &jar2, Some(&jar2.refresh))
        );
        assert!(out.is_success() || out == StatusCode::UNAUTHORIZED);
        assert_eq!(live_tokens(&pool, &user).await, 0);
    }
}

#[tokio::test]
async fn test_concurrent_grace_and_rotation_both_keep_the_session() {
    let (app, pool) = app().await;
    for _ in 0..6 {
        let (user, jar1) = signed_in(&app).await;
        let (_, _, cookies) = refresh(&app, &jar1.refresh, &jar1.csrf, None).await;
        let jar2 = Jar::from(&cookies);
        // Tab A rotates the current token while tab B (lost answer) repeats the old one.
        let ((a, ..), (b, ..)) = tokio::join!(
            refresh(&app, &jar2.refresh, &jar2.csrf, None),
            refresh(&app, &jar1.refresh, &jar1.csrf, None)
        );
        assert_eq!((a, b), (StatusCode::OK, StatusCode::OK));
        assert_eq!(live_tokens(&pool, &user).await, 1);
    }
}

#[tokio::test]
async fn test_family_alive_follows_the_session_not_the_access_token() {
    let (app, pool) = app().await;
    let (user, jar1) = signed_in(&app).await;
    let family = family_of(&pool, &user).await;
    let sessions = sessions(&pool).await;
    assert!(sessions.family_alive(family).await.unwrap());
    // Rotation kills the old access token, not the session.
    let (_, _, cookies) = refresh(&app, &jar1.refresh, &jar1.csrf, None).await;
    assert_eq!(me(&app, &jar1).await, StatusCode::UNAUTHORIZED);
    assert!(sessions.family_alive(family).await.unwrap());
    // A ban ends it, and so does a revoked family.
    sqlx::query("UPDATE cf.users SET banned_at = now() WHERE id = $1::uuid")
        .bind(&user)
        .execute(&pool)
        .await
        .unwrap();
    assert!(!sessions.family_alive(family).await.unwrap());
    sqlx::query("UPDATE cf.users SET banned_at = NULL WHERE id = $1::uuid")
        .bind(&user)
        .execute(&pool)
        .await
        .unwrap();
    let jar2 = Jar::from(&cookies);
    age_rotations(&pool, &user).await;
    let (status, ..) = refresh(&app, &jar1.refresh, &jar1.csrf, None).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    assert!(!sessions.family_alive(family).await.unwrap());
    assert_eq!(me(&app, &jar2).await, StatusCode::UNAUTHORIZED);
}
