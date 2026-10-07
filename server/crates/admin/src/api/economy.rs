//! Economy: every `settings` key with its spec, bounds and last change; the catalog
//! (read-only in stage 1). Writes go through `spec::set`, which validates and audits.

use axum::extract::{Path, State};
use axum::http::HeaderMap;
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sqlx::Row;
use time::OffsetDateTime;

use crate::api::{ApiAdmin, ApiError};
use crate::audit;
use crate::error::AdminError;
use crate::spec::{self, Kind, SWITCHES};
use crate::state::AdminState;

#[derive(Debug, Serialize)]
pub struct SettingView {
    pub key: String,
    /// `switch` | `int` | `weather`
    pub kind: &'static str,
    pub min: Option<i64>,
    pub max: Option<i64>,
    pub note: &'static str,
    /// Stored value; `null` = the default from code (docs/ECONOMY.md).
    pub value: Value,
    #[serde(with = "time::serde::rfc3339::option")]
    pub updated_at: Option<OffsetDateTime>,
    pub updated_by: Option<String>,
}

/// `GET /api/settings`
pub async fn settings(
    State(state): State<AdminState>,
    _admin: ApiAdmin,
) -> Result<Json<Vec<SettingView>>, ApiError> {
    let rows = spec::all(&state.pool).await?;
    Ok(Json(
        rows.into_iter()
            .map(|c| {
                let (kind, min, max) = match c.spec.kind {
                    Kind::Switch => ("switch", None, None),
                    Kind::Int { min, max } => ("int", Some(min), Some(max)),
                    Kind::Weather => ("weather", None, None),
                };
                SettingView {
                    key: c.spec.key.to_owned(),
                    kind,
                    min,
                    max,
                    note: c.spec.note,
                    value: c.value.unwrap_or(Value::Null),
                    updated_at: c.updated_at,
                    updated_by: c.updated_by,
                }
            })
            .collect(),
    ))
}

#[derive(Debug, Deserialize)]
pub struct SetBody {
    pub key: String,
    /// Raw text as typed; `spec::parse` turns it into the stored JSON (empty = default).
    pub value: String,
}

/// `POST /api/settings { key, value }` → `{ before, after }`; kill switches are logged louder.
pub async fn set_setting(
    State(state): State<AdminState>,
    admin: ApiAdmin,
    headers: HeaderMap,
    Json(body): Json<SetBody>,
) -> Result<Json<Value>, ApiError> {
    admin.guard(&state, &headers).await?;
    let key: String = body.key.chars().take(64).collect();
    let value: String = body.value.chars().take(64).collect();
    let (before, after) = spec::set(&state.pool, admin.wallet(), None, &key, &value).await?;
    if SWITCHES.contains(&key.as_str()) {
        tracing::warn!(actor = %admin.wallet(), key = %key, value = %after, "admin_kill_switch");
    }
    Ok(Json(
        serde_json::json!({ "key": key, "before": before, "after": after }),
    ))
}

#[derive(Debug, Serialize)]
pub struct CatalogRow {
    pub id: String,
    pub kind: String,
    pub tier: i16,
    pub enabled: bool,
    pub exclusive: bool,
    pub price_usd_cents: Option<i32>,
    pub price_credits: Option<i64>,
    pub meta: Value,
}

/// `GET /api/catalog`: the whole item table, one row per item.
pub async fn catalog(
    State(state): State<AdminState>,
    _admin: ApiAdmin,
) -> Result<Json<Vec<CatalogRow>>, ApiError> {
    let rows = sqlx::query(
        "SELECT id, kind, coalesce(tier, 1)::smallint AS tier, enabled, exclusive, price_usd_cents, price_credits, meta
         FROM cf.items ORDER BY kind, tier, id",
    )
    .fetch_all(&state.pool)
    .await?;
    let mut out = Vec::with_capacity(rows.len());
    for r in &rows {
        out.push(CatalogRow {
            id: r.try_get("id")?,
            kind: r.try_get("kind")?,
            tier: r.try_get("tier")?,
            enabled: r.try_get("enabled")?,
            exclusive: r.try_get("exclusive")?,
            price_usd_cents: r.try_get("price_usd_cents")?,
            price_credits: r.try_get("price_credits")?,
            meta: r.try_get("meta")?,
        });
    }
    Ok(Json(out))
}

/// What the admin may change on an item; anything absent stays as it is. For the
/// nullable fields a present `null` means "clear it", which serde only tells apart from
/// "absent" through `double_option`.
#[derive(Debug, Deserialize)]
pub struct ItemPatch {
    #[serde(default, deserialize_with = "double_option")]
    pub price_credits: Option<Option<i64>>,
    #[serde(default, deserialize_with = "double_option")]
    pub price_usd_cents: Option<Option<i32>>,
    pub enabled: Option<bool>,
    pub exclusive: Option<bool>,
    pub tier: Option<i16>,
    /// `null` removes the level gate.
    #[serde(default, deserialize_with = "double_option")]
    pub level_min: Option<Option<i32>>,
}

fn double_option<'de, T, D>(d: D) -> Result<Option<Option<T>>, D::Error>
where
    T: Deserialize<'de>,
    D: serde::Deserializer<'de>,
{
    Deserialize::deserialize(d).map(Some)
}

