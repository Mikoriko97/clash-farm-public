//! Living island resources (migration 0017, docs/PLAN.md етап 5): a used-up
//! tree or rock comes back somewhere free once its respawn time passes, bought
//! strips get their own share of nodes, and saplings grow by the database clock.
//! The server picks every spot: the client only draws what it is told. Free means
//! inside the island's current shape (base square plus bought strips, corners
//! rounded like the ground), off the house, off every plot and decor footprint
//! and away from other nodes. Runs from the cf-api periodic job; one island per
//! transaction with the island row locked, so placing a plot or decor (which
//! lock the same row) can never race a spawn.

use rand::rngs::StdRng;
use rand::{Rng, RngExt, SeedableRng};
use sqlx::{PgConnection, PgPool, Row};
use uuid::Uuid;

use crate::error::WorldError;
use crate::island_spots::{Circle, Occupancy, HOUSE_DM};
use crate::params::Params;
use crate::placement::CELL_M;
use crate::resources::{footprint_dm, max_hits, KIND_ROCK, KIND_TREE};

/// Half of a plot cell plus a walkway (dm).
const PLOT_CLEAR_DM: i64 = 20;
/// Breathing room between two nodes on top of their footprints (dm).
const SPACING_DM: i64 = 12;
const MAX_VARIANT: i16 = 8;

#[derive(Debug, Default, Clone, Copy)]
pub struct RegrowSummary {
    pub islands: u64,
    pub moved: u64,
    pub planted: u64,
}

/// Nodes an island should carry: starters plus a share per bought strip.
pub fn target(kind: &str, expand: [i16; 4], params: Params) -> i64 {
    let strips: i64 = expand.iter().map(|s| i64::from(*s)).sum();
    if kind == KIND_TREE {
        i64::from(params.starter_trees.max(0)) + strips * i64::from(params.strip_trees.max(0))
    } else {
        i64::from(params.starter_rocks.max(0)) + strips * i64::from(params.strip_rocks.max(0))
    }
}

/// One pass over every island that has something due: used-up nodes past their
/// respawn time, nodes without a spot, or fewer nodes than the target.
pub async fn run(pool: &PgPool, params: Params) -> Result<RegrowSummary, WorldError> {
    let mut out = RegrowSummary::default();
    let islands: Vec<Uuid> = sqlx::query(
        "SELECT i.id FROM cf.islands i
         WHERE EXISTS (SELECT 1 FROM cf.resource_nodes r WHERE r.island_id = i.id
                       AND (r.x_dm IS NULL OR (r.respawn_at IS NOT NULL AND r.respawn_at <= now())))
            OR (SELECT count(*) FROM cf.resource_nodes r WHERE r.island_id = i.id AND r.kind = 'tree')
               < $1 + (i.expand_n + i.expand_e + i.expand_s + i.expand_w) * $2
            OR (SELECT count(*) FROM cf.resource_nodes r WHERE r.island_id = i.id AND r.kind = 'rock')
               < $3 + (i.expand_n + i.expand_e + i.expand_s + i.expand_w) * $4
         ORDER BY random() LIMIT 500",
    )
    .bind(i64::from(params.starter_trees.max(0)))
    .bind(i64::from(params.strip_trees.max(0)))
    .bind(i64::from(params.starter_rocks.max(0)))
    .bind(i64::from(params.strip_rocks.max(0)))
    .fetch_all(pool)
    .await?
    .iter()
    .map(|r| r.try_get("id"))
    .collect::<Result<_, _>>()?;
    for island_id in islands {
        // One island's failure must not starve the rest of the pass.
        match regrow_island(pool, island_id, params).await {
            Ok(done) => {
                out.islands += 1;
                out.moved += done.moved;
                out.planted += done.planted;
            }
            Err(err) => tracing::warn!(%island_id, error = %err, "regrow: island skipped"),
        }
    }
    Ok(out)
}

async fn regrow_island(pool: &PgPool, island_id: Uuid, params: Params) -> Result<RegrowSummary, WorldError> {
    let mut tx = pool.begin().await?;
    let row =
        sqlx::query("SELECT expand_n, expand_e, expand_s, expand_w FROM cf.islands WHERE id = $1 FOR UPDATE")
            .bind(island_id)
            .fetch_optional(&mut *tx)
            .await?;
    let Some(row) = row else {
        return Ok(RegrowSummary::default());
    };
    let expand = [
        row.try_get("expand_n")?,
        row.try_get("expand_e")?,
        row.try_get("expand_s")?,
        row.try_get("expand_w")?,
    ];
    let mut occ = occupancy(&mut tx, island_id, expand).await?;
    // StdRng is Send (the thread-local rng is not), which the job future needs; the
    // seed comes from the thread rng inside this statement, dropped before any await.
    let seed: u64 = rand::rng().random();
    let mut rng = StdRng::seed_from_u64(seed);
    let moved = relocate_due(&mut tx, island_id, &mut occ, &mut rng, params).await?;
    let planted = top_up(&mut tx, island_id, &mut occ, &mut rng, params).await?;
    tx.commit().await?;
    Ok(RegrowSummary {
        islands: 1,
        moved,
        planted,
    })
}

