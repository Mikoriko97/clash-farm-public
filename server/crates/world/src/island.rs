//! Island lifecycle: created lazily on first `/island`, with the starting plots
//! and starter seeds in one transaction; snapshot with database `now`.

use sqlx::{PgPool, Row};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::error::WorldError;
use crate::inventory;
use crate::params::Params;
use crate::placement::assign_beds;
use crate::types::{IslandSnapshot, IslandView, PlotView, RestedView};

pub const STARTER_SEED: &str = "seed_wheat";
pub const STARTER_SEED_2: &str = "seed_carrot";
/// Tutorial crop (0027): grows in 45 s, so the first harvest comes within the first minute.
pub const STARTER_SEED_TUTORIAL: &str = "seed_radish";

pub async fn get_or_create(
    pool: &PgPool,
    user_id: Uuid,
    params: Params,
) -> Result<IslandSnapshot, WorldError> {
    let mut tx = pool.begin().await?;
    let created: Option<Uuid> = sqlx::query_scalar(
        "INSERT INTO cf.islands (user_id) VALUES ($1) ON CONFLICT (user_id) DO NOTHING RETURNING id",
    )
    .bind(user_id)
    .fetch_optional(&mut *tx)
    .await?;
    if let Some(island_id) = created {
        // Starting layout: rows of three north of the centre (same cells as the 0006 backfill).
        for idx in 0..params.plots_start {
            sqlx::query("INSERT INTO cf.plots (island_id, idx, col, row) VALUES ($1, $2, $3, $4)")
                .bind(island_id)
                .bind(idx)
                .bind((idx % 3) - 1)
                .bind((idx / 3) - 3)
                .execute(&mut *tx)
                .await?;
        }
        grant_starter_kit(&mut tx, user_id, island_id, params).await?;
        sqlx::query(
            "INSERT INTO cf.events (user_id, name, props) VALUES ($1, 'island_created', '{}'::jsonb)",
        )
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
        tracing::info!(%user_id, %island_id, "island_created");
    }
    tx.commit().await?;
    snapshot(pool, user_id, params).await
}

/// Starter kit of a new island, inside the transaction that created it (so once per account): resource points,
/// seeds and credits (docs/ECONOMY.md §Starter).
async fn grant_starter_kit(
    tx: &mut sqlx::Transaction<'_, sqlx::Postgres>,
    user_id: Uuid,
    island_id: Uuid,
    params: Params,
) -> Result<(), WorldError> {
    crate::resources::seed_starter(tx, island_id, params).await?;
    if params.starter_wheat_seeds > 0 {
        inventory::add(tx, user_id, STARTER_SEED, params.starter_wheat_seeds).await?;
    }
    if params.starter_carrot_seeds > 0 {
        inventory::add(tx, user_id, STARTER_SEED_2, params.starter_carrot_seeds).await?;
    }
    if params.starter_radish_seeds > 0 {
        inventory::add(tx, user_id, STARTER_SEED_TUTORIAL, params.starter_radish_seeds).await?;
    }
    if params.starter_credits > 0 {
        // Same double entry as a harvest reward: minted once, in the creating transaction.
        let credits = i64::from(params.starter_credits);
        cf_ledger::post(
            tx,
            "island_created",
            island_id,
            &[
                cf_ledger::Entry::system(cf_ledger::SYSTEM_MINT, cf_ledger::Currency::Credits, -credits),
                cf_ledger::Entry::user(
                    user_id,
                    cf_ledger::ACCOUNT_CREDITS,
                    cf_ledger::Currency::Credits,
                    credits,
                ),
            ],
        )
        .await?;
    }
    Ok(())
}

pub async fn snapshot(pool: &PgPool, user_id: Uuid, params: Params) -> Result<IslandSnapshot, WorldError> {
    let island_row = sqlx::query(concat!(
        "SELECT id, level, xp, expand_n, expand_e, expand_s, expand_w, now() AS now, ",
        "CASE WHEN rested_until > now() THEN rested_until END AS rested_until, rested_bonus_pct ",
        "FROM cf.islands WHERE user_id = $1"
    ))
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    .ok_or(WorldError::NotFound)?;
    let island_level: i32 = island_row.try_get("level")?;
    let island = IslandView {
        id: island_row.try_get("id")?,
        level: island_row.try_get("level")?,
        xp: island_row.try_get("xp")?,
        expand_n: island_row.try_get("expand_n")?,
        expand_e: island_row.try_get("expand_e")?,
        expand_s: island_row.try_get("expand_s")?,
        expand_w: island_row.try_get("expand_w")?,
    };
    let now: OffsetDateTime = island_row.try_get("now")?;
    let rested = island_row
        .try_get::<Option<OffsetDateTime>, _>("rested_until")?
        .map(|until| -> Result<RestedView, sqlx::Error> {
            Ok(RestedView {
                until,
                bonus_pct: island_row.try_get("rested_bonus_pct")?,
            })
        })
        .transpose()?;
    let rows = sqlx::query(concat!(
        "SELECT ",
        plot_columns!(),
        " FROM cf.plots p WHERE p.island_id = $1 ORDER BY p.idx"
    ))
    .bind(island.id)
    .fetch_all(pool)
    .await?;
    let mut plots = rows
        .iter()
        .map(PlotView::from_row)
        .collect::<Result<Vec<_>, _>>()?;
    assign_beds(&mut plots);
    let mut conn = pool.acquire().await?;
    let decor = crate::decor::list(&mut conn, island.id).await?;
    let resources = crate::resources::list(&mut conn, island.id, params).await?;
    let cleared = crate::clearings::list(&mut conn, island.id).await?;
    let plot_base = crate::catalog::price_credits(&mut conn, crate::shop::PLOT_ITEM)
        .await?
        .unwrap_or(0);
    let plot_price = crate::shop::plot_price_total(&mut conn, user_id, plot_base, 1, params).await?;
    Ok(IslandSnapshot {
        island,
        rested,
        plots,
        decor,
        resources,
        cleared,
        weather: crate::weather::at(now, params),
        now,
        plot_price,
        plot_limit: crate::placement::plot_limit(island_level, params),
    })
}
