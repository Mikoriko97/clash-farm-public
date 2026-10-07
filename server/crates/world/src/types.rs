use serde::Serialize;
use sqlx::postgres::PgRow;
use sqlx::Row;
use time::OffsetDateTime;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
pub struct PlotView {
    pub id: Uuid,
    pub idx: i16,
    /// Cell on the island grid (3 m pitch, centre = col*3, row*3 m; 0006).
    pub col: i16,
    pub row: i16,
    /// Group of side-adjacent plots (0-based per snapshot); acted on as one bed.
    pub bed: i16,
    /// `empty` | `growing` | `ready` (ready is derived from `ready_at <= now()` in SQL).
    pub state: String,
    pub crop_id: Option<String>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub planted_at: Option<OffsetDateTime>,
    #[serde(with = "time::serde::rfc3339::option")]
    pub ready_at: Option<OffsetDateTime>,
    pub fertilizer_id: Option<String>,
}

impl PlotView {
    /// Expects columns: id, idx, col, row, view_state, crop_id, planted_at, ready_at, fertilizer_id.
    /// `bed` starts at 0; `placement::assign_beds` fills it for a whole island.
    pub fn from_row(row: &PgRow) -> Result<Self, sqlx::Error> {
        Ok(Self {
            id: row.try_get("id")?,
            idx: row.try_get("idx")?,
            col: row.try_get("col")?,
            row: row.try_get("row")?,
            bed: 0,
            state: row.try_get("view_state")?,
            crop_id: row.try_get("crop_id")?,
            planted_at: row.try_get("planted_at")?,
            ready_at: row.try_get("ready_at")?,
            fertilizer_id: row.try_get("fertilizer_id")?,
        })
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct IslandView {
    pub id: Uuid,
    pub level: i32,
    pub xp: i64,
    pub expand_n: i16,
    pub expand_e: i16,
    pub expand_s: i16,
    pub expand_w: i16,
}

/// «Rested» (0026): after a sleep, every XP gain from actions is raised by `bonus_pct` percent until `until` (server clock).
#[derive(Debug, Clone, Serialize)]
pub struct RestedView {
    #[serde(with = "time::serde::rfc3339")]
    pub until: OffsetDateTime,
    pub bonus_pct: i16,
}

#[derive(Debug, Clone, Serialize)]
pub struct DecorView {
    pub id: Uuid,
    pub item_id: String,
    /// Decimetres from the island centre (0009); yaw in 15° steps.
    pub x_dm: i16,
    pub z_dm: i16,
    pub yaw_deg: i16,
}

#[derive(Debug, Clone, Serialize)]
pub struct IslandSnapshot {
    pub island: IslandView,
    /// Present only while the status is running; absent for a player who has not slept (or whose status ran out).
    pub rested: Option<RestedView>,
    pub plots: Vec<PlotView>,
    pub decor: Vec<DecorView>,
    /// Trees and rocks (0007): idx matches the island scene.
    pub resources: Vec<crate::resources::ResourceView>,
    /// Spots where the player cleared wild bushes (0023): `[x_dm, z_dm]`; the client keeps wild flora away.
    pub cleared: Vec<[i16; 2]>,
    /// Shared weather for `now` (schedule from the database clock).
    pub weather: crate::weather::WeatherView,
    /// Database clock; the client renders timers relative to this, never to its own clock.
    #[serde(with = "time::serde::rfc3339")]
    pub now: OffsetDateTime,
    /// Credits the next plot costs for this island (0019: each extra plot costs more).
    pub plot_price: i64,
    /// Plots this island may hold at its level (placed + in the bag); the shop refuses more.
    pub plot_limit: i64,
}

#[derive(Debug, Clone, Serialize)]
pub struct HarvestResult {
    pub plot: PlotView,
    pub xp: i64,
    pub level: i32,
    /// Produce added to the inventory (0008); credits come from selling it.
    pub item_id: String,
    pub qty: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct ItemQty {
    pub item_id: String,
    pub qty: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct InventoryEntry {
    pub item_id: String,
    pub qty: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct ItemView {
    pub id: String,
    pub kind: String,
    pub price_usd_cents: Option<i32>,
    pub price_credits: Option<i64>,
    pub exclusive: bool,
    pub meta: serde_json::Value,
}
