//! Mainland world nodes (docs/MAINLAND.md §5, §10 — M2 step 1). The map is a
//! 16 × 16 grid of 64 m chunks; each chunk's biome comes from the baked zone
//! table (shared/mainland_zones.json, written by client/tools/make_mainland.gd)
//! and its resource nodes come baked from the same blue-noise pass that
//! places the client's flora (shared/mainland_nodes.json), so every server
//! instance and the client agree on where a tree stands. Node *state* (hits, respawn) arrives with the gather migration.

use std::collections::HashMap;
use std::sync::OnceLock;

use serde::{Deserialize, Serialize};

use crate::error::WorldError;
pub use crate::mainland_kinds::{kind_hits, kind_info, KindInfo, KINDS};

const ZONES_JSON: &str = include_str!("../../../../shared/mainland_zones.json");
const NODES_JSON: &str = include_str!("../../../../shared/mainland_nodes.json");

/// Chunk edge in metres and in decimetres.
pub const CHUNK_M: i32 = 64;
pub const CHUNK_DM: i32 = CHUNK_M * 10;
/// Chunks per side; chunk (0, 0) starts at `origin_m`.
pub const CHUNKS: i32 = 16;
/// Largest radius a client may ask for in one request (7 × 7 chunks).
pub const MAX_RADIUS: i32 = 3;
#[derive(Debug, Deserialize)]
struct ZoneTable {
    chunk_m: i32,
    chunks: i32,
    origin_m: i32,
    rows: Vec<String>,
}

/// One resource node as the client draws it.
#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
pub struct NodeSpec {
    /// `cx_cz_i`, stable for the life of the map.
    pub id: String,
    /// Presence zone the character must stand in to hit it (`mainland` | `cave`).
    pub zone: String,
    pub kind: String,
    /// World position in decimetres from the map centre.
    pub x_dm: i32,
    pub z_dm: i32,
    /// Full stock; the gather step will overlay live state.
    pub hits_left: i32,
    /// Reserved alternate spots from the bake (0024), never sent to clients: the node stands on
    /// `[home, alts…][respawns mod (1 + alts)]`, so it returns somewhere else after every depletion.
    #[serde(skip)]
    pub alts: Vec<(i32, i32)>,
}