const MAX_CREDITS: i64 = 10_000_000;
const MAX_USD_CENTS: i32 = 100_000;
/// Kinds the shop may sell. Produce, dishes and materials are earned, never bought: a
/// price on them would open a buy → sell loop that mints credits.
const SELLABLE_KINDS: [&str; 5] = ["seed", "fertilizer", "decor", "misc", "tool"];

fn check_patch(p: &ItemPatch) -> Result<(), ApiError> {
    let bad = |what: &str| ApiError(AdminError::BadRequest(what.to_owned()));
    if let Some(Some(c)) = p.price_credits {
        if !(0..=MAX_CREDITS).contains(&c) {
            return Err(bad("price_credits: 0 … 10 000 000"));
        }
    }
    if let Some(Some(u)) = p.price_usd_cents {
        if !(1..=MAX_USD_CENTS).contains(&u) {
            return Err(bad("price_usd_cents: 1 … 100 000"));
        }
    }
    if let Some(t) = p.tier {
        if !(1..=4).contains(&t) {
            return Err(bad("tier: 1 … 4"));
        }
    }
    if let Some(Some(l)) = p.level_min {
        if !(2..=60).contains(&l) {
            return Err(bad("level_min: 2 … 60 або null"));
        }
    }
    Ok(())
}

/// Invariants a price change must keep (docs/ECONOMY.md): only shop kinds carry a price,
/// and a credit price never drops to or below what the market pays for the same item.
fn check_economy(before: &Value, p: &ItemPatch) -> Result<(), ApiError> {
    let bad = |what: &str| ApiError(AdminError::BadRequest(what.to_owned()));
    let kind = before["kind"].as_str().unwrap_or("");
    let sets_price = matches!(p.price_credits, Some(Some(_))) || matches!(p.price_usd_cents, Some(Some(_)));
    if sets_price && !SELLABLE_KINDS.contains(&kind) {
        return Err(bad(
            "цей тип предмета не продається: урожай, страви й матеріали лише здобуваються",
        ));
    }
    if let (Some(Some(credits)), Some(sell)) = (p.price_credits, before["meta"]["sell_credits"].as_i64()) {
        if credits <= sell {
            return Err(bad("ціна в кредитах має бути вищою за ціну продажу на ринку"));
        }
    }
    Ok(())
}

/// `POST /api/catalog/{id}`: the item is locked, changed and audited with before/after in
/// one transaction. cf-api reads the catalog per request, so the change is live at once.
pub async fn update_item(
    State(state): State<AdminState>,
    admin: ApiAdmin,
    headers: HeaderMap,
    Path(id): Path<String>,
    Json(patch): Json<ItemPatch>,
) -> Result<Json<Value>, ApiError> {
    admin.guard(&state, &headers).await?;
    check_patch(&patch)?;
    let id: String = id.chars().take(64).collect();
    let mut tx = state.pool.begin().await?;
    let before: Value = sqlx::query_scalar(
        "SELECT to_jsonb(t) FROM (SELECT kind, price_credits, price_usd_cents, enabled, exclusive, tier, meta FROM cf.items WHERE id = $1 FOR UPDATE) t",
    )
    .bind(&id)
    .fetch_optional(&mut *tx)
    .await?
    .ok_or(ApiError(AdminError::NotFound))?;
    check_economy(&before, &patch)?;
    let meta_patch = match patch.level_min {
        Some(Some(l)) => serde_json::json!({ "level_min": l }),
        _ => serde_json::json!({}),
    };
    let drop_level = matches!(patch.level_min, Some(None));
    let after: Value = sqlx::query_scalar(
        "UPDATE cf.items SET
            price_credits   = CASE WHEN $2 THEN $3 ELSE price_credits END,
            price_usd_cents = CASE WHEN $4 THEN $5 ELSE price_usd_cents END,
            enabled         = coalesce($6, enabled),
            exclusive       = coalesce($7, exclusive),
            tier            = coalesce($8, tier),
            meta            = (CASE WHEN $9 THEN meta - 'level_min' ELSE meta END) || $10
         WHERE id = $1
         RETURNING jsonb_build_object('price_credits', price_credits, 'price_usd_cents', price_usd_cents,
                                      'enabled', enabled, 'exclusive', exclusive, 'tier', tier, 'meta', meta)",
    )
    .bind(&id)
    .bind(patch.price_credits.is_some())
    .bind(patch.price_credits.flatten())
    .bind(patch.price_usd_cents.is_some())
    .bind(patch.price_usd_cents.flatten())
    .bind(patch.enabled)
    .bind(patch.exclusive)
    .bind(patch.tier)
    .bind(drop_level)
    .bind(&meta_patch)
    .fetch_one(&mut *tx)
    .await?;
    audit::record(
        &mut tx,
        admin.wallet(),
        "catalog_update",
        Some(&id),
        before.clone(),
        after.clone(),
        None,
    )
    .await?;
    tx.commit().await?;
    tracing::warn!(actor = %admin.wallet(), item = %id, "admin_catalog_update");
    Ok(Json(
        serde_json::json!({ "id": id, "before": before, "after": after }),
    ))
}
