//! Sign-in flow (docs/API.md §Auth, docs/SECURITY.md §3):
//! nonce → wallet signs the server-built message → verify → cookie session.
//! Guests (0028): `POST /auth/guest` gives a session without a wallet; `POST /auth/link` later attaches a wallet
//! proven by the same signed message, so the island is kept.

use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use cf_auth::message::VALIDITY;
use cf_auth::session::{ClientFingerprint, IssuedSession, ACCESS_TTL, REFRESH_TTL};
use cf_auth::{user, verify, AuthError, SignInMessage, WalletTag};
use serde::{Deserialize, Serialize};
use time::format_description::well_known::Rfc3339;
use time::OffsetDateTime;

use crate::error::ApiError;
use crate::middleware::csrf::CSRF_COOKIE;
use crate::middleware::session::{AuthUser, ACCESS_COOKIE, REFRESH_COOKIE};
use crate::middleware::{client_ip, fingerprint, ip_bucket};
use crate::state::AppState;

/// SECURITY §3: 10 req/min per IP on every `/auth/*`, 5/min per wallet on verify.
const AUTH_LIMIT_PER_IP: u64 = 10;
const VERIFY_LIMIT_PER_WALLET: u64 = 5;
const WINDOW_SECS: u64 = 60;
/// Guest islands one IP may create per day (0028): enough for a household, not for a farm of accounts.
const GUESTS_PER_IP_DAY: u64 = 3;
const DAY_SECS: u64 = 86_400;

async fn limit_ip(state: &AppState, headers: &HeaderMap) -> Result<(), ApiError> {
    state
        .limiter
        .hit(
            "auth_ip",
            &client_ip(headers),
            AUTH_LIMIT_PER_IP,
            WINDOW_SECS,
            now_ms(),
        )
        .await?;
    Ok(())
}

#[derive(Debug, Deserialize)]
pub struct NonceQuery {
    pub wallet: String,
    pub chain_id: Option<u64>,
}

#[derive(Debug, Serialize)]
pub struct NonceResponse {
    pub nonce: String,
    pub message: String,
    pub expires_at: String,
}

/// `GET /auth/nonce?wallet=<tag>&chain_id=<evm id>`
pub async fn nonce(
    State(state): State<AppState>,
    headers: HeaderMap,
    Query(query): Query<NonceQuery>,
) -> Result<Json<NonceResponse>, ApiError> {
    limit_ip(&state, &headers).await?;
    let wallet = WalletTag::parse(&query.wallet)?;
    if let Some(chain) = query.chain_id {
        if !state.config.evm_chain_ids.contains(&chain) {
            return Err(AuthError::ChainNotAllowed.into());
        }
    }
    let nonce = state.nonces.issue(&wallet.to_string()).await?;
    let now = OffsetDateTime::now_utc();
    let message = SignInMessage::new(&state.config.siws_domain, &wallet, query.chain_id, &nonce, now);
    let expires_at = (now + VALIDITY).format(&Rfc3339).unwrap_or_default();
    Ok(Json(NonceResponse {
        nonce,
        message: message.render(),
        expires_at,
    }))
}

#[derive(Deserialize)]
pub struct VerifyRequest {
    pub wallet: String,
    /// Hex-encoded raw signature (64 bytes ed25519 / 65 bytes secp256k1).
    pub signature: String,
    pub message: String,
}

/// Auth request bodies must never reach logs (SECURITY §8).
impl std::fmt::Debug for VerifyRequest {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("VerifyRequest")
            .field("wallet", &self.wallet)
            .finish_non_exhaustive()
    }
}

#[derive(Debug, Serialize)]
pub struct SessionResponse {
    pub user: UserSummary,
}

#[derive(Debug, Serialize)]
pub struct UserSummary {
    pub id: uuid::Uuid,
    /// `None` for a guest (0028) until a wallet is linked.
    pub wallet: Option<String>,
}

