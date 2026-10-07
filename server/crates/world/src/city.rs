//! The city (docs/MAINLAND.md §7, M4 step 1): what stands on the square. No
//! partners yet — the market, the workshop and the town hall run on the game's
//! own economy, the showcases wait for a project. The list is fixed here until
//! the `partners` migration; the Godot scene lays the same ids out on the square.

use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct BuildingView {
    pub id: &'static str,
    /// `market` | `workshop` | `hall` | `showcase` | `monument`.
    pub kind: &'static str,
    pub name: &'static str,
    pub tagline: &'static str,
    /// A partner project behind the door; `None` = a showcase waiting for one.
    pub partner: Option<&'static str>,
}

pub const BUILDINGS: [BuildingView; 8] = [
    BuildingView {
        id: "hall",
        kind: "hall",
        name: "Town hall",
        tagline: "explorer's journal and expeditions of the day",
        partner: None,
    },
    BuildingView {
        id: "market",
        kind: "market",
        name: "Market",
        tagline: "sell produce and dishes for credits",
        partner: None,
    },
    BuildingView {
        id: "workshop",
        kind: "workshop",
        name: "Workshop",
        tagline: "craft an axe or a pickaxe, repair worn tools",
        partner: None,
    },
    BuildingView {
        id: "showcase_staking",
        kind: "showcase",
        name: "Sanctum",
        tagline: "staking temple: SOL → clashSOL, integration ahead",
        partner: None,
    },
    BuildingView {
        id: "showcase_exchange",
        kind: "showcase",
        name: "Swap house",
        tagline: "a place for a DEX",
        partner: None,
    },
    BuildingView {
        id: "showcase_pools",
        kind: "showcase",
        name: "Pool house",
        tagline: "a place for liquidity pools",
        partner: None,
    },
    BuildingView {
        id: "showcase_gallery",
        kind: "showcase",
        name: "Gallery",
        tagline: "a place for an NFT collection",
        partner: None,
    },
    BuildingView {
        id: "monument",
        kind: "monument",
        name: "Founders' monument",
        tagline: "the island's first players",
        partner: None,
    },
];

pub fn building(id: &str) -> Option<&'static BuildingView> {
    BUILDINGS.iter().find(|b| b.id == id)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ids_are_unique_and_kinds_known() {
        let mut ids: Vec<&str> = BUILDINGS.iter().map(|b| b.id).collect();
        ids.sort_unstable();
        ids.dedup();
        assert_eq!(ids.len(), BUILDINGS.len());
        assert!(BUILDINGS
            .iter()
            .all(|b| ["market", "workshop", "hall", "showcase", "monument"].contains(&b.kind)));
        assert_eq!(building("market").map(|b| b.name), Some("Market"));
        assert!(building("bank").is_none());
    }
}
