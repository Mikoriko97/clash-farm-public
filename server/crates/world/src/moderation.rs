//! Nickname moderation (0031, ADR-040). A moderator's reset gives the player a placeholder name
//! (`Farmer_<8 hex>`, the look stays), one free rename (while the reset is the player's latest
//! `nickname_history` row — derived, no ticket, no ledger entry) and blocks the old name for
//! everybody, lookalikes included (the block is by `nickname_key`), until an admin lifts it.
//! Admins can also block a name ahead of time. Every function here runs in the caller's
//! transaction: the admin app writes its audit row in the same one.
//!
//! Lock order: the user row first, `FOR NO KEY UPDATE` like a rename (whose ticket row comes
//! before it — a reset takes no ticket; restyle.rs says why not FOR UPDATE), then the name keys through `rename::lock_keys`; create/rename check a block
//! under the same key lock, so a block and a claim of that name never interleave.

use rand::RngExt;
use serde::Serialize;
use sqlx::PgConnection;
use uuid::Uuid;

use crate::error::WorldError;
use crate::nickname::{self, Checked, PLACEHOLDER_PREFIX};
use crate::plots::record_event;
use crate::rename::{lock_keys, take_name};

/// Is this name key blocked right now?
pub(crate) async fn blocked(conn: &mut PgConnection, key: &str) -> Result<bool, WorldError> {
    Ok(sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM cf.nickname_blocks WHERE key = $1 AND lifted_at IS NULL)",
    )
    .bind(key)
    .fetch_one(conn)
    .await?)
}

/// Does the player's latest name change come from a moderator? Then the next rename is free.
/// By `id`: every history row of one player is written under that player's row lock, so the
/// id order is the commit order (`changed_at` is the transaction start, which may not be).
pub async fn free_rename(conn: &mut PgConnection, user_id: Uuid) -> Result<bool, WorldError> {
    Ok(sqlx::query_scalar(
        "SELECT coalesce((SELECT kind = 'moderation' FROM cf.nickname_history
                          WHERE user_id = $1 ORDER BY id DESC LIMIT 1), false)",
    )
    .bind(user_id)
    .fetch_one(conn)
    .await?)
}

#[derive(Debug, Clone, Serialize)]
pub struct Reset {
    /// The name that was reset (and is blocked now).
    pub old: String,
    /// The placeholder the player carries until the free rename.
    pub new: String,
    /// A repeat of a reset already done: nothing changed, nothing to audit.
    pub repeated: bool,
}

/// Resets the player's name if it still is `expected` (the name the admin saw on the card).
/// `WrongState`: the name changed since (the player already took a new one) — nothing is done, the
/// admin reloads the card. A repeat while the player still carries the placeholder (a double click,
/// a lost answer) is the same answer again, `repeated`. `NicknameTaken`: the random placeholder
/// collided (one in four billion) — the transaction is aborted, the caller retries in a new one.
pub async fn reset_nickname(
    conn: &mut PgConnection,
    user_id: Uuid,
    expected: &str,
    reason: &str,
    actor: &str,
) -> Result<Reset, WorldError> {
    let row: Option<(Option<String>, Option<String>)> =
        sqlx::query_as("SELECT nickname, nickname_key FROM cf.users WHERE id = $1 FOR NO KEY UPDATE")
            .bind(user_id)
            .fetch_optional(&mut *conn)
            .await?;
    let (Some(old), Some(old_key)) = row.ok_or(WorldError::NotFound)? else {
        return Err(WorldError::NoCharacter);
    };
    if let Some(done) = still_reset(conn, user_id, &old).await? {
        return Ok(done);
    }
    if old != expected {
        return Err(WorldError::WrongState);
    }
    let new = format!("{PLACEHOLDER_PREFIX}{:08x}", rand::rng().random::<u32>());
    let key = nickname::key_of(&new);
    lock_keys(conn, &[&old_key, &key]).await?;
    if blocked(conn, &key).await? {
        return Err(WorldError::NicknameTaken);
    }
    take_name(conn, user_id, &new, &key).await?;
    record_reset(conn, user_id, (&old, &old_key), (&new, &key), (reason, actor)).await?;
    Ok(Reset {
        old,
        new,
        repeated: false,
    })
}

