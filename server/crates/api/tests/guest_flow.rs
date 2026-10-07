//! Guest islands (migration 0028, owner ok 2026-10-02): play without a wallet, link one later with a signature in
//! the same session and keep the island; never merge two accounts; guests cannot pay; creation is capped per IP
//! (the switch is in guest_switch.rs). Against local Postgres + Memurai.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use axum::body::Body;
use axum::http::{header, Method, Request, StatusCode};
use axum::Router;
use common::{app, app_with, cookie_value, fresh_ip, get, guest, json, post, rand_bytes, Session};
use ed25519_dalek::{Signer, SigningKey};
use serde_json::{json, Value};
use tower::ServiceExt;

/// A fresh Solana key and a signed sign-in message for it (nonce from the server).
async fn signed(app: &Router, key: &SigningKey) -> Value {
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
    json!({ "wallet": wallet, "signature": signature, "message": message })
}

/// `POST /auth/link` in `session`: status, body and, on success, the fresh session the link issues (every
/// earlier session of the guest is revoked).
async fn link(app: &Router, session: &Session, payload: Value) -> (StatusCode, Value, Option<Session>) {
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/auth/link")
        .header(header::COOKIE, session.cookie())
        .header("x-forwarded-for", &session.ip)
        .header("x-csrf-token", &session.csrf)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(payload.to_string()))
        .unwrap();
    let (status, body, cookies) = json(app.clone().oneshot(req).await.unwrap()).await;
    let fresh = (status == StatusCode::OK).then(|| Session {
        user_id: session.user_id.clone(),
        wallet: body["user"]["wallet"].as_str().unwrap().to_owned(),
        access: cookie_value(&cookies, "cf_access").unwrap(),
        csrf: cookie_value(&cookies, "cf_csrf").unwrap(),
        ip: session.ip.clone(),
    });
    (status, body, fresh)
}

/// Plain wallet login with `key` (no guest): returns the user id.
async fn login_with(app: &Router, key: &SigningKey) -> String {
    let payload = signed(app, key).await;
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/auth/verify")
        .header("x-forwarded-for", fresh_ip())
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(payload.to_string()))
        .unwrap();
    let (status, body, _) = json(app.clone().oneshot(req).await.unwrap()).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    body["user"]["id"].as_str().unwrap().to_owned()
}

#[tokio::test]
async fn test_guest_plays_without_a_wallet() {
    let (app, _) = app().await;
    let (status, body, session) = guest(&app, &fresh_ip()).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body["user"]["wallet"].is_null(), "a guest has no wallet");
    let session = session.unwrap();
    let (status, me) = get(&app, &session, "/me").await;
    assert_eq!(status, StatusCode::OK, "{me}");
    assert!(me["user"]["wallet"].is_null());
    let (status, island) = get(&app, &session, "/island").await;
    assert_eq!(status, StatusCode::OK, "{island}");
    let plot = island["plots"][0]["id"].as_str().unwrap().to_owned();
    let (status, body) = post(
        &app,
        &session,
        &format!("/plots/{plot}/plant"),
        Some(json!({ "item_id": "seed_radish" })),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "a guest farms like anyone: {body}");
}

#[tokio::test]
async fn test_guest_creation_is_capped_per_ip_per_day() {
    let (app, _) = app().await;
    let ip = fresh_ip();
    for n in 0..3 {
        let (status, body, _) = guest(&app, &ip).await;
        assert_eq!(status, StatusCode::OK, "guest {n}: {body}");
    }
    let (status, body, _) = guest(&app, &ip).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "{body}");
    let (status, _, _) = guest(&app, &fresh_ip()).await;
    assert_eq!(status, StatusCode::OK, "another address is not affected");
}

