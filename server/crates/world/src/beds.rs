//! One action over a whole bed (side-adjacent plots, docs/PLAN.md): every
//! applicable plot is handled in one transaction with the same per-plot rules
//! (caps, seeds, credits count per cell). All-or-nothing: a cap or an empty
//! inventory midway rolls the whole bed back.

use serde::Serialize;
use sqlx::PgPool;
use uuid::Uuid;

use crate::error::WorldError;
use crate::params::Params;
use crate::placement::lock_bed;
use crate::plots::{self, Locked};
use crate::types::{ItemQty, PlotView};

#[derive(Debug, Clone, Serialize)]
pub struct BedResult {
    pub plots: Vec<PlotView>,
    /// Plots actually changed by this call.
    pub count: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct BedHarvest {
    pub plots: Vec<PlotView>,
    pub count: usize,
    pub xp: i64,
    pub level: i32,
    /// Produce added to the inventory, summed per item.
    pub items: Vec<ItemQty>,
}

/// Plants every empty plot of the bed with `item_id` (one seed each).
pub async fn plant(
    pool: &PgPool,
    user_id: Uuid,
    plot_id: Uuid,
    item_id: &str,
    params: Params,
) -> Result<BedResult, WorldError> {
    let mut tx = pool.begin().await?;
    let (island_id, bed) = lock_bed(&mut tx, user_id, plot_id).await?;
    let seed = plots::seed_item(&mut tx, user_id, item_id, params).await?;
    let mut out = Vec::with_capacity(bed.len());
    let mut count = 0;
    for plot in bed {
        if plot.state == "empty" {
            let locked = Locked { island_id, plot };
            out.push(plots::plant_in(&mut tx, user_id, &locked, &seed, params).await?);
            count += 1;
        } else {
            out.push(plot);
        }
    }
    if count == 0 {
        return Err(WorldError::WrongState);
    }
    tx.commit().await?;
    Ok(BedResult { plots: out, count })
}

/// Fertilizes every growing, not yet fertilized plot of the bed.
pub async fn fertilize(
    pool: &PgPool,
    user_id: Uuid,
    plot_id: Uuid,
    item_id: &str,
    params: Params,
) -> Result<BedResult, WorldError> {
    let mut tx = pool.begin().await?;
    let (island_id, bed) = lock_bed(&mut tx, user_id, plot_id).await?;
    let fert = plots::fertilizer_item(&mut tx, user_id, item_id, params).await?;
    let mut out = Vec::with_capacity(bed.len());
    let mut count = 0;
    for plot in bed {
        if plot.state == "growing" && plot.fertilizer_id.is_none() {
            let locked = Locked { island_id, plot };
            out.push(plots::fertilize_in(&mut tx, user_id, &locked, &fert, params).await?);
            count += 1;
        } else {
            out.push(plot);
        }
    }
    if count == 0 {
        return Err(WorldError::WrongState);
    }
    tx.commit().await?;
    Ok(BedResult { plots: out, count })
}

/// Harvests every ready plot of the bed; XP, level and credits are the totals.
pub async fn harvest(
    pool: &PgPool,
    user_id: Uuid,
    plot_id: Uuid,
    params: Params,
) -> Result<BedHarvest, WorldError> {
    let mut tx = pool.begin().await?;
    let (island_id, bed) = lock_bed(&mut tx, user_id, plot_id).await?;
    let mut result = BedHarvest {
        plots: Vec::with_capacity(bed.len()),
        count: 0,
        xp: 0,
        level: 0,
        items: Vec::new(),
    };
    let mut growing = false;
    for plot in bed {
        if plot.state == "ready" {
            let locked = Locked { island_id, plot };
            let one = plots::harvest_in(&mut tx, user_id, &locked, params).await?;
            result.count += 1;
            result.xp = one.xp;
            result.level = one.level;
            match result.items.iter_mut().find(|i| i.item_id == one.item_id) {
                Some(entry) => entry.qty += one.qty,
                None => result.items.push(ItemQty {
                    item_id: one.item_id.clone(),
                    qty: one.qty,
                }),
            }
            result.plots.push(one.plot);
        } else {
            growing |= plot.state == "growing";
            result.plots.push(plot);
        }
    }
    if result.count == 0 {
        return Err(if growing {
            WorldError::NotReady
        } else {
            WorldError::WrongState
        });
    }
    tx.commit().await?;
    Ok(result)
}
