//! Family-wide session operations (ADR-036): the grace window for refresh reuse, revocation
//! (theft detection, ban, logout, revoke-all) and the liveness check of long-lived sockets.
//! Every change to a family's refresh tokens holds that family's transaction-scoped advisory lock,
//! taken before any row lock, so a revocation never misses a token a concurrent rotation inserts,
//! and a grace rotation always sees the latest live token.

use time::OffsetDateTime;
use uuid::Uuid;

use super::{refresh_row, ClientFingerprint, RefreshRow, SessionRef, SessionService, REFRESH_REUSE_GRACE};
use crate::error::AuthError;

/// The first key of every session advisory lock: other subsystems pick their own namespace.
const LOCK_NAMESPACE: i32 = 36;

/// Serialises every change to `family`'s tokens until the transaction ends.
pub(super) async fn lock_family(tx: &mut sqlx::PgConnection, family: Uuid) -> Result<(), AuthError> {
    sqlx::query("SELECT pg_advisory_xact_lock($1, hashtext(($2::uuid)::text))")
        .bind(LOCK_NAMESPACE)
        .bind(family)
        .execute(tx)
        .await?;
    Ok(())
}

impl SessionService {
    /// The family's live token when `row` was rotated within `REFRESH_REUSE_GRACE` and the same
    /// browser presents it; `None` (a reuse) otherwise — also after logout, revocation or a ban,
    /// which leave the family no live token. The caller holds the family lock.
    pub(super) async fn grace_successor(
        &self,
        tx: &mut sqlx::PgConnection,
        row: &RefreshRow,
        fp: &ClientFingerprint,
    ) -> Result<Option<RefreshRow>, AuthError> {
        let recent = row
            .revoked_at
            .is_some_and(|at| at + REFRESH_REUSE_GRACE > OffsetDateTime::now_utc());
        if !recent || !fp.same_browser(row.ua_hash.as_deref()) {
            return Ok(None);
        }
        let live = sqlx::query(
            "SELECT id, user_id, family, expires_at, revoked_at, ua_hash FROM cf.sessions
             WHERE family = $1 AND revoked_at IS NULL FOR UPDATE",
        )
        .bind(row.family)
        .fetch_optional(tx)
        .await?;
        live.as_ref().map(refresh_row).transpose()
    }

    /// Revokes every refresh token of the family (how many were live); the caller holds the family lock.
    pub(super) async fn revoke_family(
        &self,
        tx: &mut sqlx::PgConnection,
        family: Uuid,
    ) -> Result<u64, AuthError> {
        let done =
            sqlx::query("UPDATE cf.sessions SET revoked_at = now() WHERE family = $1 AND revoked_at IS NULL")
                .bind(family)
                .execute(tx)
                .await?;
        Ok(done.rows_affected())
    }

    /// Is the family still signed in: a live, unexpired refresh token of a user who is not banned?
    /// For connections that outlive their 15-minute access token (the presence socket, ADR-036);
    /// logout, revocation, theft detection, a ban or expiry all end it.
    pub async fn family_alive(&self, family: Uuid) -> Result<bool, AuthError> {
        let alive: bool = sqlx::query_scalar(
            "SELECT EXISTS (SELECT 1 FROM cf.sessions s JOIN cf.users u ON u.id = s.user_id
             WHERE s.family = $1 AND s.revoked_at IS NULL AND s.expires_at > now() AND u.banned_at IS NULL)",
        )
        .bind(family)
        .fetch_one(&self.pool)
        .await?;
        Ok(alive)
    }

    /// Ends the session: the whole family, not just the refresh cookie that came along — a stale
    /// cookie (a lost refresh response) or none at all must not leave a live successor for the grace
    /// window or the presence socket. All its access tokens go too.
    pub async fn logout(
        &self,
        session: &impl SessionRef,
        refresh_token: Option<&str>,
    ) -> Result<(), AuthError> {
        let family = session.family();
        let mut tx = self.pool.begin().await?;
        lock_family(&mut tx, family).await?;
        self.revoke_family(&mut tx, family).await?;
        if let Some(token) = refresh_token {
            sqlx::query(
                "UPDATE cf.sessions SET revoked_at = now() WHERE refresh_hash = $1 AND revoked_at IS NULL",
            )
            .bind(self.hash(token))
            .execute(&mut *tx)
            .await?;
        }
        self.purge_family_access(family).await?;
        tx.commit().await?;
        let mut con = self.redis.clone();
        let _: () = redis::cmd("DEL")
            .arg(self.access_key(session.access_token()))
            .query_async(&mut con)
            .await?;
        Ok(())
    }

    /// Revokes every session of the user, refresh and access alike (ban, wallet link, incidents).
    pub async fn revoke_all(&self, user_id: Uuid) -> Result<u64, AuthError> {
        let mut tx = self.pool.begin().await?;
        let families: Vec<Uuid> = sqlx::query_scalar(
            "SELECT DISTINCT family FROM cf.sessions WHERE user_id = $1 AND revoked_at IS NULL ORDER BY family",
        )
        .bind(user_id)
        .fetch_all(&mut *tx)
        .await?;
        // In a fixed order: two revocations of one user never wait on each other in a cycle.
        for family in &families {
            lock_family(&mut tx, *family).await?;
        }
        let done = sqlx::query(
            "UPDATE cf.sessions SET revoked_at = now() WHERE user_id = $1 AND revoked_at IS NULL",
        )
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
        for family in &families {
            self.purge_family_access(*family).await?;
        }
        tx.commit().await?;
        Ok(done.rows_affected())
    }
}