impl NodeSpec {
    /// Where the node stands after `respawns` depletions (`0` = its home).
    pub fn at(&self, respawns: i32) -> (i32, i32) {
        let n = i32::try_from(self.alts.len()).unwrap_or(0) + 1;
        match usize::try_from(respawns.rem_euclid(n)).unwrap_or(0) {
            0 => (self.x_dm, self.z_dm),
            k => self.alts.get(k - 1).copied().unwrap_or((self.x_dm, self.z_dm)),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct ChunkView {
    pub cx: i32,
    pub cz: i32,
    /// Zone letter: C city, B beach, M meadow, F forest, D dark forest, R rock, S swamp, W water.
    pub biome: String,
    pub nodes: Vec<NodeSpec>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ChunksView {
    pub chunks: Vec<ChunkView>,
}

fn table() -> &'static ZoneTable {
    static TABLE: OnceLock<ZoneTable> = OnceLock::new();
    TABLE.get_or_init(|| {
        serde_json::from_str::<ZoneTable>(ZONES_JSON).unwrap_or(ZoneTable {
            chunk_m: CHUNK_M,
            chunks: CHUNKS,
            origin_m: -(CHUNK_M * CHUNKS / 2),
            rows: Vec::new(),
        })
    })
}

/// The whole zone table as the client draws its map (docs/MAINLAND.md §4).
#[derive(Debug, Clone, Serialize)]
pub struct MapView {
    pub chunk_m: i32,
    pub chunks: i32,
    pub origin_m: i32,
    pub rows: Vec<String>,
}

pub fn map_view() -> MapView {
    let t = table();
    MapView {
        chunk_m: t.chunk_m,
        chunks: t.chunks,
        origin_m: t.origin_m,
        rows: t.rows.clone(),
    }
}

/// Zone letter of a chunk; `W` (water) outside the map or for a malformed table.
pub fn biome(cx: i32, cz: i32) -> char {
    if !in_range(cx, cz) {
        return 'W';
    }
    let t = table();
    let Ok(row) = usize::try_from(cz) else {
        return 'W';
    };
    t.rows
        .get(row)
        .and_then(|r| usize::try_from(cx).ok().and_then(|c| r.chars().nth(c)))
        .unwrap_or('W')
}

/// Squared distance in decimetres (i64, no floats).
pub fn dist2_dm(ax: i32, az: i32, bx: i32, bz: i32) -> i64 {
    let dx = i64::from(ax) - i64::from(bx);
    let dz = i64::from(az) - i64::from(bz);
    dx * dx + dz * dz
}

pub fn in_range(cx: i32, cz: i32) -> bool {
    (0..CHUNKS).contains(&cx) && (0..CHUNKS).contains(&cz)
}

/// The node behind a generated id (`cx_cz_i`, or a cave `99_<cave>_<i>`);
/// storm ids (`98_…`) go through `storm::node_spec` because they expire.
pub fn node_spec(id: &str) -> Option<NodeSpec> {
    let mut parts = id.split('_');
    let cx: i32 = parts.next()?.parse().ok()?;
    let cz: i32 = parts.next()?.parse().ok()?;
    let i: usize = parts.next()?.parse().ok()?;
    if parts.next().is_some() {
        return None;
    }
    if cx == crate::cave::CAVE_CX {
        return crate::cave::node_spec(cz, i);
    }
    chunk_nodes(cx, cz).into_iter().nth(i)
}

#[derive(Debug, Deserialize)]
struct NodeRow {
    id: String,
    kind: String,
    x_dm: i32,
    z_dm: i32,
    #[serde(default)]
    alts: Vec<[i32; 2]>,
}

#[derive(Debug, Deserialize)]
struct NodeTable {
    nodes: Vec<NodeRow>,
}

/// Chunk of a world position in decimetres.
pub fn chunk_of_dm(x_dm: i32, z_dm: i32) -> (i32, i32) {
    let t = table();
    (
        (x_dm - t.origin_m * 10).div_euclid(CHUNK_DM),
        (z_dm - t.origin_m * 10).div_euclid(CHUNK_DM),
    )
}

/// Baked nodes by chunk (`shared/mainland_nodes.json`, written by
/// `client/tools/bake_flora.gd`): the blue-noise pass that places the
/// mainland's flora reserves these spots, so no tree grows through an ore vein
/// and nothing stands in water or on a cliff. Rows with an unknown kind, a
/// water chunk, or an id that does not match their chunk and order are dropped.
fn nodes() -> &'static HashMap<(i32, i32), Vec<NodeSpec>> {
    static TABLE: OnceLock<HashMap<(i32, i32), Vec<NodeSpec>>> = OnceLock::new();
    TABLE.get_or_init(|| {
        let mut out: HashMap<(i32, i32), Vec<NodeSpec>> = HashMap::new();
        let rows = serde_json::from_str::<NodeTable>(NODES_JSON)
            .map(|t| t.nodes)
            .unwrap_or_default();
        for row in rows {
            let (cx, cz) = chunk_of_dm(row.x_dm, row.z_dm);
            let list = out.entry((cx, cz)).or_default();
            let expected = format!("{cx}_{cz}_{}", list.len());
            if row.id != expected || kind_info(&row.kind).is_none() || biome(cx, cz) == 'W' {
                continue;
            }
            let hits_left = kind_hits(&row.kind);
            // Alternates must lie in the node's own chunk and be distinct; anything else is dropped.
            let mut alts: Vec<(i32, i32)> = Vec::new();
            for [ax, az] in row.alts.iter().copied().take(4) {
                let spot = (ax, az);
                if chunk_of_dm(ax, az) == (cx, cz) && spot != (row.x_dm, row.z_dm) && !alts.contains(&spot) {
                    alts.push(spot);
                }
            }
            list.push(NodeSpec {
                id: row.id,
                zone: "mainland".to_owned(),
                kind: row.kind,
                x_dm: row.x_dm,
                z_dm: row.z_dm,
                hits_left,
                alts,
            });
        }
        out
    })
}

/// Nodes of one chunk (empty outside the map, at sea and in the city).
pub fn chunk_nodes(cx: i32, cz: i32) -> Vec<NodeSpec> {
    nodes().get(&(cx, cz)).cloned().unwrap_or_default()
}

/// The square of chunks within `radius` of (cx, cz); `400` for bad arguments.
pub fn chunks(cx: i32, cz: i32, radius: i32) -> Result<ChunksView, WorldError> {
    if !in_range(cx, cz) || !(0..=MAX_RADIUS).contains(&radius) {
        return Err(WorldError::OutOfBounds);
    }
    let mut out = Vec::new();
    for dz in -radius..=radius {
        for dx in -radius..=radius {
            let (x, z) = (cx + dx, cz + dz);
            if !in_range(x, z) {
                continue;
            }
            out.push(ChunkView {
                cx: x,
                cz: z,
                biome: biome(x, z).to_string(),
                nodes: chunk_nodes(x, z),
            });
        }
    }
    Ok(ChunksView { chunks: out })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;

    #[test]
    fn zone_table_matches_the_baked_map() {
        let t = table();
        assert_eq!(t.chunk_m, CHUNK_M);
        assert_eq!(t.chunks, CHUNKS);
        assert_eq!(t.rows.len(), 16);
        assert!(t.rows.iter().all(|r| r.chars().count() == 16));
        assert_eq!(biome(0, 0), 'W');
        assert_eq!(biome(-1, 3), 'W');
        assert_eq!(biome(16, 3), 'W');
        assert_eq!(biome(7, 8), 'C');
        assert_eq!(biome(13, 3), 'R');
        assert_eq!(biome(2, 1), 'D');
    }

    #[test]
    fn baked_nodes_are_consistent_and_apart() {
        let all: Vec<NodeSpec> = (0..CHUNKS)
            .flat_map(|cz| (0..CHUNKS).flat_map(move |cx| chunk_nodes(cx, cz)))
            .collect();
        assert!(
            all.len() > 800,
            "the map holds a full set of nodes: {}",
            all.len()
        );
        assert_eq!(chunk_nodes(4, 3), chunk_nodes(4, 3));
        assert!(chunk_nodes(4, 3).len() >= 4, "forest chunk has its trees");
        assert!(chunk_nodes(4, 3).iter().any(|n| n.kind == "tree_pine"));
        assert!(chunk_nodes(7, 8).is_empty(), "no nodes in the city");
        assert!(chunk_nodes(0, 0).is_empty(), "no nodes at sea");
        assert!(chunk_nodes(13, 3).iter().any(|n| n.kind == "ore_iron"));
        for n in &all {
            let (cx, cz) = chunk_of_dm(n.x_dm, n.z_dm);
            assert!(n.id.starts_with(&format!("{cx}_{cz}_")), "{n:?}");
            assert_eq!(node_spec(&n.id).as_ref(), Some(n));
            assert!(kind_info(&n.kind).is_some(), "{n:?}");
        }
        for (i, a) in all.iter().enumerate() {
            for b in all.iter().skip(i + 1) {
                assert!(
                    dist2_dm(a.x_dm, a.z_dm, b.x_dm, b.z_dm) >= 20 * 20,
                    "too close: {a:?} {b:?}"
                );
            }
        }
    }

    #[test]
    fn a_node_cycles_through_its_home_and_reserved_alternates() {
        let all: Vec<NodeSpec> = (0..CHUNKS)
            .flat_map(|cz| (0..CHUNKS).flat_map(move |cx| chunk_nodes(cx, cz)))
            .collect();
        let with_alts = all.iter().filter(|n| !n.alts.is_empty()).count();
        assert!(
            with_alts * 10 > all.len() * 9,
            "the bake reserves spots for nearly every node: {with_alts}/{}",
            all.len()
        );
        for n in all.iter().filter(|n| !n.alts.is_empty()) {
            assert_eq!(n.at(0), (n.x_dm, n.z_dm), "a fresh node stands at home");
            let cycle = i32::try_from(n.alts.len()).unwrap() + 1;
            assert_eq!(n.at(cycle), n.at(0), "the cycle closes");
            assert_ne!(n.at(1), n.at(0), "the first return is somewhere else: {n:?}");
            assert_eq!(
                chunk_of_dm(n.at(1).0, n.at(1).1),
                chunk_of_dm(n.x_dm, n.z_dm),
                "same chunk"
            );
            assert_eq!(n.at(-1), n.at(cycle - 1), "negative counters wrap, never panic");
        }
        // No two nodes share a spot in any phase: alternates are reserved like homes.
        let mut spots = std::collections::HashSet::new();
        for n in &all {
            for k in 0..=i32::try_from(n.alts.len()).unwrap() {
                assert!(spots.insert(n.at(k)), "spot taken twice: {:?}", n.at(k));
            }
        }
    }

    #[test]
    fn node_ids_round_trip_and_garbage_is_refused() {
        let nodes = chunk_nodes(4, 3);
        let spec = node_spec(&nodes[3].id).unwrap();
        assert_eq!(spec, nodes[3]);
        assert!(node_spec("4_3_99").is_none());
        assert!(node_spec("4_3").is_none());
        assert!(node_spec("4_3_1_1").is_none());
        assert!(node_spec("x_3_1").is_none());
        assert!(node_spec("7_8_0").is_none(), "city has no nodes");
        assert!(kind_info("tree_pine").is_some_and(|k| k.item == "wood_pine" && !k.rare));
        assert!(kind_info("ore_iron").is_some_and(|k| k.rare));
        assert!(kind_info("dragon").is_none());
    }

    #[test]
    fn chunk_window_is_clipped_and_bounded() {
        let v = chunks(8, 14, 3).unwrap();
        assert_eq!(v.chunks.len(), 35, "7 × 5 inside the map");
        assert!(chunks(16, 0, 1).is_err());
        assert!(chunks(8, 8, 4).is_err());
        assert!(chunks(8, 8, -1).is_err());
        assert_eq!(chunks(0, 0, 0).unwrap().chunks.len(), 1);
    }
}
