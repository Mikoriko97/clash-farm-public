//! cf-admin sign-in, kill switches, bounded settings, the strict CSP and the app shell.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use common::*;
use ed25519_dalek::SigningKey;
use serde_json::{json, Value};
use sqlx::Row;
use tower::ServiceExt;
use uuid::Uuid;

#[tokio::test]
async fn test_only_allowlisted_wallets_sign_in_and_the_api_needs_a_session() {
    let h = harness().await;
    let stranger = SigningKey::from_bytes(&rand_bytes());
    let wallet = format!(
        "sol:{}",
        bs58::encode(stranger.verifying_key().to_bytes()).into_string()
    );
    assert_eq!(
        login(&h, &stranger, &wallet).await.unwrap_err(),
        StatusCode::FORBIDDEN
    );
    // The right wallet with the wrong key: signature check fails.
    assert_eq!(
        login(&h, &stranger, &h.wallet).await.unwrap_err(),
        StatusCode::UNAUTHORIZED
    );
    // The app shell sends strangers to the sign-in page; the API answers with JSON 401.
    let (status, body) = get(&h.app, "", "/").await;
    assert_eq!(status, StatusCode::SEE_OTHER, "{body}");
    assert!(body.contains("LOCATION:/login"));
    let (status, body) = get(&h.app, "", "/players").await;
    assert_eq!(status, StatusCode::SEE_OTHER, "{body}");
    let (status, body) = get_json(&h.app, "", "/api/me").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED, "{body}");
    assert_eq!(body["error"]["code"], "unauthorized");
    let (status, _) = get_json(&h.app, "", "/api/nope").await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "unknown API paths leak nothing without a session"
    );
}

