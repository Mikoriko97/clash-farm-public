//! Economy parameters from `settings` (keys `economy.*`), cached 30 s
//! (rules/server-ledger-world.md). Defaults mirror docs/ECONOMY.md §3.

use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};

use sqlx::{PgPool, Row};

use crate::error::WorldError;

const CACHE_TTL: Duration = Duration::from_secs(30);

#[derive(Debug, Clone, Copy)]
pub struct Params {
    pub plots_start: i16,
    /// Plot count allowed at level 1 and from level 5 on (linear in between).
    /// One more plot every this many levels, up to `plots_max` (0019).
    pub plots_per_levels: i16,
    pub plots_max: i16,
    /// Each plot beyond the starters costs this much more than the last (0019).
    pub plot_price_step: i32,
    /// Largest group of side-adjacent plots the player may build (owner: 9 = 3×3).
    pub bed_max_cells: i16,
    /// Gathering (0007): hits per day, hits per node, regrowth, XP per hit, starting nodes.
    pub gather_max_per_day: i32,
    pub tree_hits: i32,
    pub rock_hits: i32,
    pub tree_respawn_secs: i32,
    pub rock_respawn_secs: i32,
    pub gather_xp: i32,
    pub starter_trees: i16,
    pub starter_rocks: i16,
    /// 0017: seconds a sapling needs to become gatherable; extra nodes per bought strip.
    pub tree_grow_secs: i32,
    pub strip_trees: i16,
    pub strip_rocks: i16,
    /// Mainland nodes (0012): hits per day (common / rare), regrowth, XP, reach from the presence position (dm).
    pub mainland_hits_per_day: i32,
    pub mainland_rare_hits_per_day: i32,
    pub mainland_respawn_secs: i32,
    pub mainland_rare_respawn_secs: i32,
    pub mainland_gather_xp: i32,
    pub mainland_reach_dm: i32,
    /// Tools (0013): hits before an axe or pickaxe breaks.
    pub tool_durability: i32,
    /// Storm-felled trees (0016) stay this long after the storm slot began.
    pub storm_fallen_secs: i32,
    /// Crafting (0008): crafts and dishes eaten per day.
    pub craft_max_per_day: i32,
    pub eat_max_per_day: i32,
    /// The house: sleeps per UTC day and XP per sleep (owner backlog, ECONOMY §3).
    pub sleep_max_per_day: i32,
    pub sleep_xp: i32,
    /// «Rested» after a sleep (0026): minutes it lasts and the percent added to every XP gain meanwhile (0 = off).
    pub rested_minutes: i32,
    pub rested_bonus_pct: i32,
    /// Shared weather: slot length, schedule seed, pinned state index into `weather::STATES`
    /// (`weather.force` = "storm" etc.), wood per lightning strike and strikes per day.
    pub weather_period_min: i32,
    pub weather_seed: i64,
    pub weather_force: Option<u8>,
    pub lightning_wood: i32,
    pub lightning_max_per_day: i32,
    pub harvest_max_per_day: i32,
    pub plant_max_per_day: i32,
    /// Ceiling of the daily credits cap; the level formula below never exceeds it.
    pub credits_max_per_day: i32,
    /// Daily credits cap = base + per_level × island level (0019).
    pub credits_cap_base: i32,
    pub credits_cap_per_level: i32,
    pub plant_xp: i32,
    pub premium_fert_per_day: i32,
    /// Wheat seeds a day for an island with no seeds, nothing growing and no credits (0019).
    pub stipend_wheat: i32,
    pub starter_wheat_seeds: i32,
    pub starter_carrot_seeds: i32,
    /// Tutorial crop (0027): radish seeds a new island starts with (45 s growth, first harvest in a minute).
    pub starter_radish_seeds: i32,
    /// Credits booked from `system:mint` when the island is created (onboarding).
    pub starter_credits: i32,
    /// Reputation needed to plant/use `exclusive` items (ECONOMY §3: 20).
    pub exclusive_min_reputation: i32,
    pub decor_slots: i16,
    pub shop_max_qty: i32,
    /// ADR-017: share of a CLASH payment that is burned; the rest goes to the treasury.
    pub burn_share_bp: i32,
    pub order_min_usd_cents: i32,
    pub quote_ttl_sec: i32,
    pub pay_confirmed_max_usd_cents: i32,
}

