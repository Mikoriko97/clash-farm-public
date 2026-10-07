//! Per-connection limits of the presence socket (docs/SECURITY.md §3, MAINLAND §8):
//! a token bucket of `MSG_BURST` messages refilled at `MSGS_PER_SEC`, zone bounds,
//! allowed animations and the `MAX_SPEED_DM_PER_S` speed check between two poses,
//! plus the view of neighbours sent back. Integers only.

use std::collections::HashMap;
use std::time::{Duration, Instant};

use serde::Deserialize;

use super::{
    anim_ok, dist2, is_neighbour, PeerUpdate, PeerView, CAVE_ENTRY, DOCK, ISLAND_SPAWN, MAX_PEERS,
    MAX_SPEED_DM_PER_S, MSGS_PER_SEC, MSG_BURST, PEER_STALE_MS, ZONES,
};

/// How close to the zone's respawn point a jump must land to skip the speed check.
pub(super) const RESPAWN_RADIUS_DM: i64 = 30;

/// Where the game puts the character back by itself (`WorldBounds.spawn` after a fall off the world,
/// or a `reset`): the island spawn, the mainland dock, the cave mouth. A jump there is not a speed
/// violation (ADR-036) — standing at the spawn gains nothing, and without this the socket kept
/// measuring every later pose from the spot before the fall until it was closed.
pub(super) fn at_respawn(zone: &str, x_dm: i32, z_dm: i32) -> bool {
    let point = match zone {
        "island" => ISLAND_SPAWN,
        "mainland" => DOCK,
        "cave" => CAVE_ENTRY,
        _ => return false,
    };
    dist2(point.0, point.1, x_dm, z_dm) <= RESPAWN_RADIUS_DM * RESPAWN_RADIUS_DM
}

#[derive(Debug, Deserialize)]
pub(super) struct PosMsg {
    pub(super) t: String,
    pub(super) zone: String,
    pub(super) x_dm: i32,
    pub(super) z_dm: i32,
    pub(super) yaw_deg: i32,
    pub(super) anim: String,
}

/// Per-connection state: last accepted pose, limits, and the view of neighbours.
pub(super) struct Conn {
    pub(super) zone: Option<String>,
    pub(super) last: Option<(i32, i32, Instant)>,
    /// Thousandths of a message: `MSG_BURST` at once, `MSGS_PER_SEC` sustained (integers only).
    pub(super) tokens_m: i64,
    pub(super) refilled: Instant,
    pub(super) violations: u32,
    /// Last time the position went to Redis for reach checks.
    pub(super) stored: Option<Instant>,
    /// A frame that started somewhere impossible: the client is told where it stands.
    pub(super) pending_reset: Option<(String, i32, i32)>,
    /// Every peer heard of (zone, view, last seen); the zone filter is applied in `view`
    /// so an update that arrives before our own first pose is not lost.
    pub(super) peers: HashMap<String, (String, PeerView, Instant)>,
    pub(super) last_sent: Vec<PeerView>,
}

impl Conn {
    pub(super) fn new() -> Self {
        Self {
            zone: None,
            last: None,
            tokens_m: i64::from(MSG_BURST) * 1000,
            refilled: Instant::now(),
            violations: 0,
            stored: None,
            pending_reset: None,
            peers: HashMap::new(),
            last_sent: Vec::new(),
        }
    }

    pub(super) fn take_token(&mut self) -> bool {
        let now = Instant::now();
        let elapsed_ms = i64::try_from(now.duration_since(self.refilled).as_millis()).unwrap_or(i64::MAX);
        self.refilled = now;
        let refill = elapsed_ms.saturating_mul(i64::from(MSGS_PER_SEC));
        self.tokens_m = self
            .tokens_m
            .saturating_add(refill)
            .min(i64::from(MSG_BURST) * 1000);
        if self.tokens_m >= 1000 {
            self.tokens_m -= 1000;
            true
        } else {
            false
        }
    }