/// Checks a signed sign-in message end to end and returns the wallet it proves: per-wallet limit, message
/// layout and domain, one-time nonce (burnt before the signature check), signature. Used by login and by linking.
async fn proven_wallet(state: &AppState, body: &VerifyRequest) -> Result<WalletTag, ApiError> {
    let wallet = WalletTag::parse(&body.wallet)?;
    state
        .limiter
        .hit(
            "verify",
            &wallet.to_string(),
            VERIFY_LIMIT_PER_WALLET,
            WINDOW_SECS,
            now_ms(),
        )
        .await?;
    let message = SignInMessage::parse(&body.message)?;
    message.validate(
        &state.config.siws_domain,
        &wallet,
        &state.config.evm_chain_ids,
        OffsetDateTime::now_utc(),
    )?;
    // Burn the nonce before checking the signature: a wrong signature costs a new nonce.
    if let Err(err) = state.nonces.consume(&message.nonce, &wallet.to_string()).await {
        if matches!(err, AuthError::NonceInvalid) {
            tracing::warn!(wallet = %wallet, "auth_nonce_reuse: nonce missing, expired or already used");
        }
        return Err(err.into());
    }
    let signature = hex::decode(&body.signature).map_err(|_| AuthError::BadSignature)?;
    verify::verify(&wallet, body.message.as_bytes(), &signature)?;
    Ok(wallet)
}

/// `POST /auth/verify`
pub async fn verify(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: CookieJar,
    Json(body): Json<VerifyRequest>,
) -> Result<(CookieJar, Json<SessionResponse>), ApiError> {
    limit_ip(&state, &headers).await?;
    let wallet = proven_wallet(&state, &body).await?;
    let user_id = user::upsert_by_wallet(&state.pool, &wallet).await?;
    let issued = state
        .sessions
        .login(user_id, &fingerprint(&headers, state.config.csrf_secret.expose()))
        .await?;
    tracing::info!(%user_id, kind = ?wallet.kind, "login");
    let jar = set_session_cookies(jar, &issued);
    Ok((
        jar,
        Json(SessionResponse {
            user: UserSummary {
                id: user_id,
                wallet: Some(wallet.to_string()),
            },
        }),
    ))
}

/// `POST /auth/guest` (0028): a new island without a wallet and a cookie session for it. Capped per IP per day;
/// Origin is checked by the CSRF layer like every login, the token is not (a new visitor has none yet).
pub async fn guest(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: CookieJar,
) -> Result<(CookieJar, Json<SessionResponse>), ApiError> {
    limit_ip(&state, &headers).await?;
    // A live session in this browser (a second tab, a double click) keeps its island: a new guest
    // would silently swap the player onto an empty farm. The client reloads /me instead.
    if let Some(token) = jar.get(ACCESS_COOKIE) {
        if state.sessions.authenticate(token.value()).await?.is_some() {
            return Err(ApiError::new(
                StatusCode::CONFLICT,
                "session_exists",
                "already signed in on this browser",
            ));
        }
    }
    state
        .limiter
        // Fail-closed: the daily cap per address is what stops a farm of guest islands.
        .hit_strict(
            "guest_ip",
            &ip_bucket(&client_ip(&headers)),
            GUESTS_PER_IP_DAY,
            DAY_SECS,
            now_ms(),
        )
        .await?;
    let user_id = user::create_guest(&state.pool).await?;
    let issued = state
        .sessions
        .login(user_id, &fingerprint(&headers, state.config.csrf_secret.expose()))
        .await?;
    tracing::info!(%user_id, "guest_created");
    let jar = set_session_cookies(jar, &issued);
    Ok((
        jar,
        Json(SessionResponse {
            user: UserSummary {
                id: user_id,
                wallet: None,
            },
        }),
    ))
}

/// `POST /auth/link` (0028): the guest of this session proves a wallet with a signed sign-in message (same nonce
/// flow as login) and keeps the island. Needs the session and the CSRF token; never merges two accounts.
/// A new wallet is a new login (SECURITY §3): every earlier session of the guest is revoked and this one is
/// issued afresh, so a copy of the guest cookies (shared PC, planted cookie) does not open the wallet's farm.
pub async fn link(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: CookieJar,
    user: AuthUser,
    Json(body): Json<VerifyRequest>,
) -> Result<(CookieJar, Json<SessionResponse>), ApiError> {
    limit_ip(&state, &headers).await?;
    let wallet = proven_wallet(&state, &body).await?;
    user::link_wallet(&state.pool, user.user_id, &wallet).await?;
    state.sessions.revoke_all(user.user_id).await?;
    let issued = state
        .sessions
        .login(
            user.user_id,
            &fingerprint(&headers, state.config.csrf_secret.expose()),
        )
        .await?;
    tracing::info!(user_id = %user.user_id, kind = ?wallet.kind, "guest_linked");
    let jar = set_session_cookies(jar, &issued);
    Ok((
        jar,
        Json(SessionResponse {
            user: UserSummary {
                id: user.user_id,
                wallet: Some(wallet.to_string()),
            },
        }),
    ))
}

