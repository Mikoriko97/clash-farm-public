//! Where a node may stand on an island (migration 0017): the island top is the base
//! square plus bought strips with the ground's rounded corners and a shore rim; the
//! house, plots, decor and other nodes are circles to keep clear of. Pure geometry
//! in integer decimetres; `regrow` loads the obstacles from the database.

use rand::{Rng, RngExt};

use crate::placement::{EXPAND_STRIP_M, ISLAND_HALF_M};

/// Island.tscn house: centre and clear radius (dm).
pub const HOUSE_DM: (i64, i64, i64) = (-130, -70, 60);
/// Rim kept clear along the shore (dm) and the ground's corner rounding (dm).
const EDGE_MARGIN_DM: i64 = 25;
const CORNER_DM: i64 = 70;
const TRIES: usize = 240;

/// An obstacle on the island: centre (dm) and clear radius (dm).
#[derive(Debug, Clone, Copy)]
pub struct Circle {
    pub x: i64,
    pub z: i64,
    pub r: i64,
}

/// What the island currently looks like for placement purposes.
#[derive(Debug, Clone)]
pub struct Occupancy {
    /// n, e, s, w strips.
    pub expand: [i16; 4],
    pub blocked: Vec<Circle>,
}

impl Occupancy {
    /// Bounds of the island top (dm): (min_x, max_x, min_z, max_z).
    pub fn bounds(&self) -> (i64, i64, i64, i64) {
        let [n, e, s, w] = self.expand.map(|x| i64::from(x) * i64::from(EXPAND_STRIP_M) * 10);
        let half = i64::from(ISLAND_HALF_M) * 10;
        (-(half + w), half + e, -(half + n), half + s)
    }

    /// Inside the rounded island top with `r` (dm) to spare from the shore.
    pub fn inside(&self, x: i64, z: i64, r: i64) -> bool {
        let (min_x, max_x, min_z, max_z) = self.bounds();
        let margin = r + EDGE_MARGIN_DM;
        if x - margin < min_x || x + margin > max_x || z - margin < min_z || z + margin > max_z {
            return false;
        }
        // Rounded corners: within CORNER_DM of a corner the shore curves inwards.
        let cx = if x - min_x < CORNER_DM {
            Some(min_x + CORNER_DM)
        } else if max_x - x < CORNER_DM {
            Some(max_x - CORNER_DM)
        } else {
            None
        };
        let cz = if z - min_z < CORNER_DM {
            Some(min_z + CORNER_DM)
        } else if max_z - z < CORNER_DM {
            Some(max_z - CORNER_DM)
        } else {
            None
        };
        match (cx, cz) {
            (Some(cx), Some(cz)) => {
                let dx = x - cx;
                let dz = z - cz;
                let reach = CORNER_DM - margin;
                reach > 0 && dx * dx + dz * dz <= reach * reach
            }
            _ => true,
        }
    }

    /// Free for a node of footprint `r` (dm) centred at (x, z).
    pub fn is_free(&self, x: i64, z: i64, r: i64) -> bool {
        if !self.inside(x, z, r) {
            return false;
        }
        self.blocked.iter().all(|c| {
            let dx = x - c.x;
            let dz = z - c.z;
            let reach = r + c.r;
            dx * dx + dz * dz >= reach * reach
        })
    }

    /// A random free spot for footprint `r`, or None when the island is packed.
    pub fn find_spot(&self, r: i64, rng: &mut impl Rng) -> Option<(i16, i16)> {
        let (min_x, max_x, min_z, max_z) = self.bounds();
        for _ in 0..TRIES {
            let x = rng.random_range(min_x..=max_x);
            let z = rng.random_range(min_z..=max_z);
            if self.is_free(x, z, r) {
                return Some((x as i16, z as i16));
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> Occupancy {
        Occupancy {
            expand: [0; 4],
            blocked: vec![Circle {
                x: HOUSE_DM.0,
                z: HOUSE_DM.1,
                r: HOUSE_DM.2,
            }],
        }
    }

    #[test]
    fn shape_bounds_follow_strips() {
        let mut occ = base();
        assert!(occ.inside(0, 0, 18));
        assert!(!occ.inside(230, 0, 18), "past the east shore");
        occ.expand = [0, 2, 0, 0];
        assert!(occ.inside(300, 0, 18), "east strips bought");
        assert!(!occ.inside(0, 230, 18), "south not bought");
    }

    #[test]
    fn corners_are_rounded_and_house_is_kept_clear() {
        let occ = base();
        assert!(!occ.inside(-205, -205, 10), "corner of the square is water");
        assert!(occ.inside(-205, 0, 10), "middle of the west edge is land");
        assert!(!occ.is_free(-130, -70, 18), "house");
        assert!(occ.is_free(100, 100, 18));
    }

    #[test]
    fn find_spot_avoids_everything() {
        let mut occ = base();
        occ.blocked.push(Circle { x: 0, z: 0, r: 60 });
        let mut rng = rand::rng();
        for _ in 0..50 {
            let spot = occ.find_spot(18, &mut rng);
            assert!(spot.is_some(), "island is not packed");
            if let Some((x, z)) = spot {
                assert!(occ.is_free(i64::from(x), i64::from(z), 18));
            }
        }
    }
}
