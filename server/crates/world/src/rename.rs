//! Paid nickname change (0030, ADR-039): a new name costs one `rename_ticket`, bought for CLASH
//! through the verified order flow, so the payment path is untouched. The new name passes the
//! same rules as at creation (`nickname::validate`), is unique across lookalikes (`nickname_key`,
//! `UNIQUE`), and a name given up within `HOLD_DAYS` stays with its former owner: nobody else can
//! claim it, at creation or by renaming, so a fresh account cannot pose as a player who just
//! renamed. Ticket, name and the history row (`cf.nickname_history`, append-only) change in one
//! transaction; a refusal spends nothing. Also home of the live name check (`check`), which a
//! player with a character uses before renaming. After a moderator's reset (0031, ADR-040) the
//! next rename is free, and a blocked name is refused to everybody (`moderation`).

use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use crate::caps;
use crate::character::{
    Character, Look, NickCheck, CAP_NICK_CHECK, CAP_NICK_CLAIM, KEY_INDEX, NICK_CHECKS_PER_DAY,
    NICK_CLAIMS_PER_DAY,
};
use crate::error::WorldError;
use crate::inventory;
use crate::moderation::{blocked, free_rename};
use crate::nickname::{self, Checked};
use crate::plots::record_event;

/// Catalog id of the ticket (migration 0030).
pub const TICKET: &str = "rename_ticket";
/// A name given up stays with its former owner this long (owner decision 2026-10-03).
pub const HOLD_DAYS: i32 = 30;
/// Advisory-lock namespace of nickname keys (sessions use 36).
const LOCK_NAMESPACE: i32 = 39;

/// Serialises every claim and release of these name keys until the transaction ends: the hold
/// check and the write it guards cannot interleave with a rename that is giving the name up. The
/// locks are taken in the order of the hash actually locked (deduplicated), so two transactions
/// never wait on each other in a cycle, even for keys whose hashes collide.
pub(crate) async fn lock_keys(conn: &mut PgConnection, keys: &[&str]) -> Result<(), WorldError> {
    let keys: Vec<String> = keys.iter().map(|k| (*k).to_owned()).collect();
    sqlx::query(
        "SELECT pg_advisory_xact_lock($1, h) FROM (SELECT DISTINCT hashtext(k) AS h FROM unnest($2::text[]) AS k) s ORDER BY h",
    )
    .bind(LOCK_NAMESPACE)
    .bind(&keys)
    .fetch_all(&mut *conn)
    .await?;
    Ok(())
}

/// Did another account give this name up within `HOLD_DAYS`?
pub(crate) async fn held_by_other(
    conn: &mut PgConnection,
    user_id: Uuid,
    key: &str,
) -> Result<bool, WorldError> {
    Ok(sqlx::query_scalar(
        "SELECT EXISTS (SELECT 1 FROM cf.nickname_history
         WHERE old_key = $1 AND user_id <> $2 AND changed_at > now() - make_interval(days => $3))",
    )
    .bind(key)
    .bind(user_id)
    .bind(HOLD_DAYS)
    .fetch_one(conn)
    .await?)
}

/// May this account claim the name now (under `lock_keys`)? A blocked name is nobody's
/// (`NicknameNotAllowed`), one another account gave up within `HOLD_DAYS` is still that account's.
pub(crate) async fn claimable(conn: &mut PgConnection, user_id: Uuid, key: &str) -> Result<(), WorldError> {
    if blocked(conn, key).await? {
        return Err(WorldError::NicknameNotAllowed);
    }
    if held_by_other(conn, user_id, key).await? {
        return Err(WorldError::NicknameTaken);
    }
    Ok(())
}

/// `POST /me/nickname { nickname }`: validates, then spends one ticket (none after a moderator's
/// reset) and takes the name.
pub async fn rename(pool: &PgPool, user_id: Uuid, raw_nickname: &str) -> Result<Character, WorldError> {
    // One claim of the day's budget (shared with creation), refunded on success: a refused name
    // answers like the live check and must not be free to repeat.
    let mut tx = pool.begin().await?;
    caps::consume(&mut tx, user_id, CAP_NICK_CLAIM, NICK_CLAIMS_PER_DAY).await?;
    tx.commit().await?;
    let checked = nickname::validate(raw_nickname)?;
    // A reset landing between the first read and the user lock makes the rename free: that attempt
    // rolls back (the ticket stays) and the next one runs without a ticket.
    for _ in 0..2 {
        if let Some(done) = attempt(pool, user_id, &checked).await? {
            if let Err(e) = release_claim(pool, user_id).await {
                tracing::warn!(error = %e, "nickname claim refund failed");
            }
            return Ok(done);
        }
    }
    Err(WorldError::NoRenameTicket)
}

