//! Cookie sessions (ADR-007). Access tokens live in Redis (15 min), refresh
//! tokens in Postgres (7 days) with rotation; reusing a rotated refresh token
//! revokes the whole family (unless the same browser repeats it within 30 s —
//! a lost response or a second tab, ADR-036). Tokens are opaque random values; only keyed hashes
//! are stored, so rotating `CF_COOKIE_SECRET` invalidates everything.
//! Every access token is also indexed by family (`sess:family:<uuid>`) so
//! rotation, logout and revocation purge them immediately.

use std::fmt;

use cf_common::hashing::keyed_sha256;
use rand::Rng;
use redis::aio::ConnectionManager;
use serde::{Deserialize, Serialize};
use sqlx::{PgPool, Row};
use time::{Duration, OffsetDateTime};
use uuid::Uuid;

use crate::error::AuthError;

mod family;
use family::lock_family;

pub const ACCESS_TTL: Duration = Duration::minutes(15);
pub const REFRESH_TTL: Duration = Duration::days(7);
/// A rotated refresh token presented again this soon by the same browser (user agent) is a lost
/// response or a second tab racing the first (ADR-036), not a theft: it rotates from the family's
/// live token instead of revoking the family. Later, or from another browser, reuse still revokes.
pub const REFRESH_REUSE_GRACE: Duration = Duration::seconds(30);

#[derive(Clone)]
pub struct IssuedSession {
    pub user_id: Uuid,
    pub family: Uuid,
    pub access_token: String,
    pub refresh_token: String,
    pub csrf_token: String,
}

impl fmt::Debug for IssuedSession {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("IssuedSession")
            .field("user_id", &self.user_id)
            .field("family", &self.family)
            .finish_non_exhaustive()
    }
}

/// What a valid access token resolves to.
#[derive(Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Principal {
    pub user_id: Uuid,
    pub family: Uuid,
    pub csrf_token: String,
}

impl fmt::Debug for Principal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("Principal")
            .field("user_id", &self.user_id)
            .field("family", &self.family)
            .finish_non_exhaustive()
    }
}

/// The session as seen by a handler (from the `AuthUser` extractor).
pub trait SessionRef {
    fn family(&self) -> Uuid;
    fn access_token(&self) -> &str;
}

#[derive(Clone)]
pub struct SessionService {
    pool: PgPool,
    redis: ConnectionManager,
    cookie_key: Vec<u8>,
}

impl fmt::Debug for SessionService {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("SessionService").finish_non_exhaustive()
    }
}

#[derive(Debug, Clone, Default)]
pub struct ClientFingerprint {
    pub ip_hash: Option<Vec<u8>>,
    pub ua_hash: Option<Vec<u8>>,
    /// The same user agent under yesterday's salt: a grace replay right after 00:00 UTC
    /// still matches the token its other tab rotated a few seconds before midnight.
    pub ua_hash_prev: Option<Vec<u8>>,
}

impl ClientFingerprint {
    /// Does `stored` (a token's `ua_hash`) name this browser, today or yesterday?
    pub fn same_browser(&self, stored: Option<&[u8]>) -> bool {
        let Some(stored) = stored else {
            return false;
        };
        [&self.ua_hash, &self.ua_hash_prev]
            .into_iter()
            .any(|h| h.as_deref() == Some(stored))
    }
}

struct RefreshRow {
    id: Uuid,
    user_id: Uuid,
    family: Uuid,
    expires_at: OffsetDateTime,
    revoked_at: Option<OffsetDateTime>,
    ua_hash: Option<Vec<u8>>,
}

fn refresh_row(row: &sqlx::postgres::PgRow) -> Result<RefreshRow, AuthError> {
    Ok(RefreshRow {
        id: row.try_get("id")?,
        user_id: row.try_get("user_id")?,
        family: row.try_get("family")?,
        expires_at: row.try_get("expires_at")?,
        revoked_at: row.try_get("revoked_at")?,
        ua_hash: row.try_get("ua_hash")?,
    })
}

impl SessionService {
    pub fn new(pool: PgPool, redis: ConnectionManager, cookie_secret: &str) -> Self {
        Self {
            pool,
            redis,
            cookie_key: cookie_secret.as_bytes().to_vec(),
        }
    }

    /// New session family after a verified sign-in.
    pub async fn login(&self, user_id: Uuid, fp: &ClientFingerprint) -> Result<IssuedSession, AuthError> {
        let family = Uuid::new_v4();
        let refresh_token = random_token();
        self.insert_refresh(
            &mut *self.pool.acquire().await?,
            user_id,
            family,
            &refresh_token,
            fp,
        )
        .await?;
        self.issue_access(user_id, family, refresh_token).await
    }

    /// Resolves an access token; `None` when unknown/expired.
    pub async fn authenticate(&self, access_token: &str) -> Result<Option<Principal>, AuthError> {
        let mut con = self.redis.clone();
        let raw: Option<String> = redis::cmd("GET")
            .arg(self.access_key(access_token))
            .query_async(&mut con)
            .await?;
        Ok(raw.and_then(|json| serde_json::from_str(&json).ok()))
    }