#[tokio::test]
async fn test_linking_a_wallet_keeps_the_island_and_signs_in_later() {
    let (app, _) = app().await;
    let (_, _, session) = guest(&app, &fresh_ip()).await;
    let session = session.unwrap();
    let (_, island) = get(&app, &session, "/island").await;
    let key = SigningKey::from_bytes(&rand_bytes());
    let payload = signed(&app, &key).await;
    let (status, body, linked) = link(&app, &session, payload.clone()).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["user"]["wallet"], payload["wallet"]);
    // A new wallet is a new login: the guest cookies stop working, the fresh ones carry the wallet.
    let (status, _) = get(&app, &session, "/me").await;
    assert_eq!(
        status,
        StatusCode::UNAUTHORIZED,
        "the old guest session is revoked"
    );
    let linked = linked.unwrap();
    let (_, me) = get(&app, &linked, "/me").await;
    assert_eq!(
        me["user"]["wallet"], payload["wallet"],
        "the account now has the wallet"
    );
    let (_, again) = get(&app, &linked, "/island").await;
    assert_eq!(
        again["island"]["id"], island["island"]["id"],
        "the island is kept"
    );
    // Later the wallet signs in on its own and lands on the same account.
    assert_eq!(login_with(&app, &key).await, session.user_id);
}

#[tokio::test]
async fn test_link_never_merges_accounts_and_happens_once() {
    let (app, _) = app().await;
    let owned = SigningKey::from_bytes(&rand_bytes());
    login_with(&app, &owned).await;
    let (_, _, session) = guest(&app, &fresh_ip()).await;
    let session = session.unwrap();
    let payload = signed(&app, &owned).await;
    let (status, body) = post(&app, &session, "/auth/link", Some(payload), None).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["error"]["code"], "wallet_has_account");
    let first = SigningKey::from_bytes(&rand_bytes());
    let payload = signed(&app, &first).await;
    let (status, body, linked) = link(&app, &session, payload).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let second = SigningKey::from_bytes(&rand_bytes());
    let payload = signed(&app, &second).await;
    let (status, body, _) = link(&app, &linked.unwrap(), payload).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["error"]["code"], "already_linked");
    // A forged signature never links (and burns the nonce).
    let (_, _, other) = guest(&app, &fresh_ip()).await;
    let other = other.unwrap();
    let mut payload = signed(&app, &SigningKey::from_bytes(&rand_bytes())).await;
    payload["signature"] = Value::from(hex::encode([7u8; 64]));
    let (status, _, _) = link(&app, &other, payload).await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
}

#[tokio::test]
async fn test_parallel_links_of_one_wallet_give_it_one_farm() {
    let (app, _) = app().await;
    let key = SigningKey::from_bytes(&rand_bytes());
    let a = guest(&app, &fresh_ip()).await.2.unwrap();
    let b = guest(&app, &fresh_ip()).await.2.unwrap();
    let (pa, pb) = (signed(&app, &key).await, signed(&app, &key).await);
    let ((sa, ba, _), (sb, bb, _)) = tokio::join!(link(&app, &a, pa), link(&app, &b, pb));
    let mut outcomes = [
        (sa, ba["error"]["code"].clone()),
        (sb, bb["error"]["code"].clone()),
    ];
    outcomes.sort_by_key(|(status, _)| status.as_u16());
    assert_eq!(outcomes[0].0, StatusCode::OK, "{outcomes:?}");
    assert_eq!(outcomes[1].0, StatusCode::CONFLICT, "{outcomes:?}");
    assert_eq!(outcomes[1].1, "wallet_has_account");
    let owner = login_with(&app, &key).await;
    assert!(
        owner == a.user_id || owner == b.user_id,
        "the wallet signs in to the winner"
    );
}

#[tokio::test]
async fn test_parallel_links_of_one_guest_take_one_wallet() {
    let (app, _) = app().await;
    let g = guest(&app, &fresh_ip()).await.2.unwrap();
    let (k1, k2) = (
        SigningKey::from_bytes(&rand_bytes()),
        SigningKey::from_bytes(&rand_bytes()),
    );
    let (p1, p2) = (signed(&app, &k1).await, signed(&app, &k2).await);
    let ((s1, b1, _), (s2, b2, _)) = tokio::join!(link(&app, &g, p1), link(&app, &g, p2));
    assert!(
        (s1 == StatusCode::OK) ^ (s2 == StatusCode::OK),
        "exactly one wins: {s1} {s2}"
    );
    let (loser_status, loser_body, loser_key) = if s1 == StatusCode::OK {
        (s2, b2, &k2)
    } else {
        (s1, b1, &k1)
    };
    // The loser either saw the wallet already there or the session already rotated by the winner.
    assert!(
        (loser_status == StatusCode::CONFLICT && loser_body["error"]["code"] == "already_linked")
            || loser_status == StatusCode::UNAUTHORIZED,
        "{loser_status} {loser_body}"
    );
    assert_ne!(
        login_with(&app, loser_key).await,
        g.user_id,
        "the losing wallet is not on the guest farm"
    );
}

