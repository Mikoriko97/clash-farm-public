//! `GET /api/v1/me` — the caller's own profile. Ownership is implicit: the
//! user id comes from the session, never from the request.

use axum::extract::State;
use axum::Json;
use serde::Serialize;
use sqlx::Row;

use crate::error::ApiError;
use crate::middleware::session::AuthUser;
use crate::state::AppState;

#[derive(Debug, Serialize)]
pub struct MeResponse {
    pub user: UserView,
    /// Materialised balances (`credits`, ...). Never computed client-side.
    pub balances: Vec<cf_ledger::Balance>,
}

#[derive(Debug, Serialize)]
pub struct UserView {
    pub id: uuid::Uuid,
    /// `None` for a guest (0028) until a wallet is linked.
    pub wallet: Option<String>,
    pub nickname: Option<String>,
    /// Palette indexes chosen with the nickname; `null` until the character is created.
    pub look: Option<cf_world::character::Look>,
    pub reputation: i32,
    pub flags: serde_json::Value,
    /// A moderator reset the name (0031, ADR-040): the next rename needs no ticket.
    pub rename_free: bool,
}

/// `users.flags` keys a player may see. Admin marks (`suspect`, `note`, `by`, `at` from the
/// admin panel) stay server-side: a flagged player must not learn it, and the note may
/// name the admin wallet. Add a key here only for a player-facing flag.
const PLAYER_VISIBLE_FLAGS: &[&str] = &[];

fn player_flags(all: serde_json::Value) -> serde_json::Value {
    let mut out = serde_json::Map::new();
    if let serde_json::Value::Object(map) = all {
        for (k, v) in map {
            if PLAYER_VISIBLE_FLAGS.contains(&k.as_str()) {
                out.insert(k, v);
            }
        }
    }
    serde_json::Value::Object(out)
}

async fn free_rename(state: &AppState, user_id: uuid::Uuid) -> Result<bool, ApiError> {
    let mut conn = state.pool.acquire().await.map_err(cf_auth::AuthError::from)?;
    Ok(cf_world::moderation::free_rename(&mut conn, user_id).await?)
}

pub async fn me(State(state): State<AppState>, user: AuthUser) -> Result<Json<MeResponse>, ApiError> {
    let row = sqlx::query("SELECT id, wallet, nickname, look, reputation, flags FROM cf.users WHERE id = $1")
        .bind(user.user_id)
        .fetch_optional(&state.pool)
        .await
        .map_err(cf_auth::AuthError::from)?
        .ok_or_else(|| ApiError::unauthorized("session_invalid"))?;
    let view = UserView {
        id: row.try_get("id").map_err(cf_auth::AuthError::from)?,
        wallet: row.try_get("wallet").map_err(cf_auth::AuthError::from)?,
        nickname: row.try_get("nickname").map_err(cf_auth::AuthError::from)?,
        look: cf_world::character::Look::from_db(row.try_get("look").map_err(cf_auth::AuthError::from)?),
        reputation: row.try_get("reputation").map_err(cf_auth::AuthError::from)?,
        flags: player_flags(row.try_get("flags").map_err(cf_auth::AuthError::from)?),
        rename_free: free_rename(&state, user.user_id).await?,
    };
    let balances = cf_ledger::balances(&state.pool, user.user_id)
        .await
        .map_err(|e| ApiError::from(cf_world::WorldError::from(e)))?;
    Ok(Json(MeResponse { user: view, balances }))
}
