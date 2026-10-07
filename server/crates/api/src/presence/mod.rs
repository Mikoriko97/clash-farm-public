//! Player presence (docs/MAINLAND.md §8, M1): who stands where, so clients can
//! draw each other. Nothing here is economic: positions are integers in
//! decimetres, validated for bounds and speed, fanned out to neighbours at 5 Hz.
//! One process fans out through a `broadcast` channel; several api instances
//! meet through a Redis pub/sub channel per zone (`presence:<zone>`), each
//! message tagged with the publishing instance so nobody echoes its own.

mod conn;
mod handshake;
pub mod ws;

use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex, PoisonError};

use cf_world::character::Look;
use redis::aio::ConnectionManager;
use serde::{Deserialize, Serialize};
use tokio::sync::{broadcast, Notify};
use uuid::Uuid;

/// Zones a client may report (docs/MAINLAND.md): the island and the mainland.
pub const ZONES: [&str; 3] = ["island", "mainland", "cave"];
/// Neighbour cell edge (docs/MAINLAND.md §8), in decimetres.
pub const CELL_DM: i32 = 640;
/// Furthest coordinate a client may report on the island (56 m island + margin).
pub const ISLAND_LIMIT_DM: i32 = 600;
/// …and on the mainland (1 × 1 km map + a strip of sea).
pub const MAINLAND_LIMIT_DM: i32 = 5_300;
/// …and inside a cave scene (cave-local coordinates).
pub const CAVE_LIMIT_DM: i32 = 1_000;

/// Coordinate bound of a zone, or 0 for an unknown one.
pub fn zone_limit_dm(zone: &str) -> i32 {
    match zone {
        "island" => ISLAND_LIMIT_DM,
        "mainland" => MAINLAND_LIMIT_DM,
        "cave" => CAVE_LIMIT_DM,
        _ => 0,
    }
}
/// Max speed the server believes: 6 m/s.
pub const MAX_SPEED_DM_PER_S: i64 = 60;
/// Inbound position messages per second (token bucket) and the burst it tolerates.
pub const MSGS_PER_SEC: u32 = 12;
pub const MSG_BURST: u32 = 24;
/// Violations (bounds, speed, rate, garbage) before the socket is closed.
pub const MAX_VIOLATIONS: u32 = 20;
/// Most peers one client is told about.
pub const MAX_PEERS: usize = 50;
/// A peer silent for this long disappears from views.
pub const PEER_STALE_MS: u64 = 5_000;
/// Longest anim name accepted (clip names are short snake_case).
pub const ANIM_MAX_LEN: usize = 24;
/// Open sockets one api instance accepts before refusing new ones.
pub const MAX_CONNS_PER_INSTANCE: usize = 2_000;
/// Handshakes per user per minute (a reconnect storm is not a game).
pub const CONNECTS_PER_MIN: u64 = 10;
/// The last accepted position is kept this long so a reconnect resumes from it (ADR-036): a tab
/// left in the background stops sending poses, and its socket is renewed meanwhile — with a short
/// TTL the reconnect found nothing and the zone door sent the character back to the dock.
pub const POSITION_TTL_SECS: u64 = 3_600;
/// …but a `gather` only trusts a position this fresh (docs/MAINLAND.md §5).
pub const POSITION_FRESH_MS: i64 = 10_000;
/// …and written at most this often per socket.
pub const POSITION_STORE_MS: u64 = 500;
/// A socket without a stored position must start this close to the zone's entry.
pub const ENTRY_RADIUS_DM: i64 = 300;

pub const ISLAND_SPAWN: (i32, i32) = (-20, 0);
pub const DOCK: (i32, i32) = (0, 4_000);
/// Ten metres south of the `mine_gate` landmark (2800, -2000): where the cave lets out.
pub const MINE_GATE: (i32, i32) = (2_800, -1_900);
pub const CAVE_ENTRY: (i32, i32) = (0, 80);

/// Where a player may enter `to` coming from `from` (`None` = no zone yet):
/// the island spawn; the mainland dock, or the mine gate when climbing out of
/// the cave; the cave mouth only from the mainland (the caller also checks the
/// player stood at the gate and had discovered it).
pub fn zone_entries(to: &str, from: Option<&str>) -> Vec<(i32, i32)> {
    match (to, from) {
        ("mainland", Some("cave")) => vec![MINE_GATE],
        ("mainland", _) => vec![DOCK],
        ("cave", Some("mainland")) => vec![CAVE_ENTRY],
        ("cave", _) => Vec::new(),
        _ => vec![ISLAND_SPAWN],
    }
}

/// Where presence last saw a player (for server-side reach checks).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LastPosition {
    pub zone: String,
    pub x_dm: i32,
    pub z_dm: i32,
    /// Milliseconds since it was stored (server clock).
    pub age_ms: i64,
}

fn position_key(user_id: Uuid) -> String {
    format!("presence:pos:{user_id}")
}

