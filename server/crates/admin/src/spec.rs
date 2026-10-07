//! What an operator may change in `settings`, with type and bounds. Anything
//! not listed here cannot be written through the admin (defaults and meaning:
//! docs/ECONOMY.md, cf-world `params.rs`). `burn_share_bp` is deliberately
//! absent: the 70/30 split is ADR-017, not a knob.

use serde_json::Value;
use sqlx::{PgPool, Row};
use time::OffsetDateTime;

use crate::audit;
use crate::error::AdminError;

#[derive(Debug, Clone, Copy)]
pub enum Kind {
    Switch,
    Int { min: i64, max: i64 },
    Weather,
}

#[derive(Debug, Clone, Copy)]
pub struct Spec {
    pub key: &'static str,
    pub kind: Kind,
    pub note: &'static str,
}

pub const SWITCHES: [&str; 6] = [
    "maintenance",
    "shop_enabled",
    "payments_enabled",
    "registrations_enabled",
    "guests_enabled",
    "renames_enabled",
];

const fn int(key: &'static str, min: i64, max: i64, note: &'static str) -> Spec {
    Spec {
        key,
        kind: Kind::Int { min, max },
        note,
    }
}

pub static SPECS: &[Spec] = &[
    Spec {
        key: "maintenance",
        kind: Kind::Switch,
        note: "усе під /api → 503",
    },
    Spec {
        key: "shop_enabled",
        kind: Kind::Switch,
        note: "магазин і замовлення → 409",
    },
    Spec {
        key: "payments_enabled",
        kind: Kind::Switch,
        note: "замовлення CLASH → 409",
    },
    Spec {
        key: "registrations_enabled",
        kind: Kind::Switch,
        note: "нові гаманці не створюються",
    },
    Spec {
        key: "guests_enabled",
        kind: Kind::Switch,
        note: "нові гостьові острови без гаманця (0028)",
    },
    Spec {
        key: "renames_enabled",
        kind: Kind::Switch,
        note: "платна зміна ніка, куплені квитки теж (0030)",
    },
    int("economy.plots_start", 1, 12, "стартові грядки"),
    int("economy.plots_per_levels", 1, 20, "рівнів на +1 грядку"),
    int("economy.plots_max", 1, 64, "макс. грядок"),
    int(
        "economy.plot_price_step",
        0,
        100_000,
        "подорожчання кожної грядки",
    ),
    int("economy.credits_cap_base", 0, 1_000_000, "кап кредитів: база"),
    int(
        "economy.credits_cap_per_level",
        0,
        100_000,
        "кап кредитів: за рівень",
    ),
    int("economy.plant_xp", 0, 100, "XP за посадку"),
    int(
        "economy.premium_fert_per_day",
        0,
        10_000,
        "преміум-добрив на добу",
    ),
    int("economy.stipend_wheat", 0, 100, "стипендія: пшениці на добу"),
    int("economy.bed_max_cells", 1, 9, "клітинок у ділянці"),
    int("economy.harvest_max_per_day", 0, 10_000, "кап збору"),
    int("economy.plant_max_per_day", 0, 10_000, "кап посадки"),
    int(
        "economy.credits_max_per_day",
        0,
        1_000_000,
        "стеля капу кредитів за день",
    ),
    int("economy.gather_max_per_day", 0, 10_000, "кап ударів по ресурсах"),
    int(
        "alerts.review_queue_max",
        1,
        1_000,
        "сигнал: замовлень на перевірці",
    ),
    int(
        "alerts.review_age_max_min",
        5,
        1_440,
        "сигнал: найстаріше на перевірці, хв",
    ),
    int(
        "alerts.switch_off_max_min",
        1,
        1_440,
        "сигнал: перемикач вимкнений, хв",
    ),
    int(
        "alerts.signups_per_hour_max",
        10,
        100_000,
        "сигнал: реєстрацій за годину",
    ),
    int("economy.tree_hits", 1, 100, "ударів на дерево"),
    int("economy.rock_hits", 1, 100, "ударів на камінь"),
    int("economy.tree_respawn_secs", 60, 604_800, "відновлення дерева, с"),
    int("economy.rock_respawn_secs", 60, 604_800, "відновлення каменя, с"),
    int("economy.gather_xp", 0, 100, "XP за удар"),
    int("economy.craft_max_per_day", 0, 10_000, "кап крафту"),
    int("economy.eat_max_per_day", 0, 10_000, "кап страв"),
    int("economy.sleep_max_per_day", 0, 10, "снів на добу"),
    int("economy.sleep_xp", 0, 1_000, "XP за сон"),
    int(
        "economy.rested_minutes",
        0,
        1_440,
        "хвилин «Відпочив» після сну (0 = вимкнено)",
    ),
    int(
        "economy.rested_bonus_pct",
        0,
        100,
        "% додаткового XP, поки «Відпочив»",
    ),
    int("economy.lightning_wood", 0, 100, "деревини за блискавку"),
    int("economy.lightning_max_per_day", 0, 100, "блискавок на добу"),
    int(
        "economy.exclusive_min_reputation",
        0,
        10_000,
        "репутація для exclusive",
    ),
    int("economy.decor_slots", 0, 500, "макс. декору"),
    int("economy.shop_max_qty", 1, 1_000, "макс. кількість у покупці"),
    int(
        "economy.order_min_usd_cents",
        1,
        100_000,
        "мінімальне замовлення, центи",
    ),
    int("economy.quote_ttl_sec", 30, 3_600, "життя котирування, с"),
    int(
        "economy.pay_confirmed_max_usd_cents",
        0,
        1_000_000,
        "до цієї суми досить confirmed",
    ),
    int("weather.period_min", 1, 1_440, "слот погоди, хв"),
    int("weather.seed", i64::MIN, i64::MAX, "зерно розкладу погоди"),
    Spec {
        key: "weather.force",
        kind: Kind::Weather,
        note: "закріпити погоду (auto = розклад)",
    },
];