/// `POST /auth/refresh` — rotates the refresh token; reuse revokes the family.
pub async fn refresh(
    State(state): State<AppState>,
    headers: HeaderMap,
    jar: CookieJar,
) -> Result<(CookieJar, Json<SessionResponse>), ApiError> {
    limit_ip(&state, &headers).await?;
    let token = jar
        .get(REFRESH_COOKIE)
        .map(|c| c.value().to_owned())
        .ok_or_else(|| ApiError::unauthorized("session_missing"))?;
    let fp: ClientFingerprint = fingerprint(&headers, state.config.csrf_secret.expose());
    let issued = match state.sessions.refresh(&token, &fp).await {
        Ok(issued) => issued,
        Err(err) => return Err(err.into()),
    };
    let wallet = wallet_of(&state, issued.user_id).await?;
    let jar = set_session_cookies(jar, &issued);
    Ok((
        jar,
        Json(SessionResponse {
            user: UserSummary {
                id: issued.user_id,
                wallet,
            },
        }),
    ))
}

/// `POST /auth/logout`
pub async fn logout(
    State(state): State<AppState>,
    headers: HeaderMap,
    user: AuthUser,
    jar: CookieJar,
) -> Result<CookieJar, ApiError> {
    limit_ip(&state, &headers).await?;
    let refresh = jar.get(REFRESH_COOKIE).map(|c| c.value().to_owned());
    state.sessions.logout(&user, refresh.as_deref()).await?;
    Ok(clear_session_cookies(jar))
}

async fn wallet_of(state: &AppState, user_id: uuid::Uuid) -> Result<Option<String>, ApiError> {
    use sqlx::Row;
    let row = sqlx::query("SELECT wallet FROM cf.users WHERE id = $1")
        .bind(user_id)
        .fetch_one(&state.pool)
        .await
        .map_err(AuthError::from)?;
    row.try_get("wallet").map_err(|e| AuthError::from(e).into())
}

fn set_session_cookies(jar: CookieJar, issued: &IssuedSession) -> CookieJar {
    let access = Cookie::build((ACCESS_COOKIE, issued.access_token.clone()))
        .http_only(true)
        .secure(true)
        .same_site(SameSite::Strict)
        .path("/api")
        .max_age(ACCESS_TTL)
        .build();
    let refresh = Cookie::build((REFRESH_COOKIE, issued.refresh_token.clone()))
        .http_only(true)
        .secure(true)
        .same_site(SameSite::Strict)
        .path("/api/v1/auth")
        .max_age(REFRESH_TTL)
        .build();
    // Readable by JS on purpose: double-submit token (docs/SECURITY.md §3).
    let csrf = Cookie::build((CSRF_COOKIE, issued.csrf_token.clone()))
        .http_only(false)
        .secure(true)
        .same_site(SameSite::Strict)
        .path("/")
        .max_age(REFRESH_TTL)
        .build();
    jar.add(access).add(refresh).add(csrf)
}

fn clear_session_cookies(jar: CookieJar) -> CookieJar {
    let expired = |name: &'static str, path: &'static str| {
        Cookie::build((name, ""))
            .http_only(name != CSRF_COOKIE)
            .secure(true)
            .same_site(SameSite::Strict)
            .path(path)
            .max_age(time::Duration::ZERO)
            .build()
    };
    jar.add(expired(ACCESS_COOKIE, "/api"))
        .add(expired(REFRESH_COOKIE, "/api/v1/auth"))
        .add(expired(CSRF_COOKIE, "/"))
}

fn now_ms() -> i64 {
    let now = OffsetDateTime::now_utc();
    i64::try_from(now.unix_timestamp_nanos() / 1_000_000).unwrap_or(i64::MAX)
}