fn now_ms() -> i64 {
    i64::try_from(
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|d| d.as_millis())
            .unwrap_or(0),
    )
    .unwrap_or(i64::MAX)
}

/// The player's last accepted position, if any socket reported one recently.
pub async fn last_position(redis: &ConnectionManager, user_id: Uuid) -> Option<LastPosition> {
    let mut con = redis.clone();
    let raw: Option<String> = redis::cmd("GET")
        .arg(position_key(user_id))
        .query_async(&mut con)
        .await
        .ok()
        .flatten();
    parse_position(&raw?, now_ms())
}

fn parse_position(raw: &str, now: i64) -> Option<LastPosition> {
    let mut parts = raw.split(':');
    let zone = parts.next()?.to_owned();
    let x_dm = parts.next()?.parse().ok()?;
    let z_dm = parts.next()?.parse().ok()?;
    let at: i64 = parts.next()?.parse().ok()?;
    Some(LastPosition {
        zone,
        x_dm,
        z_dm,
        age_ms: now.saturating_sub(at).max(0),
    })
}

/// The one presence socket a user holds on this instance; a newer socket of the
/// same user wakes the older one so it closes (docs/SECURITY.md §3).
#[derive(Debug)]
pub struct Slot {
    user_id: Uuid,
    pub closed: Arc<Notify>,
}

/// One player's latest pose, as shared between connections and instances.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PeerUpdate {
    /// Publishing api instance; the Redis bridge drops its own echoes.
    pub origin: String,
    pub zone: String,
    /// Public short id (never the wallet or the user id).
    pub id: String,
    pub nick: String,
    /// Absent in updates from an older instance (rolling deploy) and before the character exists.
    #[serde(default)]
    pub look: Option<Look>,
    pub x_dm: i32,
    pub z_dm: i32,
    pub yaw_deg: i16,
    pub anim: String,
    /// The player left (socket closed): remove from views.
    pub gone: bool,
}

/// What a client receives about one neighbour.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct PeerView {
    pub id: String,
    pub nick: String,
    pub look: Option<Look>,
    pub x_dm: i32,
    pub z_dm: i32,
    pub yaw_deg: i16,
    pub anim: String,
}

#[derive(Clone)]
pub struct PresenceHub {
    tx: broadcast::Sender<Arc<PeerUpdate>>,
    redis: ConnectionManager,
    /// Random per process; tags every published update.
    pub instance: String,
    slots: Arc<Mutex<HashMap<Uuid, Arc<Notify>>>>,
    conns: Arc<AtomicUsize>,
}

impl std::fmt::Debug for PresenceHub {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PresenceHub")
            .field("instance", &self.instance)
            .finish_non_exhaustive()
    }
}

impl PresenceHub {
    pub fn new(redis: ConnectionManager) -> Self {
        let (tx, _) = broadcast::channel(4096);
        Self {
            tx,
            redis,
            instance: cf_auth::session::random_token()[..12].to_owned(),
            slots: Arc::new(Mutex::new(HashMap::new())),
            conns: Arc::new(AtomicUsize::new(0)),
        }
    }

    /// Takes the user's slot on this instance, waking any older socket of theirs;
    /// `None` when the instance is full.
    pub fn claim(&self, user_id: Uuid) -> Option<Slot> {
        let mut slots = self.slots.lock().unwrap_or_else(PoisonError::into_inner);
        if !slots.contains_key(&user_id) && slots.len() >= MAX_CONNS_PER_INSTANCE {
            return None;
        }
        let closed = Arc::new(Notify::new());
        if let Some(old) = slots.insert(user_id, closed.clone()) {
            old.notify_one();
        }
        self.conns.store(slots.len(), Ordering::Relaxed);
        Some(Slot { user_id, closed })
    }

    /// Frees the slot unless a newer socket of the same user already took it.
    pub fn release(&self, slot: &Slot) {
        let mut slots = self.slots.lock().unwrap_or_else(PoisonError::into_inner);
        if slots
            .get(&slot.user_id)
            .is_some_and(|cur| Arc::ptr_eq(cur, &slot.closed))
        {
            slots.remove(&slot.user_id);
        }
        self.conns.store(slots.len(), Ordering::Relaxed);
    }

    /// Sockets open on this instance (for logs and health).
    pub fn connections(&self) -> usize {
        self.conns.load(Ordering::Relaxed)
    }

    pub fn redis(&self) -> &ConnectionManager {
        &self.redis
    }

    /// Remembers where the player stands; it outlives the socket (`POSITION_TTL_SECS`) so the next
    /// one resumes from it, while actions only trust it for `POSITION_FRESH_MS`.
    pub async fn store_position(&self, user_id: Uuid, zone: &str, x_dm: i32, z_dm: i32) {
        let mut con = self.redis.clone();
        let stored: Result<(), redis::RedisError> = redis::cmd("SET")
            .arg(position_key(user_id))
            .arg(format!("{zone}:{x_dm}:{z_dm}:{}", now_ms()))
            .arg("EX")
            .arg(POSITION_TTL_SECS)
            .query_async(&mut con)
            .await;
        if let Err(err) = stored {
            tracing::debug!(error = %err, "presence: position store failed");
        }
    }