/// One rename transaction. `None`: the rename became free under the user lock; nothing was spent.
async fn attempt(pool: &PgPool, user_id: Uuid, checked: &Checked) -> Result<Option<Character>, WorldError> {
    let Checked { nickname, key } = checked;
    let mut tx = pool.begin().await?;
    // Ticket row, then user row `FOR NO KEY UPDATE`: a fulfilment holds the user row's KEY SHARE
    // (ledger foreign key) while it adds a ticket; FOR UPDATE would deadlock with it (restyle.rs).
    let free = free_rename(&mut tx, user_id).await?;
    if !free {
        inventory::take_one(&mut tx, user_id, TICKET)
            .await
            .map_err(|e| match e {
                WorldError::InsufficientItems => WorldError::NoRenameTicket,
                other => other,
            })?;
    }
    let row: Option<(Option<String>, Option<String>, Option<serde_json::Value>)> =
        sqlx::query_as("SELECT nickname, nickname_key, look FROM cf.users WHERE id = $1 FOR NO KEY UPDATE")
            .bind(user_id)
            .fetch_optional(&mut *tx)
            .await?;
    let (old, old_key, stored) = row.ok_or(WorldError::NotFound)?;
    let (Some(old), Some(old_key), Some(look)) = (old, old_key, Look::from_db(stored)) else {
        return Err(WorldError::NoCharacter);
    };
    // Re-read under the user lock (every history row of a player is written under it). A parallel
    // free rename used the free one up: refuse rather than spend a ticket the player did not offer.
    match (free, free_rename(&mut tx, user_id).await?) {
        (true, false) => return Err(WorldError::NoRenameTicket),
        (false, true) => return Ok(None),
        _ => {}
    }
    if old == *nickname {
        return Err(WorldError::SameNickname);
    }
    lock_keys(&mut tx, &[&old_key, key]).await?;
    claimable(&mut tx, user_id, key).await?;
    take_name(&mut tx, user_id, nickname, key).await?;
    record_rename(&mut tx, user_id, (&old, &old_key), (nickname, key), free).await?;
    tx.commit().await?;
    Ok(Some(Character {
        nickname: nickname.clone(),
        look,
    }))
}

/// The history row (append-only, kind `rename`) and the player's event.
async fn record_rename(
    conn: &mut PgConnection,
    user_id: Uuid,
    (old, old_key): (&str, &str),
    (new, key): (&str, &str),
    free: bool,
) -> Result<(), WorldError> {
    sqlx::query(
        "INSERT INTO cf.nickname_history (user_id, old_nickname, old_key, new_nickname, new_key, kind)
         VALUES ($1, $2, $3, $4, $5, 'rename')",
    )
    .bind(user_id)
    .bind(old)
    .bind(old_key)
    .bind(new)
    .bind(key)
    .execute(&mut *conn)
    .await?;
    record_event(
        conn,
        user_id,
        "rename",
        serde_json::json!({ "from": old, "to": new, "free": free }),
    )
    .await
}

pub(crate) async fn take_name(
    conn: &mut PgConnection,
    user_id: Uuid,
    nickname: &str,
    key: &str,
) -> Result<(), WorldError> {
    let done = sqlx::query("UPDATE cf.users SET nickname = $2, nickname_key = $3 WHERE id = $1")
        .bind(user_id)
        .bind(nickname)
        .bind(key)
        .execute(conn)
        .await;
    match done {
        Ok(_) => Ok(()),
        Err(sqlx::Error::Database(db)) if db.constraint() == Some(KEY_INDEX) => {
            Err(WorldError::NicknameTaken)
        }
        Err(e) => Err(e.into()),
    }
}

async fn release_claim(pool: &PgPool, user_id: Uuid) -> Result<(), WorldError> {
    let mut tx = pool.begin().await?;
    caps::release(&mut tx, user_id, CAP_NICK_CLAIM, 1).await?;
    tx.commit().await?;
    Ok(())
}

/// `GET /me/nickname-check`: is this name free for this account right now? Counts against a daily
/// cap so the endpoint is no free oracle of other people's names. A name the account itself holds
/// (another capitalisation of its own) is free for it; a name another account holds — taken now,
/// or given up within `HOLD_DAYS` — is taken.
pub async fn check(pool: &PgPool, user_id: Uuid, raw_nickname: &str) -> Result<NickCheck, WorldError> {
    let mut tx = pool.begin().await?;
    caps::consume(&mut tx, user_id, CAP_NICK_CHECK, NICK_CHECKS_PER_DAY).await?;
    tx.commit().await?;
    let checked = match nickname::validate(raw_nickname) {
        Ok(c) => c,
        Err(WorldError::InvalidNickname(why)) => return Ok(NickCheck::Invalid(why)),
        Err(WorldError::NicknameNotAllowed) => return Ok(NickCheck::NotAllowed),
        Err(other) => return Err(other),
    };
    let mut conn = pool.acquire().await?;
    if blocked(&mut conn, &checked.key).await? {
        return Ok(NickCheck::NotAllowed);
    }
    let taken: bool =
        sqlx::query_scalar("SELECT EXISTS (SELECT 1 FROM cf.users WHERE nickname_key = $1 AND id <> $2)")
            .bind(&checked.key)
            .bind(user_id)
            .fetch_one(&mut *conn)
            .await?;
    Ok(
        if taken || held_by_other(&mut conn, user_id, &checked.key).await? {
            NickCheck::Taken
        } else {
            NickCheck::Available
        },
    )
}
