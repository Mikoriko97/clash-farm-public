//! `GET /ws/presence`: the cookie session opens the socket (browser Origin must be
//! ours), the client streams its pose, the server streams the neighbours back.
//! Limits (docs/SECURITY.md §3, MAINLAND §8): 12 msg/s, 6 m/s, island bounds,
//! 20 violations → close, session re-checked every 30 s, ≤ 50 peers per view,
//! one socket per user per instance, 10 handshakes/min, frames ≤ 1 KiB.

use std::time::{Duration, Instant};

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::Response;
use cf_world::character::Look;
use futures_util::{SinkExt, StreamExt};
use tokio::sync::broadcast;

use super::conn::{Conn, PosMsg};
use super::handshake::{check_origin, identity_stale, load_identity};
use super::{dist2, PeerUpdate, PresenceHub, CONNECTS_PER_MIN, ENTRY_RADIUS_DM, MAX_VIOLATIONS};
use crate::error::ApiError;
use crate::middleware::now_ms;
use crate::middleware::session::AuthUser;
use crate::state::AppState;

const VIEW_TICK: Duration = Duration::from_millis(200);
/// Failed session checks in a row (one per 30 s) before the socket closes anyway.
const SESSION_CHECK_FAILURES: u8 = 3;
const SESSION_CHECK: Duration = Duration::from_secs(30);
/// A client that stops reading is dropped instead of parking the task.
const SEND_TIMEOUT: Duration = Duration::from_secs(5);
const MAX_FRAME: usize = 1024;
/// Tells the client its name changed elsewhere (a moderator's reset): reload `/me`, reconnect.
const IDENTITY_FRAME: &str = r#"{"t":"identity"}"#;

/// Public identity of the connected player: short id, display name and look (0025; both are
/// the placeholder / `None` until the account has created its character).
#[derive(Debug, Clone)]
pub struct Identity {
    pub id: String,
    pub nick: String,
    pub look: Option<Look>,
}

