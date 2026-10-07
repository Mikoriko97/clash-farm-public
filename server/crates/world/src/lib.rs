//! Island world: plots, crops, inventory, daily caps (docs/DATA_MODEL.md §3–4,
//! docs/ECONOMY.md). Every action locks the owner's row with `FOR UPDATE`, takes
//! time from the database clock and reads economy parameters from `settings`.
//! Credits/XP accounting through the ledger lands in week 3 (0003).
#![forbid(unsafe_code)]

/// Plot columns every plot query selects (view_state derives `ready` from the DB clock).
/// A macro, not a const, so queries stay literal SQL for sqlx's injection audit.
macro_rules! plot_columns {
    () => {
        "p.id, p.idx, p.col, p.row,
         CASE WHEN p.state = 'growing' AND p.ready_at <= now() THEN 'ready' ELSE p.state END AS view_state,
         p.crop_id, p.planted_at, p.ready_at, p.fertilizer_id"
    };
}

pub mod beds;
pub mod caps;
pub mod catalog;
pub mod cave;
pub mod character;
pub mod city;
pub mod clearings;
pub mod craft;
pub mod decor;
pub mod error;
pub mod expand;
pub mod expeditions;
pub mod exploration;
pub mod house;
pub mod inventory;
pub mod island;
pub mod island_spots;
pub mod levels;
pub mod mainland;
pub mod mainland_gather;
pub mod mainland_kinds;
pub mod market;
pub mod moderation;
pub mod nickname;
pub mod params;
pub mod placement;
pub mod plots;
pub mod regrow;
pub mod rename;
pub mod resources;
pub mod restyle;
pub mod shop;
pub mod stipend;
pub mod storm;
pub mod tools;
pub mod types;
pub mod weather;
pub mod xp;

pub use error::WorldError;
pub use params::{Params, ParamsCache};
pub use types::{
    DecorView, HarvestResult, InventoryEntry, IslandSnapshot, IslandView, ItemQty, ItemView, PlotView,
};