/// Everything already standing: house, plots, decor, nodes that keep their spot.
async fn occupancy(
    conn: &mut PgConnection,
    island_id: Uuid,
    expand: [i16; 4],
) -> Result<Occupancy, WorldError> {
    let mut blocked = vec![Circle {
        x: HOUSE_DM.0,
        z: HOUSE_DM.1,
        r: HOUSE_DM.2,
    }];
    for plot in sqlx::query("SELECT col, row FROM cf.plots WHERE island_id = $1")
        .bind(island_id)
        .fetch_all(&mut *conn)
        .await?
    {
        let col: i16 = plot.try_get("col")?;
        let row: i16 = plot.try_get("row")?;
        blocked.push(Circle {
            x: i64::from(col) * i64::from(CELL_M) * 10,
            z: i64::from(row) * i64::from(CELL_M) * 10,
            r: PLOT_CLEAR_DM,
        });
    }
    for decor in sqlx::query(
        "SELECT p.x_dm, p.z_dm, COALESCE((i.meta->>'footprint_dm')::bigint, 10) AS footprint
         FROM cf.decor_placements p JOIN cf.items i ON i.id = p.item_id WHERE p.island_id = $1",
    )
    .bind(island_id)
    .fetch_all(&mut *conn)
    .await?
    {
        let x: i16 = decor.try_get("x_dm")?;
        let z: i16 = decor.try_get("z_dm")?;
        let r: i64 = decor.try_get("footprint")?;
        blocked.push(Circle {
            x: i64::from(x),
            z: i64::from(z),
            r: r.clamp(1, 100) + SPACING_DM,
        });
    }
    for node in sqlx::query(
        "SELECT kind, x_dm, z_dm FROM cf.resource_nodes
         WHERE island_id = $1 AND x_dm IS NOT NULL AND (respawn_at IS NULL OR respawn_at > now())",
    )
    .bind(island_id)
    .fetch_all(&mut *conn)
    .await?
    {
        let kind: String = node.try_get("kind")?;
        let x: i16 = node.try_get("x_dm")?;
        let z: i16 = node.try_get("z_dm")?;
        blocked.push(Circle {
            x: i64::from(x),
            z: i64::from(z),
            r: footprint_dm(&kind) + SPACING_DM,
        });
    }
    Ok(Occupancy { expand, blocked })
}

/// Used-up nodes past `respawn_at` (and nodes never placed) move to a free spot,
/// fresh and — for trees — as saplings.
async fn relocate_due(
    conn: &mut PgConnection,
    island_id: Uuid,
    occ: &mut Occupancy,
    rng: &mut impl Rng,
    params: Params,
) -> Result<u64, WorldError> {
    let due = sqlx::query(
        "SELECT id, kind FROM cf.resource_nodes
         WHERE island_id = $1 AND (x_dm IS NULL OR (respawn_at IS NOT NULL AND respawn_at <= now()))
         ORDER BY idx",
    )
    .bind(island_id)
    .fetch_all(&mut *conn)
    .await?;
    let mut moved = 0;
    for node in due {
        let id: Uuid = node.try_get("id")?;
        let kind: String = node.try_get("kind")?;
        let Some((x, z)) = occ.find_spot(footprint_dm(&kind), rng) else {
            continue;
        };
        sqlx::query(
            "UPDATE cf.resource_nodes SET x_dm = $2, z_dm = $3, grown_at = now(), respawn_at = NULL,
                    hits_left = $4, variant = $5
             WHERE id = $1 AND (x_dm IS NULL OR (respawn_at IS NOT NULL AND respawn_at <= now()))",
        )
        .bind(id)
        .bind(x)
        .bind(z)
        .bind(max_hits(&kind, params))
        .bind(rng.random_range(0..MAX_VARIANT))
        .execute(&mut *conn)
        .await?;
        occ.blocked.push(Circle {
            x: i64::from(x),
            z: i64::from(z),
            r: footprint_dm(&kind) + SPACING_DM,
        });
        moved += 1;
    }
    Ok(moved)
}

/// Plants what the target (starters + strips) still lacks, as saplings.
async fn top_up(
    conn: &mut PgConnection,
    island_id: Uuid,
    occ: &mut Occupancy,
    rng: &mut impl Rng,
    params: Params,
) -> Result<u64, WorldError> {
    let mut planted = 0;
    for kind in [KIND_TREE, KIND_ROCK] {
        let have: i64 =
            sqlx::query("SELECT count(*) AS n FROM cf.resource_nodes WHERE island_id = $1 AND kind = $2")
                .bind(island_id)
                .bind(kind)
                .fetch_one(&mut *conn)
                .await?
                .try_get("n")?;
        for _ in have..target(kind, occ.expand, params) {
            let Some((x, z)) = occ.find_spot(footprint_dm(kind), rng) else {
                break;
            };
            sqlx::query(
                "INSERT INTO cf.resource_nodes (island_id, idx, kind, hits_left, x_dm, z_dm, grown_at, variant)
                 SELECT $1, COALESCE(MAX(idx), -1) + 1, $2, $3, $4, $5, now(), $6 FROM cf.resource_nodes WHERE island_id = $1",
            )
            .bind(island_id)
            .bind(kind)
            .bind(max_hits(kind, params))
            .bind(x)
            .bind(z)
            .bind(rng.random_range(0..MAX_VARIANT))
            .execute(&mut *conn)
            .await?;
            occ.blocked.push(Circle {
                x: i64::from(x),
                z: i64::from(z),
                r: footprint_dm(kind) + SPACING_DM,
            });
            planted += 1;
        }
    }
    Ok(planted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn targets_count_strips() {
        let params = Params::default();
        assert_eq!(target(KIND_TREE, [0; 4], params), 3);
        assert_eq!(target(KIND_ROCK, [1, 1, 0, 0], params), 7);
    }
}
