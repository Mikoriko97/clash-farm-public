//! Paid look change (0029, ADR-038): after the character exists, a new look costs one
//! `restyle_ticket`. The ticket is an ordinary inventory item bought for CLASH through the
//! verified order flow (like the island permits), so the payment path is untouched; this module
//! only turns a ticket into a new look, both in one transaction under the user's row lock. The
//! nickname never changes here (ADR-025). The account comes from the session only.

use sqlx::PgPool;
use uuid::Uuid;

use crate::character::{Character, Look};
use crate::error::WorldError;
use crate::inventory;
use crate::plots::record_event;

/// Catalog id of the ticket (migration 0029).
pub const TICKET: &str = "restyle_ticket";

/// `POST /me/look { look }`: validates, then spends one ticket and stores the look. A refused
/// change (invalid look, the same look, no character, no ticket) spends nothing.
pub async fn restyle(pool: &PgPool, user_id: Uuid, look: Look) -> Result<Character, WorldError> {
    if !look.is_valid() {
        return Err(WorldError::InvalidLook);
    }
    let mut tx = pool.begin().await?;
    // The ticket row first, then the user row `FOR NO KEY UPDATE`: a fulfilment holds the user
    // row's KEY SHARE (the ledger's foreign key) while it adds the ticket, and a FOR UPDATE here
    // would wait on it while holding the ticket row — a deadlock (seen in rename_flow). NO KEY
    // UPDATE does not conflict with KEY SHARE and still serialises against other changes of this
    // row. The ticket's atomic `qty >= 1` serialises two changes: one ticket, one change. A
    // refusal below rolls the ticket back.
    inventory::take_one(&mut tx, user_id, TICKET)
        .await
        .map_err(|e| match e {
            WorldError::InsufficientItems => WorldError::NoRestyleTicket,
            other => other,
        })?;
    let row: Option<(Option<String>, Option<serde_json::Value>)> =
        sqlx::query_as("SELECT nickname, look FROM cf.users WHERE id = $1 FOR NO KEY UPDATE")
            .bind(user_id)
            .fetch_optional(&mut *tx)
            .await?;
    let (nickname, stored) = row.ok_or(WorldError::NotFound)?;
    let nickname = nickname.ok_or(WorldError::NoCharacter)?;
    if Look::from_db(stored) == Some(look) {
        return Err(WorldError::SameLook);
    }
    let value = serde_json::to_value(look).map_err(|_| WorldError::InvalidLook)?;
    sqlx::query("UPDATE cf.users SET look = $2 WHERE id = $1")
        .bind(user_id)
        .bind(&value)
        .execute(&mut *tx)
        .await?;
    record_event(&mut tx, user_id, "restyle", serde_json::json!({ "look": value })).await?;
    tx.commit().await?;
    Ok(Character { nickname, look })
}
