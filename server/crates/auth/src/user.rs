//! Users are created on first successful login (no separate registration), or as a guest without a wallet
//! (0028) who can link one later.

use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::error::AuthError;
use crate::wallet::WalletTag;

/// Finds the user for `wallet`, creating it when registrations are enabled.
/// Updates `last_seen_at` on every login.
pub async fn upsert_by_wallet(pool: &PgPool, wallet: &WalletTag) -> Result<Uuid, AuthError> {
    let tag = wallet.to_string();
    // A banned wallet (0020) is refused before `last_seen_at` moves: the row stays as the admin left it.
    let existing = sqlx::query(
        "UPDATE cf.users SET last_seen_at = CASE WHEN banned_at IS NULL THEN now() ELSE last_seen_at END
         WHERE wallet = $1 RETURNING id, banned_at IS NOT NULL AS banned",
    )
    .bind(&tag)
    .fetch_optional(pool)
    .await?;
    if let Some(row) = existing {
        if row.try_get::<bool, _>("banned")? {
            return Err(AuthError::Banned);
        }
        return Ok(row.try_get("id")?);
    }
    if !registrations_enabled(pool).await? {
        return Err(AuthError::RegistrationsDisabled);
    }
    let row = sqlx::query(
        "INSERT INTO cf.users (wallet) VALUES ($1)
         ON CONFLICT (wallet) DO UPDATE SET last_seen_at = now()
         RETURNING id",
    )
    .bind(&tag)
    .fetch_one(pool)
    .await?;
    Ok(row.try_get("id")?)
}

/// A new guest: a user row without a wallet (0028). Needs both `registrations_enabled` and `guests_enabled`.
pub async fn create_guest(pool: &PgPool) -> Result<Uuid, AuthError> {
    if !registrations_enabled(pool).await? {
        return Err(AuthError::RegistrationsDisabled);
    }
    if !setting_on(pool, "guests_enabled").await? {
        return Err(AuthError::GuestsDisabled);
    }
    let row = sqlx::query("INSERT INTO cf.users (wallet) VALUES (NULL) RETURNING id")
        .fetch_one(pool)
        .await?;
    Ok(row.try_get("id")?)
}

/// Gives the guest `user_id` the verified `wallet` (0028). Never merges accounts: a wallet that already has its
/// own account is refused, and so is an account that already has a wallet. A banned guest cannot link.
pub async fn link_wallet(pool: &PgPool, user_id: Uuid, wallet: &WalletTag) -> Result<(), AuthError> {
    let tag = wallet.to_string();
    let taken: Option<Uuid> = sqlx::query_scalar("SELECT id FROM cf.users WHERE wallet = $1")
        .bind(&tag)
        .fetch_optional(pool)
        .await?;
    if taken.is_some() {
        return Err(AuthError::WalletTaken);
    }
    let row = sqlx::query(
        "UPDATE cf.users SET wallet = $2, last_seen_at = now()
         WHERE id = $1 AND wallet IS NULL AND banned_at IS NULL RETURNING id",
    )
    .bind(user_id)
    .bind(&tag)
    .fetch_optional(pool)
    .await;
    match row {
        Ok(Some(_)) => Ok(()),
        Ok(None) => {
            let banned: Option<bool> =
                sqlx::query_scalar("SELECT banned_at IS NOT NULL FROM cf.users WHERE id = $1")
                    .bind(user_id)
                    .fetch_optional(pool)
                    .await?;
            Err(if banned == Some(true) {
                AuthError::Banned
            } else {
                AuthError::AlreadyLinked
            })
        }
        // The same wallet linked by a parallel request in between: the UNIQUE constraint decides.
        Err(sqlx::Error::Database(e)) if e.is_unique_violation() => Err(AuthError::WalletTaken),
        Err(e) => Err(e.into()),
    }
}

async fn registrations_enabled(pool: &PgPool) -> Result<bool, AuthError> {
    setting_on(pool, "registrations_enabled").await
}

/// A boolean switch in `cf.settings`; a missing key reads as off.
async fn setting_on(pool: &PgPool, key: &str) -> Result<bool, AuthError> {
    let row = sqlx::query("SELECT value FROM cf.settings WHERE key = $1")
        .bind(key)
        .fetch_optional(pool)
        .await?;
    Ok(match row {
        Some(row) => row
            .try_get::<serde_json::Value, _>("value")?
            .as_bool()
            .unwrap_or(false),
        None => false,
    })
}
