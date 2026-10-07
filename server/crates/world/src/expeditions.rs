//! Daily expeditions (migration 0015, docs/MAINLAND.md §4): three tasks a day
//! rolled from (user, day), progress written by the actions themselves inside
//! their transactions, XP paid once per task under the row lock.

use serde::{Deserialize, Serialize};
use sqlx::{PgConnection, PgPool, Row};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::error::WorldError;
use crate::plots::{add_xp, record_event};

pub const TASKS_PER_DAY: usize = 3;

/// The pool every day's tasks are drawn from: (kind, target, count, xp).
const POOL: [(&str, &str, i32, i32); 9] = [
    ("gather", "tree_pine", 8, 30),
    ("gather", "tree_oak", 6, 30),
    ("gather", "rock", 6, 25),
    ("gather", "ore_copper", 4, 40),
    ("gather", "herb", 5, 25),
    ("gather", "mushroom", 3, 35),
    ("gather", "reed", 4, 25),
    ("explore", "", 4, 30),
    ("discover", "", 1, 50),
];

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct Task {
    pub kind: String,
    pub target: String,
    pub count: i32,
    pub xp: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Done {
    #[serde(default)]
    pub progress: i32,
    #[serde(default)]
    pub claimed: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct TaskView {
    pub index: i32,
    pub kind: String,
    pub target: String,
    pub count: i32,
    pub xp: i32,
    pub progress: i32,
    pub claimed: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct ExpeditionsView {
    pub day: String,
    pub tasks: Vec<TaskView>,
}

#[derive(Debug, Clone, Serialize)]
pub struct ClaimResult {
    pub task: TaskView,
    pub xp: i64,
    pub level: i32,
}

fn mix(mut x: u64) -> u64 {
    x = x.wrapping_add(0x9E37_79B9_7F4A_7C15);
    let mut z = x;
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^ (z >> 31)
}

/// Three distinct tasks for (user, day); the same every time it is asked.
pub fn roll(user_id: Uuid, day: &str) -> Vec<Task> {
    let bytes = user_id.as_bytes();
    let lo = u64::from_le_bytes(bytes[..8].try_into().unwrap_or([0; 8]));
    let hi = u64::from_le_bytes(bytes[8..].try_into().unwrap_or([0; 8]));
    let mut state = mix(lo ^ mix(hi));
    for b in day.bytes() {
        state = mix(state ^ u64::from(b));
    }
    let mut picks: Vec<usize> = (0..POOL.len()).collect();
    for i in (1..picks.len()).rev() {
        state = mix(state);
        let j = usize::try_from(state % (i as u64 + 1)).unwrap_or(0);
        picks.swap(i, j);
    }
    picks
        .iter()
        .take(TASKS_PER_DAY)
        .map(|&i| {
            let (kind, target, count, xp) = POOL[i];
            Task {
                kind: kind.to_owned(),
                target: target.to_owned(),
                count,
                xp,
            }
        })
        .collect()
}

fn today(now: OffsetDateTime) -> String {
    now.date().to_string()
}

/// Today's row, created from the roll on first sight; locked when `lock`.
async fn load(
    conn: &mut PgConnection,
    user_id: Uuid,
    lock: bool,
) -> Result<(String, Vec<Task>, serde_json::Value), WorldError> {
    let now: OffsetDateTime = sqlx::query_scalar("SELECT now()").fetch_one(&mut *conn).await?;
    let day = today(now);
    let tasks = roll(user_id, &day);
    sqlx::query(
        "INSERT INTO cf.expeditions (user_id, day, tasks) VALUES ($1, $2::date, $3) ON CONFLICT (user_id, day) DO NOTHING",
    )
    .bind(user_id)
    .bind(&day)
    .bind(serde_json::to_value(&tasks).unwrap_or_default())
    .execute(&mut *conn)
    .await?;
    let sql = if lock {
        "SELECT tasks, done FROM cf.expeditions WHERE user_id = $1 AND day = $2::date FOR UPDATE"
    } else {
        "SELECT tasks, done FROM cf.expeditions WHERE user_id = $1 AND day = $2::date"
    };
    let row = sqlx::query(sql).bind(user_id).bind(&day).fetch_one(conn).await?;
    let mut stored: Vec<Task> = serde_json::from_value(row.try_get("tasks")?).unwrap_or(tasks);
    // Defence in depth: a hand-edited row must not make a task free or absurdly rich.
    for t in &mut stored {
        t.count = t.count.max(1);
        t.xp = t.xp.clamp(0, 10_000);
    }
    let done: serde_json::Value = row.try_get("done")?;
    let done = if done.is_object() {
        done
    } else {
        serde_json::Value::Object(serde_json::Map::new())
    };
    Ok((day, stored, done))
}

/// Writes one task's state without indexing into a non-object (no panics).
fn set_done(done: &mut serde_json::Value, index: usize, d: &Done) {
    if let Some(map) = done.as_object_mut() {
        map.insert(index.to_string(), serde_json::to_value(d).unwrap_or_default());
    }
}

fn done_of(done: &serde_json::Value, index: usize) -> Done {
    done.get(index.to_string())
        .cloned()
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default()
}

fn views(tasks: &[Task], done: &serde_json::Value) -> Vec<TaskView> {
    tasks
        .iter()
        .enumerate()
        .map(|(i, t)| {
            let d = done_of(done, i);
            TaskView {
                index: i32::try_from(i).unwrap_or(0),
                kind: t.kind.clone(),
                target: t.target.clone(),
                count: t.count,
                xp: t.xp,
                progress: d.progress.min(t.count),
                claimed: d.claimed,
            }
        })
        .collect()
}

/// `GET /mainland/expeditions`.
pub async fn view(pool: &PgPool, user_id: Uuid) -> Result<ExpeditionsView, WorldError> {
    let mut tx = pool.begin().await?;
    let (day, tasks, done) = load(&mut tx, user_id, false).await?;
    tx.commit().await?;
    Ok(ExpeditionsView {
        day,
        tasks: views(&tasks, &done),
    })
}

/// Counts `amount` toward every matching task of today (called inside the
/// action's transaction; a task past its count stops growing).
pub async fn progress(
    conn: &mut PgConnection,
    user_id: Uuid,
    kind: &str,
    target: &str,
    amount: i32,
) -> Result<(), WorldError> {
    if amount <= 0 {
        return Ok(());
    }
    let (day, tasks, mut done) = load(conn, user_id, true).await?;
    let mut changed = false;
    for (i, task) in tasks.iter().enumerate() {
        if task.kind != kind || (!task.target.is_empty() && task.target != target) {
            continue;
        }
        let mut d = done_of(&done, i);
        if d.claimed || d.progress >= task.count {
            continue;
        }
        d.progress = d.progress.saturating_add(amount).min(task.count);
        set_done(&mut done, i, &d);
        changed = true;
    }
    if changed {
        sqlx::query(
            "UPDATE cf.expeditions SET done = $3, updated_at = now() WHERE user_id = $1 AND day = $2::date",
        )
        .bind(user_id)
        .bind(&day)
        .bind(&done)
        .execute(conn)
        .await?;
    }
    Ok(())
}

/// `POST /mainland/expeditions/{n}/claim`: pays the task's XP once it is complete.
pub async fn claim(pool: &PgPool, user_id: Uuid, index: usize) -> Result<ClaimResult, WorldError> {
    let mut tx = pool.begin().await?;
    let island_id: Uuid = sqlx::query_scalar("SELECT id FROM cf.islands WHERE user_id = $1 FOR UPDATE")
        .bind(user_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(WorldError::NotFound)?;
    let (day, tasks, mut done) = load(&mut tx, user_id, true).await?;
    let task = tasks.get(index).ok_or(WorldError::NotFound)?;
    let mut d = done_of(&done, index);
    if d.claimed {
        return Err(WorldError::AlreadyClaimed);
    }
    if d.progress < task.count {
        return Err(WorldError::NotReady);
    }
    d.claimed = true;
    set_done(&mut done, index, &d);
    sqlx::query(
        "UPDATE cf.expeditions SET done = $3, updated_at = now() WHERE user_id = $1 AND day = $2::date",
    )
    .bind(user_id)
    .bind(&day)
    .bind(&done)
    .execute(&mut *tx)
    .await?;
    let (xp, level) = add_xp(&mut tx, user_id, island_id, i64::from(task.xp.max(0))).await?;
    record_event(
        &mut tx,
        user_id,
        "expedition_claim",
        serde_json::json!({ "day": day, "index": index, "xp": task.xp }),
    )
    .await?;
    tx.commit().await?;
    let task_view = views(&tasks, &done)
        .into_iter()
        .nth(index)
        .ok_or(WorldError::NotFound)?;
    Ok(ClaimResult {
        task: task_view,
        xp,
        level,
    })
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;

    #[test]
    fn roll_is_stable_and_distinct() {
        let u = Uuid::from_u128(42);
        let a = roll(u, "2026-09-09");
        let b = roll(u, "2026-09-09");
        assert_eq!(a, b);
        assert_eq!(a.len(), 3);
        let keys: std::collections::HashSet<(String, String)> =
            a.iter().map(|t| (t.kind.clone(), t.target.clone())).collect();
        assert_eq!(keys.len(), 3, "three different tasks");
        assert_ne!(roll(u, "2026-09-10"), a, "tomorrow rolls differently");
        assert_ne!(
            roll(Uuid::from_u128(7), "2026-09-09"),
            a,
            "another player rolls differently"
        );
    }
}
