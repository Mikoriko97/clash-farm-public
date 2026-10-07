//! Exploration (migration 0014, docs/MAINLAND.md §4): the fog mask of chunks a
//! player has stood in, landmarks, and the one-time discovery reward. The
//! caller passes the position the presence socket vouched for; nothing here
//! trusts client coordinates.

use serde::Serialize;
use sqlx::{PgConnection, PgPool, Row};
use uuid::Uuid;

use crate::error::WorldError;
use crate::expeditions;
use crate::mainland::{self, CHUNKS};
use crate::plots::{add_xp, record_event};
use crate::{inventory, tools};

/// 16 × 16 chunks → 256 bits → 32 bytes.
pub const MASK_BYTES: usize = 32;
/// Chunks revealed around the one the character stands in.
pub const REVEAL_RADIUS: i32 = 1;

#[derive(Debug, Clone, Serialize)]
pub struct LandmarkView {
    pub id: String,
    pub name: String,
    pub x_dm: i32,
    pub z_dm: i32,
    pub radius_dm: i32,
    pub xp: i32,
    pub discovered: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExploredView {
    /// 64 hex chars, bit `cz * 16 + cx` set when the chunk is explored.
    pub mask_hex: String,
    pub explored: i32,
    /// Landmarks the player has discovered or that stand in an explored chunk.
    pub landmarks: Vec<LandmarkView>,
    pub total_landmarks: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExploreResult {
    pub view: ExploredView,
    /// Chunks that were fog until this call.
    pub revealed: Vec<(i32, i32)>,
    pub xp: i64,
    pub level: i32,
}

#[derive(Debug, Clone, Serialize)]
pub struct DiscoverResult {
    pub landmark: LandmarkView,
    pub reward: Vec<tools::RepairCost>,
    pub xp: i64,
    pub level: i32,
}

/// Chunk of a world position in decimetres (the client's chunk_of).
pub fn chunk_of(x_dm: i32, z_dm: i32) -> (i32, i32) {
    let cx = (x_dm + 5_120).div_euclid(mainland::CHUNK_DM);
    let cz = (z_dm + 5_120).div_euclid(mainland::CHUNK_DM);
    (cx, cz)
}

pub fn bit_set(mask: &[u8], cx: i32, cz: i32) -> bool {
    if !mainland::in_range(cx, cz) {
        return false;
    }
    let bit = usize::try_from(cz * CHUNKS + cx).unwrap_or(0);
    mask.get(bit / 8).is_some_and(|b| b & (1 << (bit % 8)) != 0)
}

fn set_bit(mask: &mut [u8; MASK_BYTES], cx: i32, cz: i32) -> bool {
    if !mainland::in_range(cx, cz) {
        return false;
    }
    let bit = usize::try_from(cz * CHUNKS + cx).unwrap_or(0);
    let was = mask[bit / 8] & (1 << (bit % 8)) != 0;
    mask[bit / 8] |= 1 << (bit % 8);
    !was
}

pub fn count_bits(mask: &[u8]) -> i32 {
    i32::try_from(mask.iter().map(|b| b.count_ones()).sum::<u32>()).unwrap_or(i32::MAX)
}

fn padded(raw: Vec<u8>) -> [u8; MASK_BYTES] {
    let mut mask = [0u8; MASK_BYTES];
    for (i, b) in raw.iter().take(MASK_BYTES).enumerate() {
        mask[i] = *b;
    }
    mask
}

async fn load_mask(
    conn: &mut PgConnection,
    user_id: Uuid,
    lock: bool,
) -> Result<[u8; MASK_BYTES], WorldError> {
    let sql = if lock {
        "SELECT mask FROM cf.explored WHERE user_id = $1 FOR UPDATE"
    } else {
        "SELECT mask FROM cf.explored WHERE user_id = $1"
    };
    let raw: Option<Vec<u8>> = sqlx::query_scalar(sql).bind(user_id).fetch_optional(conn).await?;
    Ok(padded(raw.unwrap_or_default()))
}

async fn landmarks_for(
    conn: &mut PgConnection,
    user_id: Uuid,
    mask: &[u8],
) -> Result<(Vec<LandmarkView>, i32), WorldError> {
    let rows = sqlx::query(
        "SELECT l.id, l.name, l.x_dm, l.z_dm, l.radius_dm, l.xp, (d.user_id IS NOT NULL) AS discovered
         FROM cf.landmarks l LEFT JOIN cf.discoveries d ON d.landmark_id = l.id AND d.user_id = $1
         WHERE l.enabled ORDER BY l.id",
    )
    .bind(user_id)
    .fetch_all(conn)
    .await?;
    let total = i32::try_from(rows.len()).unwrap_or(i32::MAX);
    let mut out = Vec::new();
    for row in &rows {
        let view = LandmarkView {
            id: row.try_get("id")?,
            name: row.try_get("name")?,
            x_dm: row.try_get("x_dm")?,
            z_dm: row.try_get("z_dm")?,
            radius_dm: row.try_get("radius_dm")?,
            xp: row.try_get("xp")?,
            discovered: row.try_get("discovered")?,
        };
        let (cx, cz) = chunk_of(view.x_dm, view.z_dm);
        if view.discovered || bit_set(mask, cx, cz) {
            out.push(view);
        }
    }
    Ok((out, total))
}

fn view_of(mask: &[u8; MASK_BYTES], landmarks: Vec<LandmarkView>, total: i32) -> ExploredView {
    ExploredView {
        mask_hex: hex::encode(mask),
        explored: count_bits(mask),
        landmarks,
        total_landmarks: total,
    }
}

/// `GET /mainland/explored`.
pub async fn view(pool: &PgPool, user_id: Uuid) -> Result<ExploredView, WorldError> {
    let mut conn = pool.acquire().await?;
    let mask = load_mask(&mut conn, user_id, false).await?;
    let (landmarks, total) = landmarks_for(&mut conn, user_id, &mask).await?;
    Ok(view_of(&mask, landmarks, total))
}

/// `POST /mainland/explore`: reveals the chunk at (x, z) and its neighbours;
/// newly revealed chunks count toward "explore" expeditions.
pub async fn explore(
    pool: &PgPool,
    user_id: Uuid,
    x_dm: i32,
    z_dm: i32,
) -> Result<ExploreResult, WorldError> {
    // Presence allows a strip of sea past the map edge: clamp to the last chunk.
    let (cx, cz) = chunk_of(x_dm, z_dm);
    let (cx, cz) = (cx.clamp(0, CHUNKS - 1), cz.clamp(0, CHUNKS - 1));
    let mut tx = pool.begin().await?;
    let island_id: Uuid = sqlx::query_scalar("SELECT id FROM cf.islands WHERE user_id = $1 FOR UPDATE")
        .bind(user_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(WorldError::NotFound)?;
    sqlx::query("INSERT INTO cf.explored (user_id) VALUES ($1) ON CONFLICT (user_id) DO NOTHING")
        .bind(user_id)
        .execute(&mut *tx)
        .await?;
    let mut mask = load_mask(&mut tx, user_id, true).await?;
    let mut revealed = Vec::new();
    for dz in -REVEAL_RADIUS..=REVEAL_RADIUS {
        for dx in -REVEAL_RADIUS..=REVEAL_RADIUS {
            if set_bit(&mut mask, cx + dx, cz + dz) {
                revealed.push((cx + dx, cz + dz));
            }
        }
    }
    if !revealed.is_empty() {
        sqlx::query("UPDATE cf.explored SET mask = $2, updated_at = now() WHERE user_id = $1")
            .bind(user_id)
            .bind(mask.as_slice())
            .execute(&mut *tx)
            .await?;
        let n = i32::try_from(revealed.len()).unwrap_or(i32::MAX);
        expeditions::progress(&mut tx, user_id, "explore", "", n).await?;
        record_event(
            &mut tx,
            user_id,
            "explore",
            serde_json::json!({ "chunks": revealed }),
        )
        .await?;
    }
    let (landmarks, total) = landmarks_for(&mut tx, user_id, &mask).await?;
    let (xp, level): (i64, i32) = sqlx::query_as("SELECT xp, level FROM cf.islands WHERE id = $1")
        .bind(island_id)
        .fetch_one(&mut *tx)
        .await?;
    tx.commit().await?;
    Ok(ExploreResult {
        view: view_of(&mask, landmarks, total),
        revealed,
        xp,
        level,
    })
}

/// `POST /mainland/discover`: first visit within `radius_dm` pays XP and items.
pub async fn discover(
    pool: &PgPool,
    user_id: Uuid,
    landmark_id: &str,
    x_dm: i32,
    z_dm: i32,
) -> Result<DiscoverResult, WorldError> {
    let mut tx = pool.begin().await?;
    let island_id: Uuid = sqlx::query_scalar("SELECT id FROM cf.islands WHERE user_id = $1 FOR UPDATE")
        .bind(user_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(WorldError::NotFound)?;
    let row = sqlx::query(
        "SELECT id, name, x_dm, z_dm, radius_dm, xp, reward FROM cf.landmarks WHERE id = $1 AND enabled",
    )
    .bind(landmark_id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(WorldError::NotFound)?;
    let lx: i32 = row.try_get("x_dm")?;
    let lz: i32 = row.try_get("z_dm")?;
    let radius = i64::from(row.try_get::<i32, _>("radius_dm")?);
    if mainland::dist2_dm(lx, lz, x_dm, z_dm) > radius * radius {
        return Err(WorldError::TooFar);
    }
    let inserted = sqlx::query(
        "INSERT INTO cf.discoveries (user_id, landmark_id) VALUES ($1, $2) ON CONFLICT DO NOTHING",
    )
    .bind(user_id)
    .bind(landmark_id)
    .execute(&mut *tx)
    .await?;
    if inserted.rows_affected() == 0 {
        return Err(WorldError::AlreadyDiscovered);
    }
    let reward_json: serde_json::Value = row.try_get("reward")?;
    let mut reward = Vec::new();
    if let Some(items) = reward_json.as_object() {
        for (item_id, qty) in items {
            let qty = i32::try_from(qty.as_i64().unwrap_or(0)).unwrap_or(0);
            if qty > 0 {
                inventory::add(&mut tx, user_id, item_id, qty).await?;
                reward.push(tools::RepairCost {
                    item_id: item_id.clone(),
                    qty,
                });
            }
        }
    }
    let landmark_xp: i32 = row.try_get("xp")?;
    let (xp, level) = add_xp(&mut tx, user_id, island_id, i64::from(landmark_xp.max(0))).await?;
    expeditions::progress(&mut tx, user_id, "discover", "", 1).await?;
    record_event(
        &mut tx,
        user_id,
        "discover",
        serde_json::json!({ "landmark": landmark_id }),
    )
    .await?;
    tx.commit().await?;
    Ok(DiscoverResult {
        landmark: LandmarkView {
            id: row.try_get("id")?,
            name: row.try_get("name")?,
            x_dm: lx,
            z_dm: lz,
            radius_dm: row.try_get("radius_dm")?,
            xp: landmark_xp,
            discovered: true,
        },
        reward,
        xp,
        level,
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;

    #[test]
    fn chunks_and_bits() {
        assert_eq!(chunk_of(0, 4000), (8, 14));
        assert_eq!(chunk_of(-5120, -5120), (0, 0));
        assert_eq!(chunk_of(5119, 5119), (15, 15));
        let mut mask = [0u8; MASK_BYTES];
        assert!(set_bit(&mut mask, 8, 14));
        assert!(!set_bit(&mut mask, 8, 14), "second set is a no-op");
        assert!(bit_set(&mask, 8, 14));
        assert!(!bit_set(&mask, 9, 14));
        assert!(!set_bit(&mut mask, 16, 0), "outside the map");
        assert_eq!(count_bits(&mask), 1);
        assert_eq!(hex::encode(mask).len(), 64);
    }
}