#[tokio::test]
async fn test_kill_switch_toggle_is_audited_and_csrf_bound() {
    let h = harness().await;
    let (cookie, csrf) = login(&h, &h.key, &h.wallet).await.unwrap();
    let (status, me) = get_json(&h.app, &cookie, "/api/me").await;
    assert_eq!(status, StatusCode::OK, "{me}");
    assert_eq!(me["wallet"], h.wallet);
    assert_eq!(me["csrf"], csrf);
    let (status, dash) = get_json(&h.app, &cookie, "/api/dashboard").await;
    assert_eq!(status, StatusCode::OK, "{dash}");
    assert!(
        dash["switches"]
            .as_array()
            .unwrap()
            .iter()
            .any(|s| s["key"] == "shop_enabled"),
        "{dash}"
    );
    // A foreign Origin is refused even with the right token (defence in depth next to CSRF).
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/settings")
        .header(header::COOKIE, &cookie)
        .header("x-csrf", &csrf)
        .header(header::HOST, "127.0.0.1:9100")
        .header(header::ORIGIN, "http://localhost:5173")
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(
            json!({ "key": "shop_enabled", "value": "false" }).to_string(),
        ))
        .unwrap();
    let (status, body, _) = read(h.app.clone().oneshot(req).await.unwrap()).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(setting(&h.pool, "shop_enabled").await, Some(Value::Bool(true)));
    // Missing / wrong CSRF token is refused before anything changes.
    let (status, body) = post_json(
        &h.app,
        &cookie,
        "nope",
        "/api/settings",
        json!({ "key": "shop_enabled", "value": "false" }),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(body["error"]["code"], "forbidden");
    assert_eq!(setting(&h.pool, "shop_enabled").await, Some(Value::Bool(true)));
    let (status, body) = post_json(
        &h.app,
        &cookie,
        &csrf,
        "/api/settings",
        json!({ "key": "shop_enabled", "value": "false" }),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["before"], Value::Bool(true));
    assert_eq!(body["after"], Value::Bool(false));
    assert_eq!(setting(&h.pool, "shop_enabled").await, Some(Value::Bool(false)));
    let row = sqlx::query(
        "SELECT actor, before, after FROM cf.audit_log WHERE action = 'setting' AND target = 'shop_enabled' ORDER BY id DESC LIMIT 1",
    )
    .fetch_one(&h.pool)
    .await
    .unwrap();
    assert_eq!(row.try_get::<String, _>("actor").unwrap(), h.wallet);
    assert_eq!(row.try_get::<Value, _>("before").unwrap(), Value::Bool(true));
    assert_eq!(row.try_get::<Value, _>("after").unwrap(), Value::Bool(false));
    let (_, dash) = get_json(&h.app, &cookie, "/api/dashboard").await;
    assert!(
        dash["alerts"]
            .as_array()
            .unwrap()
            .iter()
            .any(|a| a["text"].as_str().unwrap().contains("shop_enabled")),
        "{dash}"
    );
    set_setting(&h.pool, "shop_enabled", Value::Bool(true)).await;
    // Logout kills the session.
    let (status, _) = post_json(&h.app, &cookie, &csrf, "/api/logout", Value::Null).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    let (status, _) = get_json(&h.app, &cookie, "/api/me").await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_settings_are_bounded_and_typed() {
    let h = harness().await;
    let (cookie, csrf) = login(&h, &h.key, &h.wallet).await.unwrap();
    let set = |key: &'static str, value: &'static str| {
        let (app, cookie, csrf) = (h.app.clone(), cookie.clone(), csrf.clone());
        async move {
            post_json(
                &app,
                &cookie,
                &csrf,
                "/api/settings",
                json!({ "key": key, "value": value }),
            )
            .await
            .0
        }
    };
    assert_eq!(set("economy.sleep_xp", "99999").await, StatusCode::BAD_REQUEST);
    assert_eq!(set("economy.sleep_xp", "abc").await, StatusCode::BAD_REQUEST);
    assert_eq!(
        set("economy.burn_share_bp", "5000").await,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(set("weather.force", "hail").await, StatusCode::BAD_REQUEST);
    assert_eq!(set("economy.credits_cap_per_level", "150").await, StatusCode::OK);
    assert_eq!(
        set("economy.rested_bonus_pct", "101").await,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        set("economy.rested_minutes", "1441").await,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(
        set("economy.rested_bonus_pct", "-1").await,
        StatusCode::BAD_REQUEST
    );
    assert_eq!(set("economy.sleep_xp", "12").await, StatusCode::OK);
    assert_eq!(setting(&h.pool, "economy.sleep_xp").await, Some(Value::from(12)));
    assert_eq!(set("weather.force", "auto").await, StatusCode::OK);
    assert_eq!(setting(&h.pool, "weather.force").await, Some(Value::Null));
    let (status, body) = get_json(&h.app, &cookie, "/api/settings").await;
    assert_eq!(status, StatusCode::OK);
    let sleep = body
        .as_array()
        .unwrap()
        .iter()
        .find(|s| s["key"] == "economy.sleep_xp")
        .unwrap();
    assert_eq!(sleep["value"], 12);
    assert_eq!(sleep["kind"], "int");
    assert_eq!(sleep["updated_by"], h.wallet);
    sqlx::query("DELETE FROM cf.settings WHERE key IN ('economy.sleep_xp', 'weather.force')")
        .execute(&h.pool)
        .await
        .unwrap();
    let (status, catalog) = get_json(&h.app, &cookie, "/api/catalog").await;
    assert_eq!(status, StatusCode::OK);
    let wheat = catalog
        .as_array()
        .unwrap()
        .iter()
        .find(|i| i["id"] == "seed_wheat")
        .unwrap();
    assert_eq!(wheat["price_credits"], 5);
}

#[tokio::test]
async fn test_csp_stays_strict_and_the_shell_explains_a_missing_build() {
    let h = harness().await;
    let (cookie, _) = login(&h, &h.key, &h.wallet).await.unwrap();
    let req = Request::builder()
        .uri("/")
        .header(header::COOKIE, &cookie)
        .body(Body::empty())
        .unwrap();
    let response = h.app.clone().oneshot(req).await.unwrap();
    let csp = response
        .headers()
        .get("content-security-policy")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    assert_eq!(
        csp,
        "default-src 'none'; script-src 'self'; style-src 'self'; connect-src 'self'; form-action 'self'; base-uri 'none'; frame-ancestors 'none'"
    );
    let (status, body, _) = read(response).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        body.contains("build:admin"),
        "no CF_ADMIN_STATIC_DIR in tests: {body}"
    );
    assert!(
        !body.contains(" style=\""),
        "inline styles are blocked by the CSP: {body}"
    );
    let (status, body) = get(&h.app, &cookie, "/login").await;
    assert_eq!(status, StatusCode::OK);
    assert!(!body.contains("<script>") && body.contains("/login.js"), "{body}");
}

#[tokio::test]
async fn test_built_app_is_served_only_with_a_session_and_assets_stay_inside() {
    let dir = std::env::temp_dir().join(format!("cf-admin-static-{}", Uuid::new_v4()));
    std::fs::create_dir_all(dir.join("assets")).unwrap();
    std::fs::write(
        dir.join("admin.html"),
        "<!doctype html><title>shell</title><div id=\"root\"></div>",
    )
    .unwrap();
    std::fs::write(dir.join("assets").join("app.css"), "body{}").unwrap();
    let h = harness_with(Some(&dir)).await;
    let (status, body) = get(&h.app, "", "/").await;
    assert_eq!(status, StatusCode::SEE_OTHER, "no session → sign-in: {body}");
    let (status, body) = get(&h.app, "", "/assets/app.css").await;
    assert_eq!(status, StatusCode::OK, "hashed build files are public: {body}");
    let (status, _) = get(&h.app, "", "/assets/../admin.html").await;
    assert_ne!(status, StatusCode::OK, "no path traversal out of assets");
    let (cookie, _) = login(&h, &h.key, &h.wallet).await.unwrap();
    for path in ["/", "/players/abc", "/economy/catalog"] {
        let (status, body) = get(&h.app, &cookie, path).await;
        assert_eq!(status, StatusCode::OK, "{path}: {body}");
        assert!(body.contains("id=\"root\""), "{path} serves the shell: {body}");
    }
    let (status, _) = get(&h.app, &cookie, "/favicon.ico").await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    std::fs::remove_dir_all(&dir).unwrap();
}
