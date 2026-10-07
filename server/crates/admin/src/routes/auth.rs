//! Admin sign-in: the same nonce → signed message → verify flow as the game, restricted
//! to the allowlisted Solana wallets, plus a second factor (ADR-023): a wallet with a
//! TOTP secret gets a five-minute pending cookie after its signature and must confirm a
//! fresh code before any session exists.

use axum::extract::{Query, State};
use axum::http::header::CONTENT_TYPE;
use axum::response::{Html, IntoResponse, Redirect};
use axum::{Form, Json};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use cf_auth::{verify, AuthError, SignInMessage, WalletTag};
use cf_common::Env;
use serde::Deserialize;
use serde_json::{json, Value};
use time::OffsetDateTime;

use crate::error::AdminError;
use crate::html::{self, LOGIN_JS};
use crate::session::{Admin, COOKIE, PENDING_COOKIE, PENDING_TTL_SECS, TTL_SECS};
use crate::state::{now_ms, AdminState};
use crate::{audit, CsrfForm};

/// No proxy sits in front of cf-admin (SSH tunnel), so client IPs are meaningless:
/// the limits key on the wallet and on the whole binary instead.
const LOGIN_LIMIT_PER_WALLET: u64 = 10;
const LOGIN_LIMIT_TOTAL: u64 = 60;
/// Code guesses per wallet per minute: 6 digits and a ±1-step window make brute force
/// at this rate hopeless, and every refusal is logged.
const TOTP_LIMIT_PER_WALLET: u64 = 5;
/// And per day (successes count too): six digits over a ±1-step window are three valid
/// codes in a million, so a per-minute limit alone would let a thief holding the wallet
/// key expect a hit within weeks.
const TOTP_LIMIT_PER_DAY: u64 = 50;
const WINDOW_SECS: u64 = 60;
const DAY_SECS: u64 = 24 * 60 * 60;

pub async fn login_page() -> Html<String> {
    html::page(
        "Вхід",
        "<p><small>Підпиши повідомлення адмін-гаманцем із allowlist. Доступ лише через SSH-тунель.</small></p>\
<p><button id=\"login\" type=\"button\">Увійти гаманцем</button></p>\
<form id=\"totp\" hidden><p><small>Другий фактор: код із застосунку-автентифікатора.</small></p>\
<p><input id=\"code\" name=\"code\" inputmode=\"numeric\" autocomplete=\"one-time-code\" pattern=\"[0-9]{6}\" maxlength=\"6\" required placeholder=\"000000\"></p>\
<p><button type=\"submit\">Підтвердити код</button></p></form>\
<p id=\"msg\" class=\"err\" hidden></p><script src=\"/login.js\"></script>",
    )
}

pub async fn login_js() -> impl IntoResponse {
    (
        [(CONTENT_TYPE, "application/javascript; charset=utf-8")],
        LOGIN_JS,
    )
}

pub async fn css() -> impl IntoResponse {
    ([(CONTENT_TYPE, "text/css; charset=utf-8")], html::CSS)
}

async fn limit_login(state: &AdminState, wallet: &str) -> Result<(), AdminError> {
    state
        .limiter
        .hit_strict("admin_login_all", "all", LOGIN_LIMIT_TOTAL, WINDOW_SECS, now_ms())
        .await?;
    state
        .limiter
        .hit_strict(
            "admin_login_wallet",
            wallet,
            LOGIN_LIMIT_PER_WALLET,
            WINDOW_SECS,
            now_ms(),
        )
        .await?;
    Ok(())
}

/// Allowlist check before anything else; refusals are logged, never silent.
fn admin_wallet(state: &AdminState, raw: &str) -> Result<WalletTag, AdminError> {
    let wallet = WalletTag::parse(raw)?;
    if !state.config.is_admin(&wallet) {
        tracing::warn!(wallet = %wallet, "admin_login_denied: wallet not in allowlist");
        return Err(AdminError::Forbidden("wallet is not an admin"));
    }
    Ok(wallet)
}

#[derive(Debug, Deserialize)]
pub struct NonceQuery {
    pub wallet: String,
}

/// `GET /login/nonce?wallet=sol:<base58>` → `{ nonce, message }`.
pub async fn nonce(
    State(state): State<AdminState>,
    Query(query): Query<NonceQuery>,
) -> Result<Json<Value>, AdminError> {
    limit_login(&state, &query.wallet).await?;
    let wallet = admin_wallet(&state, &query.wallet)?;
    let nonce = state.nonces.issue(&wallet.to_string()).await?;
    let (domain, uri) = state.admin_origin();
    let message =
        SignInMessage::new(&domain, &wallet, None, &nonce, OffsetDateTime::now_utc()).with_uri(&uri);
    Ok(Json(json!({ "nonce": nonce, "message": message.render() })))
}

#[derive(Deserialize)]
pub struct VerifyBody {
    pub wallet: String,
    /// Hex-encoded ed25519 signature.
    pub signature: String,
    pub message: String,
}

impl std::fmt::Debug for VerifyBody {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VerifyBody")
            .field("wallet", &self.wallet)
            .finish_non_exhaustive()
    }
}