/// The player still carries the placeholder of the latest reset: that reset, as a repeat.
async fn still_reset(
    conn: &mut PgConnection,
    user_id: Uuid,
    current: &str,
) -> Result<Option<Reset>, WorldError> {
    let last: Option<(String, String, String)> = sqlx::query_as(
        "SELECT kind, old_nickname, new_nickname FROM cf.nickname_history WHERE user_id = $1 ORDER BY id DESC LIMIT 1",
    )
    .bind(user_id)
    .fetch_optional(conn)
    .await?;
    Ok(last
        .filter(|(kind, _, new)| kind == "moderation" && new == current)
        .map(|(_, old, new)| Reset {
            old,
            new,
            repeated: true,
        }))
}

/// The history row (kind `moderation`), the block of the old name and the player's event.
async fn record_reset(
    conn: &mut PgConnection,
    user_id: Uuid,
    (old, old_key): (&str, &str),
    (new, key): (&str, &str),
    (reason, actor): (&str, &str),
) -> Result<(), WorldError> {
    sqlx::query(
        "INSERT INTO cf.nickname_history (user_id, old_nickname, old_key, new_nickname, new_key, kind)
         VALUES ($1, $2, $3, $4, $5, 'moderation')",
    )
    .bind(user_id)
    .bind(old)
    .bind(old_key)
    .bind(new)
    .bind(key)
    .execute(&mut *conn)
    .await?;
    // Already blocked by hand: that block (and its reason) stays.
    sqlx::query(
        "INSERT INTO cf.nickname_blocks (key, nickname, reason, blocked_by) VALUES ($1, $2, $3, $4)
         ON CONFLICT (key) WHERE lifted_at IS NULL DO NOTHING",
    )
    .bind(old_key)
    .bind(old)
    .bind(reason)
    .bind(actor)
    .execute(&mut *conn)
    .await?;
    record_event(
        conn,
        user_id,
        "nickname_reset",
        serde_json::json!({ "from": old, "to": new }),
    )
    .await
}

#[derive(Debug, Clone, Serialize)]
pub struct Block {
    pub id: i64,
    pub nickname: String,
    pub key: String,
    /// The account that carries this name (or a lookalike of it) right now: the block does not
    /// rename it — the admin resets it separately.
    pub holder: Option<Uuid>,
}

/// Blocks a name ahead of time. `Ok(None)`: a live block of this key exists already.
/// A name the rules refuse anyway is `NicknameNotAllowed` (nothing to block).
pub async fn block_name(
    conn: &mut PgConnection,
    raw: &str,
    reason: &str,
    actor: &str,
) -> Result<Option<Block>, WorldError> {
    let Checked { nickname, key } = nickname::validate(raw.trim())?;
    lock_keys(conn, &[&key]).await?;
    let holder: Option<Uuid> = sqlx::query_scalar("SELECT id FROM cf.users WHERE nickname_key = $1")
        .bind(&key)
        .fetch_optional(&mut *conn)
        .await?;
    let id: Option<i64> = sqlx::query_scalar(
        "INSERT INTO cf.nickname_blocks (key, nickname, reason, blocked_by) VALUES ($1, $2, $3, $4)
         ON CONFLICT (key) WHERE lifted_at IS NULL DO NOTHING RETURNING id",
    )
    .bind(&key)
    .bind(&nickname)
    .bind(reason)
    .bind(actor)
    .fetch_optional(&mut *conn)
    .await?;
    Ok(id.map(|id| Block {
        id,
        nickname,
        key,
        holder,
    }))
}

/// Lifts a live block: the name is free to claim again (a 30-day hold still applies). Returns
/// the blocked name; `NotFound` for an unknown or already lifted block.
pub async fn lift_block(conn: &mut PgConnection, id: i64, actor: &str) -> Result<String, WorldError> {
    sqlx::query_scalar(
        "UPDATE cf.nickname_blocks SET lifted_at = now(), lifted_by = $2
         WHERE id = $1 AND lifted_at IS NULL RETURNING nickname",
    )
    .bind(id)
    .bind(actor)
    .fetch_optional(conn)
    .await?
    .ok_or(WorldError::NotFound)
}
