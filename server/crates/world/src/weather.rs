//! Shared weather (owner backlog: server-driven weather events). The schedule is
//! a pure function of the database clock: time is cut into slots of
//! `weather.period_min` minutes and each slot rolls its state from a seeded
//! hash, so every server instance and every client agree without state.
//! `weather.force` (settings) pins a state for demos and tests.
//!
//! Lightning: during a storm slot each island may be struck once — a tree
//! splits and drops wood into the inventory. The client only asks
//! (`POST /weather/lightning`); the roll, the slot and the caps are server-side.

use serde::Serialize;
use sqlx::{PgPool, Row};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::caps;
use crate::error::WorldError;
use crate::inventory;
use crate::params::Params;
use crate::plots::record_event;

/// State names as the client knows them (Weather.state_from_name).
pub const STATES: [&str; 5] = ["clear", "cloudy", "fog", "rain", "storm"];
pub const STORM: &str = "storm";

#[derive(Debug, Clone, Serialize)]
pub struct WeatherView {
    pub state: String,
    pub slot: i64,
    #[serde(with = "time::serde::rfc3339")]
    pub since: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub until: OffsetDateTime,
}

#[derive(Debug, Clone, Serialize)]
pub struct LightningResult {
    /// False when this storm slot already struck the island (idempotent replay).
    pub struck: bool,
    pub slot: i64,
    pub resource_idx: Option<i16>,
    pub item_id: String,
    pub qty: i32,
}

/// splitmix64: cheap, stable across platforms, good enough for a weather roll.
fn mix(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    x = (x ^ (x >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    x = (x ^ (x >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    x ^ (x >> 31)
}

/// Same thresholds as the client's fallback schedule (client/scripts/weather.gd).
pub fn state_for_slot(slot: i64, seed: i64) -> &'static str {
    let roll = mix((slot as u64) ^ mix(seed as u64)) % 1000;
    match roll {
        0..=99 => "storm",
        100..=279 => "rain",
        280..=379 => "fog",
        380..=599 => "cloudy",
        _ => "clear",
    }
}

fn period_secs(params: Params) -> i64 {
    i64::from(params.weather_period_min.max(1)) * 60
}

/// Weather at `now` (the caller passes the database clock).
pub fn at(now: OffsetDateTime, params: Params) -> WeatherView {
    let period = period_secs(params);
    let slot = now.unix_timestamp().div_euclid(period);
    let state = match params.weather_force.and_then(|i| STATES.get(usize::from(i))) {
        Some(forced) => (*forced).to_owned(),
        None => state_for_slot(slot, params.weather_seed).to_owned(),
    };
    let since = OffsetDateTime::from_unix_timestamp(slot * period).unwrap_or(now);
    let until = OffsetDateTime::from_unix_timestamp((slot + 1) * period).unwrap_or(now);
    WeatherView {
        state,
        slot,
        since,
        until,
    }
}

/// The database clock, for anything timed on the server (rules: never the process clock).
pub async fn db_now(pool: &PgPool) -> Result<OffsetDateTime, WorldError> {
    Ok(sqlx::query_scalar("SELECT now()").fetch_one(pool).await?)
}

/// `GET /weather`.
pub async fn current(pool: &PgPool, params: Params) -> Result<WeatherView, WorldError> {
    let now: OffsetDateTime = sqlx::query_scalar("SELECT now()").fetch_one(pool).await?;
    Ok(at(now, params))
}

/// `POST /weather/lightning`: one strike per island per storm slot, `lightning_max_per_day` a day.
pub async fn lightning(pool: &PgPool, user_id: Uuid, params: Params) -> Result<LightningResult, WorldError> {
    let mut tx = pool.begin().await?;
    let now: OffsetDateTime = sqlx::query_scalar("SELECT now()").fetch_one(&mut *tx).await?;
    let weather = at(now, params);
    if weather.state != STORM {
        return Err(WorldError::NoStorm);
    }
    let island_id: Uuid = sqlx::query_scalar("SELECT id FROM cf.islands WHERE user_id = $1 FOR UPDATE")
        .bind(user_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(WorldError::NotFound)?;
    let trees: Vec<i16> =
        sqlx::query("SELECT idx FROM cf.resource_nodes WHERE island_id = $1 AND kind = 'tree' ORDER BY idx")
            .bind(island_id)
            .fetch_all(&mut *tx)
            .await?
            .iter()
            .map(|r| r.try_get::<i16, _>("idx"))
            .collect::<Result<_, _>>()?;
    let none = LightningResult {
        struck: false,
        slot: weather.slot,
        resource_idx: None,
        item_id: "wood".to_owned(),
        qty: 0,
    };
    if trees.is_empty() {
        return Ok(none);
    }
    // One strike per storm slot: the slot id is part of the cap key.
    let slot_key = format!("lightning:{}", weather.slot);
    if caps::consume(&mut tx, user_id, &slot_key, 1).await.is_err() {
        return Ok(none);
    }
    caps::consume(&mut tx, user_id, "lightning", params.lightning_max_per_day).await?;
    let pick = (mix((weather.slot as u64) ^ (island_id.as_u128() as u64)) % trees.len() as u64) as usize;
    let idx = trees[pick];
    let qty = params.lightning_wood.max(0);
    if qty > 0 {
        inventory::add(&mut tx, user_id, "wood", qty).await?;
    }
    record_event(
        &mut tx,
        user_id,
        "lightning",
        serde_json::json!({ "slot": weather.slot, "idx": idx, "wood": qty }),
    )
    .await?;
    tx.commit().await?;
    Ok(LightningResult {
        struck: true,
        slot: weather.slot,
        resource_idx: Some(idx),
        item_id: "wood".to_owned(),
        qty,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::field_reassign_with_default)]
mod tests {
    use super::*;

    #[test]
    fn schedule_is_deterministic_and_mixed() {
        let mut seen = std::collections::HashSet::new();
        for slot in 0..200 {
            assert_eq!(state_for_slot(slot, 7), state_for_slot(slot, 7));
            seen.insert(state_for_slot(slot, 7));
        }
        assert!(seen.len() >= 4, "200 slots should show most states: {seen:?}");
        assert_ne!(
            (0..50).map(|s| state_for_slot(s, 1)).collect::<Vec<_>>(),
            (0..50).map(|s| state_for_slot(s, 2)).collect::<Vec<_>>(),
            "the seed changes the schedule"
        );
    }

    #[test]
    fn slot_bounds_wrap_the_clock() {
        let params = Params::default();
        let now = OffsetDateTime::from_unix_timestamp(1_800_000_123).unwrap();
        let w = at(now, params);
        assert!(w.since <= now && now < w.until);
        assert_eq!((w.until - w.since).whole_seconds(), 12 * 60);
        assert!(STATES.contains(&w.state.as_str()));
    }

    #[test]
    fn force_pins_the_state() {
        let mut params = Params::default();
        params.weather_force = Some(4);
        let now = OffsetDateTime::from_unix_timestamp(1_800_000_123).unwrap();
        assert_eq!(at(now, params).state, "storm");
    }
}
