//! Admin sessions live in Redis only: a random token in an HttpOnly, SameSite=Strict
//! cookie, stored under an HMAC of the token, 30 minutes sliding. The stored
//! record carries the CSRF token that every mutating form must echo back.

use std::sync::Arc;

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum_extra::extract::cookie::CookieJar;
use cf_auth::session::random_token;
use cf_common::hashing::keyed_sha256;
use redis::aio::ConnectionManager;
use serde::{Deserialize, Serialize};
use subtle::ConstantTimeEq;

use crate::error::AdminError;
use crate::state::AdminState;

pub const COOKIE: &str = "cf_admin";
/// A signature that still waits for its TOTP code (ADR-023); scoped to `/login`.
pub const PENDING_COOKIE: &str = "cf_admin_2fa";
pub const PENDING_TTL_SECS: u64 = 5 * 60;
/// A time step is accepted once per wallet; the mark outlives the ±1-step window.
const TOTP_STEP_TTL_SECS: u64 = 120;
/// Sliding idle timeout.
pub const TTL_SECS: u64 = 30 * 60;
/// Absolute cap: an always-open tab still has to sign in again.
pub const ABSOLUTE_MAX_SECS: i64 = 8 * 60 * 60;

#[derive(Clone)]
pub struct AdminSessions {
    redis: ConnectionManager,
    secret: Arc<String>,
}

impl std::fmt::Debug for AdminSessions {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("AdminSessions")
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct Stored {
    wallet: String,
    csrf: String,
    issued_at: i64,
}

#[derive(Debug, Clone)]
pub struct Issued {
    pub token: String,
    pub csrf: String,
}

impl AdminSessions {
    pub fn new(redis: ConnectionManager, cookie_secret: &str) -> Self {
        Self {
            redis,
            secret: Arc::new(cookie_secret.to_owned()),
        }
    }

    fn key(&self, token: &str) -> String {
        format!(
            "admin:sess:{}",
            hex::encode(keyed_sha256(self.secret.as_bytes(), token.as_bytes()))
        )
    }

    pub async fn issue(&self, wallet: &str) -> Result<Issued, redis::RedisError> {
        let token = random_token();
        let csrf = random_token();
        let stored = serde_json::to_string(&Stored {
            wallet: wallet.to_owned(),
            csrf: csrf.clone(),
            issued_at: time::OffsetDateTime::now_utc().unix_timestamp(),
        })
        .unwrap_or_default();
        let mut con = self.redis.clone();
        let _: () = redis::cmd("SET")
            .arg(self.key(&token))
            .arg(stored)
            .arg("EX")
            .arg(TTL_SECS)
            .query_async(&mut con)
            .await?;
        Ok(Issued { token, csrf })
    }

    /// Wallet + CSRF token for a live session; touching it extends the TTL.
    pub async fn get(&self, token: &str) -> Result<Option<(String, String)>, redis::RedisError> {
        let key = self.key(token);
        let mut con = self.redis.clone();
        let raw: Option<String> = redis::cmd("GET").arg(&key).query_async(&mut con).await?;
        let Some(raw) = raw else { return Ok(None) };
        let Some(stored) = serde_json::from_str::<Stored>(&raw).ok() else {
            return Ok(None);
        };
        let age = time::OffsetDateTime::now_utc().unix_timestamp() - stored.issued_at;
        if age > ABSOLUTE_MAX_SECS {
            let _: () = redis::cmd("DEL").arg(&key).query_async(&mut con).await?;
            return Ok(None);
        }
        let _: () = redis::cmd("EXPIRE")
            .arg(&key)
            .arg(TTL_SECS)
            .query_async(&mut con)
            .await?;
        Ok(Some((stored.wallet, stored.csrf)))
    }

    pub async fn revoke(&self, token: &str) -> Result<(), redis::RedisError> {
        let mut con = self.redis.clone();
        let _: () = redis::cmd("DEL")
            .arg(self.key(token))
            .query_async(&mut con)
            .await?;
        Ok(())
    }

    fn pending_key(&self, token: &str) -> String {
        format!(
            "admin:pending:{}",
            hex::encode(keyed_sha256(self.secret.as_bytes(), token.as_bytes()))
        )
    }

    /// A verified signature waiting for its code: wallet only, five minutes.
    pub async fn issue_pending(&self, wallet: &str) -> Result<String, redis::RedisError> {
        let token = random_token();
        let mut con = self.redis.clone();
        let _: () = redis::cmd("SET")
            .arg(self.pending_key(&token))
            .arg(wallet)
            .arg("EX")
            .arg(PENDING_TTL_SECS)
            .query_async(&mut con)
            .await?;
        Ok(token)
    }

    pub async fn pending_wallet(&self, token: &str) -> Result<Option<String>, redis::RedisError> {
        let mut con = self.redis.clone();
        redis::cmd("GET")
            .arg(self.pending_key(token))
            .query_async(&mut con)
            .await
    }

    pub async fn consume_pending(&self, token: &str) -> Result<(), redis::RedisError> {
        let mut con = self.redis.clone();
        let _: () = redis::cmd("DEL")
            .arg(self.pending_key(token))
            .query_async(&mut con)
            .await?;
        Ok(())
    }

    /// `true` the first time a (wallet, step) pair is seen (RFC 6238 §5.2: no replay).
    pub async fn claim_totp_step(&self, wallet: &str, step: u64) -> Result<bool, redis::RedisError> {
        let mut con = self.redis.clone();
        let set: Option<String> = redis::cmd("SET")
            .arg(format!("admin:totp_step:{wallet}:{step}"))
            .arg("1")
            .arg("NX")
            .arg("EX")
            .arg(TOTP_STEP_TTL_SECS)
            .query_async(&mut con)
            .await?;
        Ok(set.is_some())
    }
}

/// The signed-in admin, resolved from the cookie on every request.
#[derive(Clone)]
pub struct Admin {
    pub wallet: String,
    pub token: String,
    csrf: String,
}

impl std::fmt::Debug for Admin {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Admin")
            .field("wallet", &self.wallet)
            .finish_non_exhaustive()
    }
}

impl Admin {
    pub fn csrf(&self) -> &str {
        &self.csrf
    }

    /// Every mutating form carries the session's CSRF token; compared in constant time.
    pub fn check_csrf(&self, submitted: &str) -> Result<(), AdminError> {
        if submitted.as_bytes().ct_eq(self.csrf.as_bytes()).into() {
            Ok(())
        } else {
            Err(AdminError::Forbidden("csrf token mismatch"))
        }
    }
}

impl FromRequestParts<AdminState> for Admin {
    type Rejection = AdminError;

    async fn from_request_parts(parts: &mut Parts, state: &AdminState) -> Result<Self, Self::Rejection> {
        let jar = CookieJar::from_headers(&parts.headers);
        let token = jar
            .get(COOKIE)
            .map(|c| c.value().to_owned())
            .ok_or(AdminError::Unauthorized)?;
        let (wallet, csrf) = state
            .sessions
            .get(&token)
            .await?
            .ok_or(AdminError::Unauthorized)?;
        Ok(Admin { wallet, token, csrf })
    }
}