    pub fn subscribe(&self) -> broadcast::Receiver<Arc<PeerUpdate>> {
        self.tx.subscribe()
    }

    /// Local fan-out first (never blocks), then Redis for the other instances.
    pub async fn publish(&self, update: PeerUpdate) {
        let channel = format!("presence:{}", update.zone);
        let payload = serde_json::to_string(&update).unwrap_or_default();
        let _ = self.tx.send(Arc::new(update));
        let mut con = self.redis.clone();
        let published: Result<i64, redis::RedisError> = redis::cmd("PUBLISH")
            .arg(channel)
            .arg(payload)
            .query_async(&mut con)
            .await;
        if let Err(err) = published {
            tracing::debug!(error = %err, "presence publish to redis failed");
        }
    }

    /// Feeds updates from the other instances into the local channel. Runs for
    /// the life of the process; reconnects with a short pause when Redis drops.
    pub async fn run_bridge(self, client: redis::Client) {
        use futures_util::StreamExt;
        loop {
            let mut pubsub = match client.get_async_pubsub().await {
                Ok(p) => p,
                Err(err) => {
                    tracing::warn!(error = %err, "presence bridge: redis pubsub unavailable");
                    tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                    continue;
                }
            };
            if let Err(err) = pubsub.psubscribe("presence:*").await {
                tracing::warn!(error = %err, "presence bridge: psubscribe failed");
                tokio::time::sleep(std::time::Duration::from_secs(2)).await;
                continue;
            }
            let mut stream = pubsub.into_on_message();
            while let Some(msg) = stream.next().await {
                let payload: String = match msg.get_payload() {
                    Ok(p) => p,
                    Err(_) => continue,
                };
                let Ok(update) = serde_json::from_str::<PeerUpdate>(&payload) else {
                    continue;
                };
                if update.origin != self.instance {
                    let _ = self.tx.send(Arc::new(update));
                }
            }
            tracing::warn!("presence bridge: redis stream ended; reconnecting");
        }
    }
}

/// Neighbour cell of a coordinate (floor division keeps negatives contiguous).
pub fn cell_of(dm: i32) -> i32 {
    dm.div_euclid(CELL_DM)
}

/// Same cell or one of the eight around it.
pub fn is_neighbour(ax: i32, az: i32, bx: i32, bz: i32) -> bool {
    (cell_of(ax) - cell_of(bx)).abs() <= 1 && (cell_of(az) - cell_of(bz)).abs() <= 1
}

/// Squared distance in dm², in i64 so far-apart coordinates never overflow.
pub fn dist2(ax: i32, az: i32, bx: i32, bz: i32) -> i64 {
    let dx = i64::from(ax) - i64::from(bx);
    let dz = i64::from(az) - i64::from(bz);
    dx * dx + dz * dz
}

/// Allowed clip name: short snake_case.
pub fn anim_ok(anim: &str) -> bool {
    !anim.is_empty()
        && anim.len() <= ANIM_MAX_LEN
        && anim
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn cells_and_neighbours() {
        assert_eq!(cell_of(0), 0);
        assert_eq!(cell_of(639), 0);
        assert_eq!(cell_of(640), 1);
        assert_eq!(cell_of(-1), -1);
        assert!(is_neighbour(0, 0, 639, 639));
        assert!(is_neighbour(0, 0, -1, 0));
        assert!(!is_neighbour(0, 0, 1280, 0));
        assert_eq!(dist2(0, 0, 30, 40), 2500);
    }

    #[test]
    fn positions_round_trip_and_reject_garbage() {
        assert_eq!(
            parse_position("mainland:-120:4050:1000", 1600),
            Some(LastPosition {
                zone: "mainland".into(),
                x_dm: -120,
                z_dm: 4050,
                age_ms: 600
            })
        );
        assert!(parse_position("island:1:2", 0).is_none());
        assert!(parse_position("island:a:b:1", 0).is_none());
        assert_eq!(zone_entries("mainland", None), vec![DOCK]);
        assert_eq!(zone_entries("mainland", Some("island")), vec![DOCK]);
        assert_eq!(zone_entries("mainland", Some("cave")), vec![MINE_GATE]);
        assert_eq!(zone_entries("cave", Some("mainland")), vec![CAVE_ENTRY]);
        assert!(zone_entries("cave", Some("island")).is_empty());
        assert!(zone_entries("cave", None).is_empty());
        assert_eq!(zone_entries("island", Some("mainland")), vec![ISLAND_SPAWN]);
    }

    #[test]
    fn anim_names() {
        assert!(anim_ok("walk_forward"));
        assert!(anim_ok("idle"));
        assert!(!anim_ok(""));
        assert!(!anim_ok("Walk"));
        assert!(!anim_ok("a".repeat(25).as_str()));
        assert!(!anim_ok("run forward"));
    }
}