impl Default for Params {
    fn default() -> Self {
        Self {
            plots_start: 6,
            plots_per_levels: 2,
            plots_max: 20,
            plot_price_step: 100,
            bed_max_cells: 9,
            gather_max_per_day: 60,
            tree_hits: 5,
            rock_hits: 8,
            tree_respawn_secs: 21_600,
            rock_respawn_secs: 14_400,
            gather_xp: 1,
            starter_trees: 3,
            starter_rocks: 5,
            tree_grow_secs: 3_600,
            strip_trees: 1,
            strip_rocks: 1,
            mainland_hits_per_day: 60,
            mainland_rare_hits_per_day: 20,
            mainland_respawn_secs: 1_800,
            mainland_rare_respawn_secs: 3_600,
            mainland_gather_xp: 2,
            mainland_reach_dm: 60,
            tool_durability: 100,
            storm_fallen_secs: 1_800,
            craft_max_per_day: 100,
            eat_max_per_day: 20,
            sleep_max_per_day: 1,
            sleep_xp: 10,
            rested_minutes: 180,
            rested_bonus_pct: 10,
            weather_period_min: 12,
            weather_seed: 12_345,
            weather_force: None,
            lightning_wood: 3,
            lightning_max_per_day: 3,
            harvest_max_per_day: 40,
            plant_max_per_day: 40,
            credits_max_per_day: 10_000,
            credits_cap_base: 500,
            credits_cap_per_level: 150,
            plant_xp: 1,
            premium_fert_per_day: 12,
            stipend_wheat: 3,
            starter_wheat_seeds: 6,
            starter_carrot_seeds: 2,
            // 0 until 0027 adds the radish (and sets 4): a binary started before the migration must not grant
            // an item the catalog lacks (FK) and break every new island.
            starter_radish_seeds: 0,
            starter_credits: 30,
            exclusive_min_reputation: 20,
            decor_slots: 16,
            shop_max_qty: 100,
            burn_share_bp: 7000,
            order_min_usd_cents: 50,
            quote_ttl_sec: 90,
            pay_confirmed_max_usd_cents: 2000,
        }
    }
}

impl Params {
    /// Credits an island may mint today: grows with the level, never above the ceiling.
    pub fn credits_cap(self, level: i32) -> i32 {
        let by_level = self
            .credits_cap_base
            .saturating_add(self.credits_cap_per_level.saturating_mul(level.max(0)));
        by_level.min(self.credits_max_per_day).max(0)
    }
}

#[derive(Debug, Clone, Default)]
pub struct ParamsCache {
    inner: Arc<RwLock<Option<(Instant, Params)>>>,
}

impl ParamsCache {
    pub async fn get(&self, pool: &PgPool) -> Result<Params, WorldError> {
        if let Some((at, params)) = *self.inner.read().unwrap_or_else(|e| e.into_inner()) {
            if at.elapsed() < CACHE_TTL {
                return Ok(params);
            }
        }
        let params = load(pool).await?;
        *self.inner.write().unwrap_or_else(|e| e.into_inner()) = Some((Instant::now(), params));
        Ok(params)
    }

    /// Drops the cache (tests, admin updates).
    pub fn invalidate(&self) {
        *self.inner.write().unwrap_or_else(|e| e.into_inner()) = None;
    }
}