#[tokio::test]
async fn test_cross_site_pages_cannot_create_guests() {
    let (app, _) = app().await;
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/auth/guest")
        .header("x-forwarded-for", fresh_ip())
        .header(header::ORIGIN, "https://evil.example")
        .body(Body::empty())
        .unwrap();
    let (status, body, cookies) = json(app.clone().oneshot(req).await.unwrap()).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert!(cookies.is_empty(), "no session for a cross-site page");
}

#[tokio::test]
async fn test_a_banned_guest_cannot_link() {
    let (app, pool) = app().await;
    let g = guest(&app, &fresh_ip()).await.2.unwrap();
    sqlx::query("UPDATE cf.users SET banned_at = now(), ban_reason = 'test' WHERE id = $1::uuid")
        .bind(&g.user_id)
        .execute(&pool)
        .await
        .unwrap();
    let payload = signed(&app, &SigningKey::from_bytes(&rand_bytes())).await;
    let (status, body, _) = link(&app, &g, payload).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(body["error"]["code"], "banned");
    let wallet: Option<String> = sqlx::query_scalar("SELECT wallet FROM cf.users WHERE id = $1::uuid")
        .bind(&g.user_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert!(wallet.is_none(), "nothing was linked");
}

#[tokio::test]
async fn test_link_needs_the_csrf_token() {
    let (app, _) = app().await;
    let (_, _, session) = guest(&app, &fresh_ip()).await;
    let session = session.unwrap();
    let payload = signed(&app, &SigningKey::from_bytes(&rand_bytes())).await;
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/auth/link")
        .header(header::COOKIE, session.cookie())
        .header("x-forwarded-for", &session.ip)
        .header(header::CONTENT_TYPE, "application/json")
        .body(Body::from(payload.to_string()))
        .unwrap();
    let (status, _, _) = json(app.clone().oneshot(req).await.unwrap()).await;
    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "a cross-site form cannot link a wallet"
    );
}

#[tokio::test]
async fn test_guest_cannot_pay_with_clash() {
    let (app, _) = app_with(true).await;
    let (_, _, session) = guest(&app, &fresh_ip()).await;
    let session = session.unwrap();
    get(&app, &session, "/island").await;
    let (status, body) = post(
        &app,
        &session,
        "/orders",
        Some(json!({ "item_id": "decor_farm_sign", "qty": 1 })),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{body}");
    assert_eq!(body["error"]["code"], "wallet_required");
}

#[tokio::test]
async fn test_a_signed_in_browser_does_not_spawn_a_second_guest() {
    let (app, _) = app().await;
    let ip = fresh_ip();
    let (_, _, session) = guest(&app, &ip).await;
    let session = session.unwrap();
    // The same browser (its cookies) asks again: a second tab, a double click.
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/auth/guest")
        .header("x-forwarded-for", &ip)
        .header(header::COOKIE, session.cookie())
        .body(Body::empty())
        .unwrap();
    let (status, body, cookies) = json(app.clone().oneshot(req).await.unwrap()).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["error"]["code"], "session_exists");
    assert!(cookies.is_empty(), "the live session is left as it is");
    // A stale or forged access cookie does not block a new guest.
    let stale = Session {
        access: "not-a-token".into(),
        ..session
    };
    let req = Request::builder()
        .method(Method::POST)
        .uri("/api/v1/auth/guest")
        .header("x-forwarded-for", &ip)
        .header(header::COOKIE, stale.cookie())
        .body(Body::empty())
        .unwrap();
    let (status, body, _) = json(app.clone().oneshot(req).await.unwrap()).await;
    assert_eq!(status, StatusCode::OK, "{body}");
}
