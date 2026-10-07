//! Storm-felled trees (docs/MAINLAND.md §4 «Сезонні події»): while a storm
//! slot is on, and for `economy.storm_fallen_secs` after it began, a dozen
//! trees lie in the forests for everyone to strip without an axe. Which trees
//! is a pure function of the weather slot; live state shares `cf.mainland_nodes`
//! with ids `98_<slot>_<i>`.

use time::OffsetDateTime;

use crate::mainland::{self, kind_hits, NodeSpec, CHUNKS, CHUNK_DM};
use crate::params::Params;
use crate::weather;

/// Pseudo chunk column that marks storm node ids.
pub const STORM_CX: i32 = 98;
pub const KIND: &str = "tree_fallen";
pub const TREES_PER_STORM: usize = 12;

fn mix(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = x;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// The storm slot whose felled trees still lie around at `now`, if any: the
/// current slot when it storms, or a recent one within the fallen window.
pub fn active_slot(now: OffsetDateTime, params: Params) -> Option<i64> {
    let period = i64::from(params.weather_period_min.max(1)) * 60;
    let window = i64::from(params.storm_fallen_secs.max(0));
    let current = now.unix_timestamp().div_euclid(period);
    if params.weather_force.is_some() {
        // Pinned weather (demos, tests): felled trees follow the pinned state.
        return (weather::at(now, params).state == weather::STORM).then_some(current);
    }
    let oldest = (now.unix_timestamp() - window).div_euclid(period);
    (oldest..=current)
        .rev()
        .find(|slot| weather::state_for_slot(*slot, params.weather_seed) == weather::STORM)
}

/// Forest chunks (F and D) in row-major order.
fn forest_chunks() -> Vec<(i32, i32)> {
    let mut out = Vec::new();
    for cz in 0..CHUNKS {
        for cx in 0..CHUNKS {
            if matches!(mainland::biome(cx, cz), 'F' | 'D') {
                out.push((cx, cz));
            }
        }
    }
    out
}

/// Trees felled by storm `slot`: one per chosen forest chunk, 12 chunks.
pub fn fallen_trees(slot: i64) -> Vec<NodeSpec> {
    let mut chunks = forest_chunks();
    if chunks.is_empty() {
        return Vec::new();
    }
    let mut state = mix(slot as u64 ^ 0x5701_4E55);
    for i in (1..chunks.len()).rev() {
        state = mix(state);
        let j = usize::try_from(state % (i as u64 + 1)).unwrap_or(0);
        chunks.swap(i, j);
    }
    chunks
        .iter()
        .take(TREES_PER_STORM)
        .enumerate()
        .map(|(i, (cx, cz))| {
            state = mix(state);
            let jx = i32::try_from(state % 500).unwrap_or(0) + 70;
            state = mix(state);
            let jz = i32::try_from(state % 500).unwrap_or(0) + 70;
            NodeSpec {
                id: format!("{STORM_CX}_{slot}_{i}"),
                zone: "mainland".to_owned(),
                kind: KIND.to_owned(),
                x_dm: -5_120 + cx * CHUNK_DM + jx,
                z_dm: -5_120 + cz * CHUNK_DM + jz,
                hits_left: kind_hits(KIND),
                alts: Vec::new(),
            }
        })
        .collect()
}

/// The node behind `98_<slot>_<i>` while that storm's trees still lie around.
pub fn node_spec(id: &str, active: Option<i64>) -> Option<NodeSpec> {
    let mut parts = id.split('_');
    let cx: i32 = parts.next()?.parse().ok()?;
    let slot: i64 = parts.next()?.parse().ok()?;
    let i: usize = parts.next()?.parse().ok()?;
    if cx != STORM_CX || parts.next().is_some() || active != Some(slot) {
        return None;
    }
    fallen_trees(slot).into_iter().nth(i)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;

    fn params() -> Params {
        Params::default()
    }

    #[test]
    fn felled_trees_are_stable_and_in_forests() {
        let a = fallen_trees(1234);
        assert_eq!(a, fallen_trees(1234));
        assert_eq!(a.len(), TREES_PER_STORM);
        assert_ne!(a, fallen_trees(1235));
        for n in &a {
            let (cx, cz) = crate::exploration::chunk_of(n.x_dm, n.z_dm);
            assert!(matches!(mainland::biome(cx, cz), 'F' | 'D'), "{n:?}");
            assert!(n.id.starts_with("98_1234_"));
        }
        assert_eq!(node_spec("98_1234_3", Some(1234)).unwrap(), a[3]);
        assert!(
            node_spec("98_1234_3", Some(1235)).is_none(),
            "trees of another storm are gone"
        );
        assert!(node_spec("98_1234_3", None).is_none());
        assert!(node_spec("98_1234_99", Some(1234)).is_none());
        assert!(node_spec("4_3_0", Some(1234)).is_none());
    }

    #[test]
    fn active_slot_follows_the_schedule_and_the_window() {
        let p = params();
        let period = i64::from(p.weather_period_min) * 60;
        // Find a storm slot from the schedule and stand right after it.
        let storm = (0..10_000)
            .find(|s| weather::state_for_slot(*s, p.weather_seed) == weather::STORM)
            .unwrap();
        let during = OffsetDateTime::from_unix_timestamp(storm * period + 60).unwrap();
        assert_eq!(active_slot(during, p), Some(storm));
        let after = OffsetDateTime::from_unix_timestamp(storm * period + period + 60).unwrap();
        let next_is_storm = weather::state_for_slot(storm + 1, p.weather_seed) == weather::STORM;
        assert_eq!(
            active_slot(after, p),
            Some(if next_is_storm { storm + 1 } else { storm }),
            "still within the fallen window"
        );
        let mut later = p;
        later.storm_fallen_secs = 0;
        if !next_is_storm {
            assert_eq!(active_slot(after, later), None, "window closed");
        }
        let mut forced = p;
        forced.weather_force = Some(4);
        assert!(active_slot(during, forced).is_some());
        forced.weather_force = Some(0);
        assert_eq!(active_slot(during, forced), None);
    }
}
