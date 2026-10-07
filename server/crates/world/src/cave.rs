//! Cave #1 (docs/MAINLAND.md §3, M3 step 3): a separate zone entered at the
//! mine gate. Its nodes are a fixed list in cave-local decimetres (the Godot
//! scene is laid out to match); live state shares `cf.mainland_nodes` with
//! ids `99_<cave>_<i>`.

use crate::mainland::{kind_hits, NodeSpec};

/// Pseudo chunk column that marks cave node ids.
pub const CAVE_CX: i32 = 99;
pub const ZONE: &str = "cave";
/// Where the character enters (and leaves) cave 1, in cave-local decimetres.
pub const ENTRY_DM: (i32, i32) = (0, 80);

/// Cave 1: (kind, x_dm, z_dm). Entrance chamber, a copper tunnel west, crystals east.
const CAVE_1: [(&str, i32, i32); 14] = [
    ("rock", -60, 10),
    ("rock", 70, -20),
    ("rock", -20, -90),
    ("ore_copper", -180, -120),
    ("ore_copper", -230, -160),
    ("ore_copper", -170, -210),
    ("ore_copper", -250, -90),
    ("ore_iron", -210, -250),
    ("ore_iron", -140, -170),
    ("ore_iron", 250, -240),
    ("crystal", 200, -160),
    ("crystal", 260, -190),
    ("crystal", 220, -260),
    ("crystal", 300, -140),
];

pub fn exists(cave: i32) -> bool {
    cave == 1
}

pub fn nodes(cave: i32) -> Vec<NodeSpec> {
    if !exists(cave) {
        return Vec::new();
    }
    CAVE_1
        .iter()
        .enumerate()
        .map(|(i, (kind, x, z))| NodeSpec {
            id: format!("{CAVE_CX}_{cave}_{i}"),
            zone: ZONE.to_owned(),
            kind: (*kind).to_owned(),
            x_dm: *x,
            z_dm: *z,
            hits_left: kind_hits(kind),
            alts: Vec::new(),
        })
        .collect()
}

/// Node `i` of `cave`, or `None`.
pub fn node_spec(cave: i32, i: usize) -> Option<NodeSpec> {
    nodes(cave).into_iter().nth(i)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;

    #[test]
    fn cave_one_has_crystals_and_stable_ids() {
        let n = nodes(1);
        assert_eq!(n.len(), 14);
        assert_eq!(n[0].id, "99_1_0");
        assert!(n.iter().all(|s| s.zone == "cave"));
        assert_eq!(n.iter().filter(|s| s.kind == "crystal").count(), 4);
        assert_eq!(node_spec(1, 10).unwrap().kind, "crystal");
        assert!(node_spec(1, 14).is_none());
        assert!(node_spec(2, 0).is_none());
    }
}
