//! Plots placed by the player (migration 0006, docs/ECONOMY.md §Острів).
//! `place` spends a `plot` item on a free cell inside the island bounds, under
//! the level's plot limit and the bed size limit; `remove` deletes an empty plot
//! and returns the item. The island row is locked first so two concurrent
//! placements cannot both pass the count checks; the cell UNIQUE is the last barrier.

use std::collections::{HashMap, HashSet};

use sqlx::{PgConnection, PgPool, Row};
use uuid::Uuid;

use crate::error::WorldError;
use crate::inventory;
use crate::params::Params;
use crate::plots::{lock_plot, record_event};
use crate::types::PlotView;

pub const PLOT_ITEM: &str = "plot";
/// Distance between cell centres (m); the plot itself is 2.4 m wide.
pub const CELL_M: i32 = 3;
/// ADR-015: base island half extent and the strip each paid expansion adds (m).
pub const ISLAND_HALF_M: i32 = 24;
pub const EXPAND_STRIP_M: i32 = 8;
/// Half width of the soil plus the walkable rim kept clear at the island edge (m).
const PLOT_HALF_M: i32 = 2;
/// `cf.plots` CHECK from migration 0006; a wider island needs a new migration first.
pub const CELL_MAX: i16 = 16;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Cell {
    pub col: i16,
    pub row: i16,
}

struct IslandLock {
    id: Uuid,
    level: i32,
    expand: [i16; 4], // n, e, s, w
}

async fn lock_island(conn: &mut PgConnection, user_id: Uuid) -> Result<IslandLock, WorldError> {
    let row = sqlx::query(
        "SELECT id, level, expand_n, expand_e, expand_s, expand_w FROM cf.islands WHERE user_id = $1 FOR UPDATE",
    )
    .bind(user_id)
    .fetch_optional(conn)
    .await?
    .ok_or(WorldError::NotFound)?;
    Ok(IslandLock {
        id: row.try_get("id")?,
        level: row.try_get("level")?,
        expand: [
            row.try_get("expand_n")?,
            row.try_get("expand_e")?,
            row.try_get("expand_s")?,
            row.try_get("expand_w")?,
        ],
    })
}

/// Plot count allowed at `level`: the starters plus one every `plots_per_levels` levels,
/// never more than `plots_max` (docs/ECONOMY_LAB.md §4.1).
pub fn plot_limit(level: i32, params: Params) -> i64 {
    let start = i64::from(params.plots_start);
    let extra = i64::from(level.max(0)) / i64::from(params.plots_per_levels.max(1));
    (start + extra).min(i64::from(params.plots_max)).max(start)
}

/// Whether a plot centred on `cell` stays inside the island top (north = -row, east = +col).
pub fn inside_island(cell: Cell, expand: [i16; 4]) -> bool {
    if cell.col.abs() > CELL_MAX || cell.row.abs() > CELL_MAX {
        return false;
    }
    let [n, e, s, w] = expand.map(|x| i32::from(x) * EXPAND_STRIP_M);
    let x = i32::from(cell.col) * CELL_M;
    let z = i32::from(cell.row) * CELL_M;
    x - PLOT_HALF_M >= -(ISLAND_HALF_M + w)
        && x + PLOT_HALF_M <= ISLAND_HALF_M + e
        && z - PLOT_HALF_M >= -(ISLAND_HALF_M + n)
        && z + PLOT_HALF_M <= ISLAND_HALF_M + s
}

/// Size of the side-adjacent group `cell` would join (including itself) among `occupied`.
pub fn bed_size_with(cell: Cell, occupied: &HashSet<Cell>) -> usize {
    let mut all = occupied.clone();
    all.insert(cell);
    component(cell, &all).len()
}

fn component(start: Cell, cells: &HashSet<Cell>) -> HashSet<Cell> {
    let mut seen = HashSet::new();
    let mut stack = vec![start];
    while let Some(c) = stack.pop() {
        if !cells.contains(&c) || !seen.insert(c) {
            continue;
        }
        for (dc, dr) in [(1, 0), (-1, 0), (0, 1), (0, -1)] {
            stack.push(Cell {
                col: c.col + dc,
                row: c.row + dr,
            });
        }
    }
    seen
}

/// Numbers the beds of an island (0-based, in order of the lowest idx in each bed).
pub fn assign_beds(plots: &mut [PlotView]) {
    let cells: HashSet<Cell> = plots
        .iter()
        .map(|p| Cell {
            col: p.col,
            row: p.row,
        })
        .collect();
    let mut bed_of: HashMap<Cell, i16> = HashMap::new();
    let mut next: i16 = 0;
    for plot in plots.iter() {
        let cell = Cell {
            col: plot.col,
            row: plot.row,
        };
        if bed_of.contains_key(&cell) {
            continue;
        }
        for member in component(cell, &cells) {
            bed_of.insert(member, next);
        }
        next += 1;
    }
    for plot in plots.iter_mut() {
        plot.bed = bed_of
            .get(&Cell {
                col: plot.col,
                row: plot.row,
            })
            .copied()
            .unwrap_or(0);
    }
}

async fn occupied_cells(conn: &mut PgConnection, island_id: Uuid) -> Result<HashSet<Cell>, WorldError> {
    let rows = sqlx::query("SELECT col, row FROM cf.plots WHERE island_id = $1")
        .bind(island_id)
        .fetch_all(conn)
        .await?;
    rows.iter()
        .map(|r| {
            Ok(Cell {
                col: r.try_get("col")?,
                row: r.try_get("row")?,
            })
        })
        .collect()
}

