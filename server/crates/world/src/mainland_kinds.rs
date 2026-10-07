//! Node kinds of the mainland (docs/MAINLAND.md §5–6): the item one hit gives,
//! hits per fresh node and the rare cap. How many of each a chunk holds is
//! decided by the flora bake (`client/tools/bake_flora.gd`, RECIPE). Split out
//! of `mainland` to keep that file within the size limit; re-exported there.

/// A node kind: the item one hit gives, hits per fresh node, whether it counts
/// against the rare cap (docs/MAINLAND.md §5–6).
#[derive(Debug, Clone, Copy)]
pub struct KindInfo {
    pub kind: &'static str,
    pub item: &'static str,
    pub hits: i32,
    pub rare: bool,
}

pub const KINDS: [KindInfo; 11] = [
    KindInfo {
        kind: "tree_pine",
        item: "wood_pine",
        hits: 5,
        rare: false,
    },
    KindInfo {
        kind: "tree_oak",
        item: "wood_oak",
        hits: 5,
        rare: false,
    },
    KindInfo {
        kind: "rock",
        item: "stone",
        hits: 6,
        rare: false,
    },
    KindInfo {
        kind: "ore_copper",
        item: "ore_copper",
        hits: 4,
        rare: false,
    },
    KindInfo {
        kind: "ore_iron",
        item: "ore_iron",
        hits: 4,
        rare: true,
    },
    KindInfo {
        kind: "herb",
        item: "herb",
        hits: 1,
        rare: false,
    },
    KindInfo {
        kind: "mushroom",
        item: "mushroom",
        hits: 1,
        rare: false,
    },
    KindInfo {
        kind: "mushroom_rare",
        item: "mushroom_rare",
        hits: 1,
        rare: true,
    },
    KindInfo {
        kind: "reed",
        item: "reed",
        hits: 2,
        rare: false,
    },
    // Cave (0016): rare, pickaxe.
    KindInfo {
        kind: "crystal",
        item: "crystal",
        hits: 3,
        rare: true,
    },
    // Felled by a storm (0016): free wood, no tool.
    KindInfo {
        kind: "tree_fallen",
        item: "wood_pine",
        hits: 3,
        rare: false,
    },
];

pub fn kind_info(kind: &str) -> Option<KindInfo> {
    KINDS.iter().copied().find(|k| k.kind == kind)
}

/// Hits a fresh node of `kind` takes (1 for an unknown kind).
pub fn kind_hits(kind: &str) -> i32 {
    kind_info(kind).map_or(1, |k| k.hits)
}