/// `POST /login/verify` → session cookie + `{ ok, csrf }`, or — for a wallet with a
/// TOTP secret — a pending cookie + `{ ok, totp: true }` and no session yet.
pub async fn verify(
    State(state): State<AdminState>,
    jar: CookieJar,
    Json(body): Json<VerifyBody>,
) -> Result<(CookieJar, Json<Value>), AdminError> {
    limit_login(&state, &body.wallet).await?;
    let wallet = admin_wallet(&state, &body.wallet)?;
    let message = SignInMessage::parse(&body.message)?;
    let (domain, uri) = state.admin_origin();
    message.validate_with_uri(&domain, &uri, &wallet, &[], OffsetDateTime::now_utc())?;
    state.nonces.consume(&message.nonce, &wallet.to_string()).await?;
    let signature = hex::decode(&body.signature).map_err(|_| AuthError::BadSignature)?;
    verify::verify(&wallet, body.message.as_bytes(), &signature)?;
    let wallet = wallet.to_string();
    if state.config.totp_secret(&wallet).is_some() {
        let token = state.sessions.issue_pending(&wallet).await?;
        tracing::info!(wallet, "admin_login_totp_pending");
        let cookie = base_cookie(&state, PENDING_COOKIE, token, "/login", PENDING_TTL_SECS);
        return Ok((jar.add(cookie), Json(json!({ "ok": true, "totp": true }))));
    }
    if state.config.totp_required {
        // Config refuses to start in this state; kept so no later config path opens a hole.
        tracing::error!(
            wallet,
            "admin_login_denied: totp required but no secret configured"
        );
        return Err(AdminError::Forbidden("second factor required for this wallet"));
    }
    issue_session(&state, jar, &wallet, &["wallet"]).await
}

#[derive(Deserialize)]
pub struct TotpBody {
    pub code: String,
}

impl std::fmt::Debug for TotpBody {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("TotpBody(***)")
    }
}

/// `POST /login/totp { code }` with the pending cookie → session cookie + `{ ok, csrf }`.
/// A code is accepted once per time step; guesses are limited per wallet and logged.
pub async fn totp(
    State(state): State<AdminState>,
    jar: CookieJar,
    Json(body): Json<TotpBody>,
) -> Result<(CookieJar, Json<Value>), AdminError> {
    let token = jar
        .get(PENDING_COOKIE)
        .map(|c| c.value().to_owned())
        .ok_or(AdminError::Forbidden("sign in with the wallet first"))?;
    let wallet = state
        .sessions
        .pending_wallet(&token)
        .await?
        .ok_or(AdminError::Forbidden("sign-in expired; start again"))?;
    state
        .limiter
        .hit_strict(
            "admin_totp",
            &wallet,
            TOTP_LIMIT_PER_WALLET,
            WINDOW_SECS,
            now_ms(),
        )
        .await?;
    state
        .limiter
        .hit_strict("admin_totp_day", &wallet, TOTP_LIMIT_PER_DAY, DAY_SECS, now_ms())
        .await?;
    let secret = state
        .config
        .totp_secret(&wallet)
        .ok_or(AdminError::Forbidden("no second factor for this wallet"))?;
    let now = OffsetDateTime::now_utc().unix_timestamp();
    let Some(step) = secret.verify(body.code.trim(), now) else {
        return Err(refuse_code(&state, &wallet, "code").await);
    };
    if !state.sessions.claim_totp_step(&wallet, step).await? {
        return Err(refuse_code(&state, &wallet, "replay").await);
    }
    state.sessions.consume_pending(&token).await?;
    let jar = jar.remove(base_cookie(&state, PENDING_COOKIE, String::new(), "/login", 0));
    issue_session(&state, jar, &wallet, &["wallet", "totp"]).await
}

/// A refused code is logged and audited (`admin_totp_rejected`) so the alerts job can raise
/// `totp_rejected` when someone holding a wallet key starts guessing.
async fn refuse_code(state: &AdminState, wallet: &str, reason: &'static str) -> AdminError {
    tracing::warn!(wallet, reason, "admin_totp_rejected");
    let audited: Result<(), sqlx::Error> = async {
        let mut conn = state.pool.acquire().await?;
        audit::record(
            &mut conn,
            wallet,
            "admin_totp_rejected",
            None,
            Value::Null,
            json!({ "reason": reason }),
            None,
        )
        .await
    }
    .await;
    if let Err(err) = audited {
        tracing::error!(wallet, error = %err, "admin_totp_rejected: audit row not written");
    }
    AdminError::Forbidden("code rejected")
}

/// The session itself: cookie, audit row with the factors that opened it, CSRF token.
async fn issue_session(
    state: &AdminState,
    jar: CookieJar,
    wallet: &str,
    factors: &[&str],
) -> Result<(CookieJar, Json<Value>), AdminError> {
    let issued = state.sessions.issue(wallet).await?;
    let mut conn = state.pool.acquire().await?;
    audit::record(
        &mut conn,
        wallet,
        "admin_login",
        None,
        Value::Null,
        json!({ "factors": factors }),
        None,
    )
    .await?;
    tracing::info!(wallet, ?factors, "admin_login");
    let cookie = base_cookie(state, COOKIE, issued.token, "/", TTL_SECS);
    Ok((jar.add(cookie), Json(json!({ "ok": true, "csrf": issued.csrf }))))
}

fn base_cookie(
    state: &AdminState,
    name: &'static str,
    value: String,
    path: &'static str,
    max_age_secs: u64,
) -> Cookie<'static> {
    let mut cookie = Cookie::new(name, value);
    cookie.set_http_only(true);
    cookie.set_same_site(SameSite::Strict);
    cookie.set_path(path);
    cookie.set_secure(state.config.env != Env::Local);
    cookie.set_max_age(time::Duration::seconds(
        i64::try_from(max_age_secs).unwrap_or(1800),
    ));
    cookie
}

/// `POST /logout` (form with csrf).
pub async fn logout(
    State(state): State<AdminState>,
    admin: Admin,
    jar: CookieJar,
    Form(form): Form<CsrfForm>,
) -> Result<(CookieJar, Redirect), AdminError> {
    admin.check_csrf(&form.csrf)?;
    state.sessions.revoke(&admin.token).await?;
    let mut gone = Cookie::new(COOKIE, "");
    gone.set_path("/");
    Ok((jar.remove(gone), Redirect::to("/login")))
}