    /// Rotates the refresh token. Reuse of an already-rotated token revokes the family.
    pub async fn refresh(
        &self,
        refresh_token: &str,
        fp: &ClientFingerprint,
    ) -> Result<IssuedSession, AuthError> {
        let mut tx = self.pool.begin().await?;
        // The family lock first, then the row: a revocation of this family waits for the rotation to
        // finish (or the rotation sees it done) — it never misses the token being inserted (ADR-036).
        let family: Uuid = sqlx::query_scalar("SELECT family FROM cf.sessions WHERE refresh_hash = $1")
            .bind(self.hash(refresh_token))
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(AuthError::SessionInvalid)?;
        lock_family(&mut tx, family).await?;
        let mut row = self.lock_refresh_row(&mut tx, refresh_token).await?;
        if row.revoked_at.is_some() {
            let Some(live) = self.grace_successor(&mut tx, &row, fp).await? else {
                let revoked = self.revoke_family(&mut tx, row.family).await?;
                self.purge_family_access(row.family).await?;
                tx.commit().await?;
                // A family that still had live tokens: theft detection. One already ended (logout, revocation): noise.
                if revoked > 0 {
                    tracing::warn!(family = %row.family, "session_family_revoked: refresh token reuse");
                } else {
                    tracing::info!(family = %row.family, "session_refresh_after_end");
                }
                return Err(AuthError::RefreshReuse);
            };
            tracing::info!(family = %row.family, "session_refresh_grace");
            row = live;
        }
        if row.expires_at <= OffsetDateTime::now_utc() {
            return Err(AuthError::SessionInvalid);
        }
        // Banned while holding a refresh token (0020): the family ends here.
        let banned: bool =
            sqlx::query_scalar("SELECT banned_at IS NOT NULL FROM cf.users WHERE id = $1 FOR SHARE")
                .bind(row.user_id)
                .fetch_one(&mut *tx)
                .await?;
        if banned {
            self.revoke_family(&mut tx, row.family).await?;
            self.purge_family_access(row.family).await?;
            tx.commit().await?;
            return Err(AuthError::Banned);
        }
        sqlx::query("UPDATE cf.sessions SET revoked_at = now() WHERE id = $1")
            .bind(row.id)
            .execute(&mut *tx)
            .await?;
        let new_token = random_token();
        self.insert_refresh(&mut tx, row.user_id, row.family, &new_token, fp)
            .await?;
        // One live access token per family: the pre-rotation one dies now. Redis changes while the
        // family lock is held, so a revocation waiting for it purges the new access token as well.
        self.purge_family_access(row.family).await?;
        let issued = self.issue_access(row.user_id, row.family, new_token).await?;
        tx.commit().await?;
        Ok(issued)
    }

    async fn lock_refresh_row(
        &self,
        tx: &mut sqlx::PgConnection,
        token: &str,
    ) -> Result<RefreshRow, AuthError> {
        let row = sqlx::query(
            "SELECT id, user_id, family, expires_at, revoked_at, ua_hash FROM cf.sessions
             WHERE refresh_hash = $1 FOR UPDATE",
        )
        .bind(self.hash(token))
        .fetch_optional(tx)
        .await?
        .ok_or(AuthError::SessionInvalid)?;
        refresh_row(&row)
    }

    async fn insert_refresh(
        &self,
        conn: &mut sqlx::PgConnection,
        user_id: Uuid,
        family: Uuid,
        token: &str,
        fp: &ClientFingerprint,
    ) -> Result<(), AuthError> {
        sqlx::query(
            "INSERT INTO cf.sessions (user_id, family, refresh_hash, expires_at, ip_hash, ua_hash)
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(user_id)
        .bind(family)
        .bind(self.hash(token))
        .bind(OffsetDateTime::now_utc() + REFRESH_TTL)
        .bind(&fp.ip_hash)
        .bind(&fp.ua_hash)
        .execute(conn)
        .await?;
        Ok(())
    }

    async fn issue_access(
        &self,
        user_id: Uuid,
        family: Uuid,
        refresh_token: String,
    ) -> Result<IssuedSession, AuthError> {
        let access_token = random_token();
        let csrf_token = random_token();
        let principal = Principal {
            user_id,
            family,
            csrf_token: csrf_token.clone(),
        };
        let json = serde_json::to_string(&principal).map_err(|_| AuthError::SessionInvalid)?;
        let key = self.access_key(&access_token);
        let mut con = self.redis.clone();
        let _: () = redis::pipe()
            .atomic()
            .cmd("SET")
            .arg(&key)
            .arg(json)
            .arg("EX")
            .arg(ACCESS_TTL.whole_seconds())
            .ignore()
            .cmd("SADD")
            .arg(family_key(family))
            .arg(&key)
            .ignore()
            .cmd("EXPIRE")
            .arg(family_key(family))
            .arg(REFRESH_TTL.whole_seconds())
            .ignore()
            .query_async(&mut con)
            .await?;
        Ok(IssuedSession {
            user_id,
            family,
            access_token,
            refresh_token,
            csrf_token,
        })
    }

    /// Deletes all access tokens indexed under the family.
    async fn purge_family_access(&self, family: Uuid) -> Result<(), AuthError> {
        let mut con = self.redis.clone();
        let keys: Vec<String> = redis::cmd("SMEMBERS")
            .arg(family_key(family))
            .query_async(&mut con)
            .await?;
        let mut del = redis::cmd("DEL");
        del.arg(family_key(family));
        for key in &keys {
            del.arg(key);
        }
        let _: () = del.query_async(&mut con).await?;
        Ok(())
    }

    fn hash(&self, token: &str) -> Vec<u8> {
        keyed_sha256(&self.cookie_key, token.as_bytes()).to_vec()
    }

    fn access_key(&self, token: &str) -> String {
        format!("sess:access:{}", hex::encode(self.hash(token)))
    }
}

fn family_key(family: Uuid) -> String {
    format!("sess:family:{family}")
}

/// 32 random bytes as 64 hex chars.
pub fn random_token() -> String {
    let mut bytes = [0u8; 32];
    rand::rng().fill_bytes(&mut bytes);
    hex::encode(bytes)
}
