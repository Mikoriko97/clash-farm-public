//! Opening the presence socket: only our browser Origin may connect, and the
//! player's public identity (short keyed-hash id and nickname) is loaded once.

use axum::http::header::ORIGIN;
use axum::http::HeaderMap;
use cf_common::hashing::keyed_sha256;

use super::ws::Identity;
use crate::error::ApiError;
use crate::state::AppState;

/// Browsers always send Origin on a WebSocket handshake; only ours may connect.
pub(super) fn check_origin(state: &AppState, headers: &HeaderMap) -> Result<(), ApiError> {
    let origin = headers.get(ORIGIN).and_then(|v| v.to_str().ok()).unwrap_or("");
    if state.config.ws_origins.iter().any(|o| o == origin) {
        Ok(())
    } else {
        let shown: String = origin.chars().take(128).collect();
        tracing::warn!(origin = %shown, "presence: origin refused");
        Err(ApiError::new(
            axum::http::StatusCode::FORBIDDEN,
            "origin_forbidden",
            "origin not allowed",
        ))
    }
}

/// Has the account's name changed since the handshake (a moderator's reset, 0031)? Checked with
/// the session every 30 s; a read error keeps the socket (the session check guards access).
pub(super) async fn identity_stale(state: &AppState, user_id: uuid::Uuid, me: &Identity) -> bool {
    if me.look.is_none() {
        // No character at the handshake: the client reconnects by itself once it creates one.
        return false;
    }
    match cf_world::character::load(&state.pool, user_id).await {
        Ok(Some(c)) => c.nickname != me.nick,
        Ok(None) => true,
        Err(e) => {
            tracing::warn!(error = %e, "presence: identity check failed");
            false
        }
    }
}

/// Short id = keyed hash of the user id (stable, never the wallet); name and look from the
/// account's character, or a placeholder from the same hash until it has created one (0025).
pub(super) async fn load_identity(state: &AppState, user_id: uuid::Uuid) -> Result<Identity, ApiError> {
    let character = cf_world::character::load(&state.pool, user_id).await?;
    // Own domain under the cookie secret, so no other identifier shares this hash.
    let material = [b"presence-id".as_slice(), user_id.as_bytes()].concat();
    let digest = keyed_sha256(state.config.cookie_secret.expose().as_bytes(), &material);
    let id = hex::encode(&digest[..6]);
    Ok(match character {
        Some(c) => Identity {
            id,
            nick: c.nickname,
            look: Some(c.look),
        },
        None => Identity {
            nick: format!("Farmer #{}", &id[..4]),
            id,
            look: None,
        },
    })
}