async fn load(pool: &PgPool) -> Result<Params, WorldError> {
    let rows =
        sqlx::query("SELECT key, value FROM cf.settings WHERE key LIKE 'economy.%' OR key LIKE 'weather.%'")
            .fetch_all(pool)
            .await?;
    let mut params = Params::default();
    for row in rows {
        let key: String = row.try_get("key")?;
        let value: serde_json::Value = row.try_get("value")?;
        if key == "weather.force" {
            // A state name pins the weather; anything else (null, "auto") releases it.
            params.weather_force = value
                .as_str()
                .and_then(|s| crate::weather::STATES.iter().position(|x| *x == s))
                .and_then(|i| u8::try_from(i).ok());
            continue;
        }
        let Some(number) = value.as_i64() else { continue };
        match key.as_str() {
            "economy.plots_start" => params.plots_start = clamp_i16(number),
            "economy.plots_per_levels" => params.plots_per_levels = clamp_i16(number).max(1),
            "economy.plots_max" => params.plots_max = clamp_i16(number),
            "economy.plot_price_step" => params.plot_price_step = clamp_i32(number),
            "economy.bed_max_cells" => params.bed_max_cells = clamp_i16(number),
            "economy.gather_max_per_day" => params.gather_max_per_day = clamp_i32(number),
            "economy.tree_hits" => params.tree_hits = clamp_i32(number),
            "economy.rock_hits" => params.rock_hits = clamp_i32(number),
            "economy.tree_respawn_secs" => params.tree_respawn_secs = clamp_i32(number),
            "economy.rock_respawn_secs" => params.rock_respawn_secs = clamp_i32(number),
            "economy.gather_xp" => params.gather_xp = clamp_i32(number),
            "economy.starter_trees" => params.starter_trees = clamp_i16(number),
            "economy.starter_rocks" => params.starter_rocks = clamp_i16(number),
            "economy.tree_grow_secs" => params.tree_grow_secs = clamp_i32(number),
            "economy.strip_trees" => params.strip_trees = clamp_i16(number),
            "economy.strip_rocks" => params.strip_rocks = clamp_i16(number),
            "economy.craft_max_per_day" => params.craft_max_per_day = clamp_i32(number),
            "economy.mainland_hits_per_day" => params.mainland_hits_per_day = clamp_i32(number),
            "economy.mainland_rare_hits_per_day" => params.mainland_rare_hits_per_day = clamp_i32(number),
            "economy.mainland_respawn_secs" => params.mainland_respawn_secs = clamp_i32(number),
            "economy.mainland_rare_respawn_secs" => params.mainland_rare_respawn_secs = clamp_i32(number),
            "economy.mainland_gather_xp" => params.mainland_gather_xp = clamp_i32(number),
            "economy.mainland_reach_dm" => params.mainland_reach_dm = clamp_i32(number),
            "economy.tool_durability" => params.tool_durability = clamp_i32(number),
            "economy.storm_fallen_secs" => params.storm_fallen_secs = clamp_i32(number),
            "economy.eat_max_per_day" => params.eat_max_per_day = clamp_i32(number),
            "economy.sleep_max_per_day" => params.sleep_max_per_day = clamp_i32(number),
            "economy.sleep_xp" => params.sleep_xp = clamp_i32(number),
            "economy.rested_minutes" => params.rested_minutes = clamp_i32(number),
            "economy.rested_bonus_pct" => params.rested_bonus_pct = clamp_i32(number),
            "weather.period_min" => params.weather_period_min = clamp_i32(number).max(1),
            "weather.seed" => params.weather_seed = number,
            "economy.lightning_wood" => params.lightning_wood = clamp_i32(number),
            "economy.lightning_max_per_day" => params.lightning_max_per_day = clamp_i32(number),
            "economy.harvest_max_per_day" => params.harvest_max_per_day = clamp_i32(number),
            "economy.plant_max_per_day" => params.plant_max_per_day = clamp_i32(number),
            "economy.credits_max_per_day" => params.credits_max_per_day = clamp_i32(number),
            "economy.credits_cap_base" => params.credits_cap_base = clamp_i32(number),
            "economy.credits_cap_per_level" => params.credits_cap_per_level = clamp_i32(number),
            "economy.plant_xp" => params.plant_xp = clamp_i32(number),
            "economy.premium_fert_per_day" => params.premium_fert_per_day = clamp_i32(number),
            "economy.stipend_wheat" => params.stipend_wheat = clamp_i32(number),
            "economy.starter_wheat_seeds" => params.starter_wheat_seeds = clamp_i32(number),
            "economy.starter_carrot_seeds" => params.starter_carrot_seeds = clamp_i32(number),
            "economy.starter_radish_seeds" => params.starter_radish_seeds = clamp_i32(number),
            "economy.starter_credits" => params.starter_credits = clamp_i32(number),
            "economy.exclusive_min_reputation" => params.exclusive_min_reputation = clamp_i32(number),
            "economy.decor_slots" => params.decor_slots = clamp_i16(number),
            "economy.shop_max_qty" => params.shop_max_qty = clamp_i32(number),
            // `burns.amount > 0` is a DB CHECK: a zero share would park every order in needs_review.
            "economy.burn_share_bp" => params.burn_share_bp = clamp_i32(number).clamp(1, 10_000),
            "economy.order_min_usd_cents" => params.order_min_usd_cents = clamp_i32(number),
            "economy.quote_ttl_sec" => params.quote_ttl_sec = clamp_i32(number),
            "economy.pay_confirmed_max_usd_cents" => params.pay_confirmed_max_usd_cents = clamp_i32(number),
            _ => {}
        }
    }
    Ok(params)
}

fn clamp_i32(value: i64) -> i32 {
    i32::try_from(value.clamp(0, i64::from(i32::MAX))).unwrap_or(i32::MAX)
}

fn clamp_i16(value: i64) -> i16 {
    i16::try_from(value.clamp(0, i64::from(i16::MAX))).unwrap_or(i16::MAX)
}