pub fn spec(key: &str) -> Option<&'static Spec> {
    SPECS.iter().find(|s| s.key == key)
}

/// Parses a form value for `spec`; the message is shown to the operator.
pub fn parse(spec: &Spec, raw: &str) -> Result<Value, String> {
    let raw = raw.trim();
    match spec.kind {
        Kind::Switch => match raw {
            "true" | "on" | "1" => Ok(Value::Bool(true)),
            "false" | "off" | "0" => Ok(Value::Bool(false)),
            _ => Err("очікується true або false".to_owned()),
        },
        Kind::Int { min, max } => {
            let n: i64 = raw.parse().map_err(|_| "очікується ціле число".to_owned())?;
            if n < min || n > max {
                return Err(format!("дозволено від {min} до {max}"));
            }
            Ok(Value::from(n))
        }
        Kind::Weather => match raw {
            "auto" | "" => Ok(Value::Null),
            "clear" | "cloudy" | "fog" | "rain" | "storm" => Ok(Value::from(raw)),
            _ => Err("clear, cloudy, fog, rain, storm або auto".to_owned()),
        },
    }
}

#[derive(Debug, Clone)]
pub struct Current {
    pub spec: &'static Spec,
    pub value: Option<Value>,
    pub updated_at: Option<OffsetDateTime>,
    pub updated_by: Option<String>,
}

/// Every editable key with its stored value (None = default from code).
pub async fn all(pool: &PgPool) -> Result<Vec<Current>, sqlx::Error> {
    let rows = sqlx::query("SELECT key, value, updated_at, updated_by FROM cf.settings")
        .fetch_all(pool)
        .await?;
    let mut out = Vec::with_capacity(SPECS.len());
    for spec in SPECS {
        let row = rows
            .iter()
            .find(|r| r.try_get::<String, _>("key").ok().as_deref() == Some(spec.key));
        out.push(Current {
            spec,
            value: row.and_then(|r| r.try_get("value").ok()),
            updated_at: row.and_then(|r| r.try_get("updated_at").ok()),
            updated_by: row.and_then(|r| r.try_get("updated_by").ok()),
        });
    }
    Ok(out)
}

/// Validates, writes the setting and the audit row in one transaction.
pub async fn set(
    pool: &PgPool,
    actor: &str,
    ip_hash: Option<&[u8]>,
    key: &str,
    raw: &str,
) -> Result<(Option<Value>, Value), AdminError> {
    let spec = spec(key).ok_or_else(|| AdminError::BadRequest(format!("невідомий параметр {key}")))?;
    let value = parse(spec, raw).map_err(|m| AdminError::BadRequest(format!("{key}: {m}")))?;
    let mut tx = pool.begin().await?;
    let before: Option<Value> = sqlx::query_scalar("SELECT value FROM cf.settings WHERE key = $1 FOR UPDATE")
        .bind(spec.key)
        .fetch_optional(&mut *tx)
        .await?;
    sqlx::query(
        "INSERT INTO cf.settings (key, value, updated_at, updated_by) VALUES ($1, $2, now(), $3)
         ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value, updated_at = now(), updated_by = EXCLUDED.updated_by",
    )
    .bind(spec.key)
    .bind(&value)
    .bind(actor)
    .execute(&mut *tx)
    .await?;
    audit::record(
        &mut tx,
        actor,
        "setting",
        Some(spec.key),
        before.clone().unwrap_or(Value::Null),
        value.clone(),
        ip_hash,
    )
    .await?;
    tx.commit().await?;
    tracing::info!(actor, key = spec.key, "admin_setting_changed");
    Ok((before, value))
}