/// `POST /plots/place { col, row }`: one `plot` item → a new empty plot on the cell.
pub async fn place(pool: &PgPool, user_id: Uuid, cell: Cell, params: Params) -> Result<PlotView, WorldError> {
    let mut tx = pool.begin().await?;
    let island = lock_island(&mut tx, user_id).await?;
    if !inside_island(cell, island.expand) {
        return Err(WorldError::OutOfBounds);
    }
    let occupied = occupied_cells(&mut tx, island.id).await?;
    if occupied.contains(&cell) {
        return Err(WorldError::CellTaken);
    }
    if occupied.len() as i64 >= plot_limit(island.level, params) {
        return Err(WorldError::PlotLimit);
    }
    if bed_size_with(cell, &occupied) > usize::from(params.bed_max_cells.max(1) as u16) {
        return Err(WorldError::BedTooLarge);
    }
    inventory::take_one(&mut tx, user_id, PLOT_ITEM).await?;
    let plot_id: Uuid = sqlx::query_scalar(
        "INSERT INTO cf.plots (island_id, idx, col, row)
         SELECT $1, COALESCE(MAX(idx), -1) + 1, $2, $3 FROM cf.plots WHERE island_id = $1
         RETURNING id",
    )
    .bind(island.id)
    .bind(cell.col)
    .bind(cell.row)
    .fetch_one(&mut *tx)
    .await?;
    record_event(
        &mut tx,
        user_id,
        "plot_place",
        serde_json::json!({ "plot": plot_id, "col": cell.col, "row": cell.row }),
    )
    .await?;
    let plot = crate::plots::reload(&mut tx, plot_id).await?;
    tx.commit().await?;
    Ok(plot)
}

/// `POST /plots/{id}/remove`: an empty plot goes back to the inventory as a `plot` item.
pub async fn remove(pool: &PgPool, user_id: Uuid, plot_id: Uuid) -> Result<(), WorldError> {
    let mut tx = pool.begin().await?;
    let locked = lock_plot(&mut tx, user_id, plot_id).await?;
    if locked.plot.state != "empty" {
        return Err(WorldError::WrongState);
    }
    sqlx::query("DELETE FROM cf.plots WHERE id = $1")
        .bind(plot_id)
        .execute(&mut *tx)
        .await?;
    inventory::add(&mut tx, user_id, PLOT_ITEM, 1).await?;
    record_event(
        &mut tx,
        user_id,
        "plot_remove",
        serde_json::json!({ "plot": plot_id, "col": locked.plot.col, "row": locked.plot.row }),
    )
    .await?;
    tx.commit().await?;
    Ok(())
}

/// All plots of the bed containing `plot_id`, owner-checked. Lock order is the
/// same for every bed call (island row, then plots by id) so two concurrent
/// calls on one island queue up instead of deadlocking.
pub(crate) async fn lock_bed(
    conn: &mut PgConnection,
    user_id: Uuid,
    plot_id: Uuid,
) -> Result<(Uuid, Vec<PlotView>), WorldError> {
    let island = lock_island(conn, user_id).await?;
    let rows = sqlx::query(concat!(
        "SELECT ",
        plot_columns!(),
        " FROM cf.plots p WHERE p.island_id = $1 ORDER BY p.id FOR UPDATE"
    ))
    .bind(island.id)
    .fetch_all(conn)
    .await?;
    let all = rows
        .iter()
        .map(PlotView::from_row)
        .collect::<Result<Vec<_>, _>>()?;
    // A foreign or unknown plot is simply not on this island: 404, never confirmed.
    let seed = all.iter().find(|p| p.id == plot_id).ok_or(WorldError::NotFound)?;
    let cells: HashSet<Cell> = all
        .iter()
        .map(|p| Cell {
            col: p.col,
            row: p.row,
        })
        .collect();
    let members = component(
        Cell {
            col: seed.col,
            row: seed.row,
        },
        &cells,
    );
    let bed = all
        .into_iter()
        .filter(|p| {
            members.contains(&Cell {
                col: p.col,
                row: p.row,
            })
        })
        .collect();
    Ok((island.id, bed))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn limit_grows_one_plot_per_two_levels_to_the_max() {
        let p = Params::default();
        assert_eq!(plot_limit(1, p), 6);
        assert_eq!(plot_limit(2, p), 7);
        assert_eq!(plot_limit(3, p), 7);
        assert_eq!(plot_limit(10, p), 11);
        assert_eq!(plot_limit(30, p), 20);
        assert_eq!(plot_limit(99, p), 20);
    }

    #[test]
    fn bounds_follow_expansions() {
        let base = [0, 0, 0, 0];
        assert!(inside_island(Cell { col: 7, row: -7 }, base));
        assert!(!inside_island(Cell { col: 8, row: 0 }, base));
        assert!(inside_island(Cell { col: 9, row: 0 }, [0, 1, 0, 0]));
        assert!(!inside_island(Cell { col: 0, row: -9 }, [0, 1, 0, 0]));
        // Four paid strips would allow col 18, but the 0006 CHECK stops at 16 until a later migration.
        assert!(inside_island(Cell { col: 16, row: 0 }, [0, 4, 0, 0]));
        assert!(!inside_island(Cell { col: 17, row: 0 }, [0, 4, 0, 0]));
    }

    #[test]
    fn bed_size_counts_side_neighbours_only() {
        let occupied: HashSet<Cell> = [(0, 0), (1, 0), (2, 0), (5, 5)]
            .into_iter()
            .map(|(col, row)| Cell { col, row })
            .collect();
        assert_eq!(bed_size_with(Cell { col: 3, row: 0 }, &occupied), 4);
        assert_eq!(bed_size_with(Cell { col: 3, row: 1 }, &occupied), 1);
        assert_eq!(bed_size_with(Cell { col: 1, row: 1 }, &occupied), 4);
    }
}