pub async fn presence(
    State(state): State<AppState>,
    user: AuthUser,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> Result<Response, ApiError> {
    check_origin(&state, &headers)?;
    state
        .limiter
        .hit(
            "presence_user",
            &user.user_id.to_string(),
            CONNECTS_PER_MIN,
            60,
            now_ms(),
        )
        .await?;
    let identity = load_identity(&state, user.user_id).await?;
    Ok(ws
        .max_message_size(MAX_FRAME)
        .max_frame_size(MAX_FRAME)
        .on_upgrade(move |socket| run(state, user, identity, socket)))
}

type Sink = futures_util::stream::SplitSink<WebSocket, Message>;
type Stream = futures_util::stream::SplitStream<WebSocket>;

/// Sends one text frame; false when the client is gone or stopped reading.
async fn send_text(sink: &mut Sink, text: String) -> bool {
    tokio::time::timeout(SEND_TIMEOUT, sink.send(Message::Text(text.into())))
        .await
        .is_ok_and(|r| r.is_ok())
}

async fn run(state: AppState, user: AuthUser, me: Identity, socket: WebSocket) {
    let hub: PresenceHub = state.presence.clone();
    let (mut sink, stream) = socket.split();
    let Some(slot) = hub.claim(user.user_id) else {
        tracing::warn!(open = hub.connections(), "presence: instance full");
        let busy = serde_json::json!({ "t": "error", "code": "busy" }).to_string();
        let _ = send_text(&mut sink, busy).await;
        let _ = sink.close().await;
        return;
    };
    // The stored position outlives the socket (ADR-036): the next one resumes from it, so a
    // reconnect neither teleports nor sends the character back to the zone's entry.
    serve(&hub, &state, &user, &me, &slot, sink, stream).await;
    hub.release(&slot);
}

async fn serve(
    hub: &PresenceHub,
    state: &AppState,
    user: &AuthUser,
    me: &Identity,
    slot: &super::Slot,
    mut sink: Sink,
    mut stream: Stream,
) {
    let mut rx = hub.subscribe();
    let mut conn = Conn::new();
    let hello =
        serde_json::json!({ "t": "hello", "id": me.id, "nick": me.nick, "look": me.look }).to_string();
    if !send_text(&mut sink, hello).await {
        return;
    }
    let mut view_tick = tokio::time::interval(VIEW_TICK);
    // Spread by user over the 30 s (after a restart every socket opens at once): the session and
    // name checks then reach the database evenly, not in one burst.
    let b = user.user_id.as_bytes();
    let spread = Duration::from_millis(u64::from(u16::from_le_bytes([b[0], b[1]])) % 30_000);
    let mut session_tick = tokio::time::interval_at(tokio::time::Instant::now() + spread, SESSION_CHECK);
    let mut last_full = Instant::now();
    let mut replaced = false;
    let mut check_failures = 0u8;
    loop {
        tokio::select! {
            incoming = stream.next() => {
                match incoming {
                    Some(Ok(Message::Text(text))) => {
                        if !on_text(hub, state, user, me, &mut conn, &mut sink, &text).await { break; }
                    }
                    Some(Ok(Message::Close(_))) | None | Some(Err(_)) => break,
                    Some(Ok(Message::Binary(_))) => {
                        conn.violations += 1;
                        if conn.violations >= MAX_VIOLATIONS { break; }
                    }
                    Some(Ok(_)) => {}
                }
            }
            update = rx.recv() => {
                match update {
                    Ok(u) => conn.absorb(&me.id, &u),
                    Err(broadcast::error::RecvError::Lagged(_)) => {}
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
            _ = view_tick.tick() => {
                if !send_view(&mut conn, &mut sink, &mut last_full).await { break; }
            }
            () = slot.closed.notified() => {
                tracing::info!(id = %me.id, "presence: replaced by a newer socket");
                // The newer socket carries the same character on: a "gone" now would blink it out.
                replaced = true;
                break;
            }
            _ = session_tick.tick() => {
                if !still_current(state, user, me, &mut sink, &mut check_failures).await { break; }
            }
        }
    }
    if !replaced {
        publish_gone(hub, conn.zone.clone(), me).await;
    }
    let _ = sink.close().await;
}

/// One pose frame; false closes the socket (too many violations, or the client stopped reading).
async fn on_text(
    hub: &PresenceHub,
    state: &AppState,
    user: &AuthUser,
    me: &Identity,
    conn: &mut Conn,
    sink: &mut Sink,
    text: &str,
) -> bool {
    if !handle_pos(hub, &state.pool, me, user.user_id, conn, text).await {
        return false;
    }
    match conn.pending_reset.take() {
        Some((zone, x_dm, z_dm)) => {
            let text =
                serde_json::json!({ "t": "reset", "zone": zone, "x_dm": x_dm, "z_dm": z_dm }).to_string();
            send_text(sink, text).await
        }
        None => true,
    }
}

/// The neighbours, when they changed, and in full once a second.
async fn send_view(conn: &mut Conn, sink: &mut Sink, last_full: &mut Instant) -> bool {
    let peers = conn.view();
    let force = last_full.elapsed() >= Duration::from_secs(1);
    if peers != conn.last_sent || force {
        let text = serde_json::json!({ "t": "peers", "peers": peers }).to_string();
        if !send_text(sink, text).await {
            return false;
        }
        conn.last_sent = peers;
        *last_full = Instant::now();
    }
    if force {
        // Honest jitter forgives itself: one violation a second.
        conn.violations = conn.violations.saturating_sub(1);
    }
    true
}

/// Every 30 s: is the session alive, and is the name still the one the others were shown?
async fn still_current(
    state: &AppState,
    user: &AuthUser,
    me: &Identity,
    sink: &mut Sink,
    failures: &mut u8,
) -> bool {
    // The family, not this socket's access token (ADR-036): the token dies every
    // 15 minutes by design, the session ends only by logout, revocation, ban or expiry.
    // A database hiccup is not a logout: only a definite "no" closes the socket at once, and an
    // outage closes it after three checks in a row (~90 s), so a revoked session cannot linger.
    match state.sessions.family_alive(user.family).await {
        Ok(true) => *failures = 0,
        Ok(false) => {
            tracing::info!(id = %me.id, "presence: session gone, closing");
            return false;
        }
        Err(e) => {
            *failures = failures.saturating_add(1);
            tracing::warn!(id = %me.id, error = %e, failures = *failures, "presence: session check failed");
            if *failures >= SESSION_CHECK_FAILURES {
                return false;
            }
        }
    }
    // A moderator's reset (0031, ADR-040) renames a player who is online: the client
    // reloads its character and reconnects; the others see the new name.
    if identity_stale(state, user.user_id, me).await {
        tracing::info!(id = %me.id, "presence: identity changed, closing");
        let _ = send_text(sink, IDENTITY_FRAME.to_owned()).await;
        return false;
    }
    true
}

/// The others drop this character at once (the island is private: nobody else sees it there).
async fn publish_gone(hub: &PresenceHub, zone: Option<String>, me: &Identity) {
    if let Some(zone) = zone.filter(|z| z != "island") {
        hub.publish(PeerUpdate {
            origin: hub.instance.clone(),
            zone,
            id: me.id.clone(),
            nick: me.nick.clone(),
            look: me.look,
            x_dm: 0,
            z_dm: 0,
            yaw_deg: 0,
            anim: String::new(),
            gone: true,
        })
        .await;
    }
}

/// Entering a zone (first frame, or a zone change): resume from the stored
/// position when it is recent and in that zone, otherwise the character must
/// stand at the zone's entry — anything else is told where it really is.
async fn enter_zone(
    hub: &PresenceHub,
    pool: &sqlx::PgPool,
    user_id: uuid::Uuid,
    conn: &mut Conn,
    msg: &PosMsg,
) -> bool {
    if conn.zone.as_deref() == Some(msg.zone.as_str()) {
        return true;
    }
    let stored = super::last_position(hub.redis(), user_id).await;
    if conn.last.is_none() {
        if let Some(p) = &stored {
            if p.zone == msg.zone {
                let age = Duration::from_millis(u64::try_from(p.age_ms).unwrap_or(0));
                conn.last = Some((
                    p.x_dm,
                    p.z_dm,
                    Instant::now().checked_sub(age).unwrap_or_else(Instant::now),
                ));
                conn.zone = Some(msg.zone.clone());
                return true;
            }
        }
    }
    // Where the player comes from: this socket's zone, else the stored one.
    let from = conn
        .zone
        .clone()
        .or_else(|| stored.as_ref().map(|p| p.zone.clone()));
    let mut entries = super::zone_entries(&msg.zone, from.as_deref());
    if msg.zone == "cave" && !entries.is_empty() {
        // The cave opens only to someone who stood at the gate a moment ago and has found it.
        let (gx, gz) = super::MINE_GATE;
        let at_gate = stored.as_ref().is_some_and(|p| {
            p.zone == "mainland"
                && p.age_ms <= super::POSITION_FRESH_MS
                && dist2(gx, gz, p.x_dm, p.z_dm) <= ENTRY_RADIUS_DM * ENTRY_RADIUS_DM
        });
        if !at_gate || !discovered(pool, user_id, "mine_gate").await {
            entries.clear();
        }
    }
    if entries
        .iter()
        .any(|(ex, ez)| dist2(*ex, *ez, msg.x_dm, msg.z_dm) <= ENTRY_RADIUS_DM * ENTRY_RADIUS_DM)
    {
        conn.last = None;
        return true;
    }
    // Refused: send the client where it belongs. A refused cave puts it back on the mainland.
    let reset = match (msg.zone.as_str(), entries.first()) {
        ("cave", _) => ("mainland".to_owned(), super::DOCK),
        (_, Some(e)) => (msg.zone.clone(), *e),
        (_, None) => (msg.zone.clone(), super::ISLAND_SPAWN),
    };
    conn.pending_reset = Some((reset.0, reset.1 .0, reset.1 .1));
    false
}

/// Has the player recorded this landmark (docs/MAINLAND.md §4)?
async fn discovered(pool: &sqlx::PgPool, user_id: uuid::Uuid, landmark: &str) -> bool {
    sqlx::query_scalar::<_, i32>("SELECT 1 FROM cf.discoveries WHERE user_id = $1 AND landmark_id = $2")
        .bind(user_id)
        .bind(landmark)
        .fetch_optional(pool)
        .await
        .ok()
        .flatten()
        .is_some()
}

/// One inbound frame; false = close the socket (too many violations).
async fn handle_pos(
    hub: &PresenceHub,
    pool: &sqlx::PgPool,
    me: &Identity,
    user_id: uuid::Uuid,
    conn: &mut Conn,
    text: &str,
) -> bool {
    if !conn.take_token() {
        return violation(conn, me, "rate");
    }
    let msg: PosMsg = match serde_json::from_str(text) {
        Ok(m) => m,
        Err(_) => return violation(conn, me, "json"),
    };
    let previous = conn.zone.clone();
    if !enter_zone(hub, pool, user_id, conn, &msg).await {
        return violation(conn, me, "entry");
    }
    if let Err(why) = conn.validate(&msg) {
        return violation(conn, me, why);
    }
    if previous.as_deref().is_some_and(|z| z != msg.zone) {
        // Left a zone (mainland → cave): the players there drop the character now, not on a timeout.
        publish_gone(hub, previous, me).await;
    }
    let due = conn
        .stored
        .is_none_or(|at| at.elapsed() >= Duration::from_millis(super::POSITION_STORE_MS));
    if due {
        conn.stored = Some(Instant::now());
        hub.store_position(user_id, &msg.zone, msg.x_dm, msg.z_dm).await;
    }
    // The island is private (owner decision 2026-09-29): its poses feed the zone doors and
    // reach checks but are never fanned out, so nobody appears on someone else's farm.
    if msg.zone == "island" {
        return true;
    }
    hub.publish(PeerUpdate {
        origin: hub.instance.clone(),
        zone: msg.zone,
        id: me.id.clone(),
        nick: me.nick.clone(),
        look: me.look,
        x_dm: msg.x_dm,
        z_dm: msg.z_dm,
        yaw_deg: i16::try_from(msg.yaw_deg.rem_euclid(360)).unwrap_or(0),
        anim: msg.anim,
        gone: false,
    })
    .await;
    true
}

fn violation(conn: &mut Conn, me: &Identity, why: &'static str) -> bool {
    conn.violations += 1;
    tracing::debug!(id = %me.id, why, violations = conn.violations, "presence: rejected pose");
    conn.violations < MAX_VIOLATIONS
}