    /// Bounds, zone, anim and speed; returns the accepted message or the violation name.
    pub(super) fn validate(&mut self, msg: &PosMsg) -> Result<(), &'static str> {
        if msg.t != "pos" {
            return Err("type");
        }
        if !ZONES.contains(&msg.zone.as_str()) {
            return Err("zone");
        }
        let limit = super::zone_limit_dm(&msg.zone);
        if msg.x_dm.abs() > limit || msg.z_dm.abs() > limit {
            return Err("bounds");
        }
        if !anim_ok(&msg.anim) {
            return Err("anim");
        }
        let now = Instant::now();
        if let Some((lx, lz, at)) = self.last {
            if self.zone.as_deref() == Some(msg.zone.as_str()) {
                let dt_ms = i64::try_from(now.duration_since(at).as_millis()).unwrap_or(i64::MAX);
                // dist² · 1000² ≤ (v · dt)²  in integer arithmetic (dm, ms).
                let allowed = MAX_SPEED_DM_PER_S
                    .saturating_mul(dt_ms.max(50))
                    .saturating_add(10_000);
                if dist2(lx, lz, msg.x_dm, msg.z_dm).saturating_mul(1_000_000)
                    > allowed.saturating_mul(allowed)
                    && !at_respawn(&msg.zone, msg.x_dm, msg.z_dm)
                {
                    return Err("speed");
                }
            }
        }
        self.last = Some((msg.x_dm, msg.z_dm, now));
        self.zone = Some(msg.zone.clone());
        Ok(())
    }

    pub(super) fn absorb(&mut self, me: &str, update: &PeerUpdate) {
        if update.id == me {
            return;
        }
        if update.gone {
            self.peers.remove(&update.id);
            return;
        }
        let view = PeerView {
            id: update.id.clone(),
            nick: update.nick.clone(),
            look: update.look,
            x_dm: update.x_dm,
            z_dm: update.z_dm,
            yaw_deg: update.yaw_deg,
            anim: update.anim.clone(),
        };
        self.peers
            .insert(update.id.clone(), (update.zone.clone(), view, Instant::now()));
    }

    /// Neighbours in the 3×3 cells, nearest first, at most `MAX_PEERS`; stale ones dropped.
    pub(super) fn view(&mut self) -> Vec<PeerView> {
        let (Some((mx, mz, _)), Some(zone)) = (self.last, self.zone.as_deref()) else {
            return Vec::new();
        };
        let stale = Duration::from_millis(PEER_STALE_MS);
        let now = Instant::now();
        self.peers
            .retain(|_, (_, _, seen)| now.duration_since(*seen) < stale);
        let mut near: Vec<&PeerView> = self
            .peers
            .values()
            .filter(|(z, _, _)| z == zone)
            .map(|(_, v, _)| v)
            .filter(|v| is_neighbour(mx, mz, v.x_dm, v.z_dm))
            .collect();
        near.sort_by_key(|v| dist2(mx, mz, v.x_dm, v.z_dm));
        near.truncate(MAX_PEERS);
        near.into_iter().cloned().collect()
    }
}

#[cfg(test)]
mod tests {
    use super::{at_respawn, Conn, PosMsg};

    fn pos(zone: &str, x_dm: i32, z_dm: i32) -> PosMsg {
        PosMsg {
            t: "pos".into(),
            zone: zone.into(),
            x_dm,
            z_dm,
            yaw_deg: 0,
            anim: "idle".into(),
        }
    }

    #[test]
    fn a_fall_back_to_the_spawn_is_not_a_teleport_anywhere_else_is() {
        assert!(
            at_respawn("mainland", 0, 4_000) && at_respawn("island", -20, 0) && at_respawn("cave", 0, 80)
        );
        assert!(!at_respawn("mainland", 0, 4_100) && !at_respawn("moon", 0, 0));
        // The mine gate is a door between zones, not a respawn point: no shortcut to the cave.
        assert!(!at_respawn("mainland", 2_800, -1_900) && !at_respawn("mainland", 2_800, -2_000));
        let mut conn = Conn::new();
        assert_eq!(conn.validate(&pos("mainland", 2_000, 0)), Ok(()));
        // 400 m in no time: refused, and the socket still measures from where it was.
        assert_eq!(conn.validate(&pos("mainland", -2_000, 0)), Err("speed"));
        assert_eq!(conn.last.map(|(x, z, _)| (x, z)), Some((2_000, 0)));
        // The same jump onto the dock (the respawn point) is accepted and re-anchors the socket.
        assert_eq!(conn.validate(&pos("mainland", 3, 4_002)), Ok(()));
        assert_eq!(conn.last.map(|(x, z, _)| (x, z)), Some((3, 4_002)));
        assert_eq!(conn.validate(&pos("mainland", 8, 4_002)), Ok(()));
    }
}
