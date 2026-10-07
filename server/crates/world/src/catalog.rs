//! Item catalog: admin-managed rows in `cf.items`, read-only for the server.

use sqlx::{PgConnection, PgPool, Row};

use crate::error::WorldError;
use crate::types::ItemView;

#[derive(Debug, Clone)]
pub struct Item {
    pub id: String,
    pub kind: String,
    pub enabled: bool,
    pub exclusive: bool,
    pub meta: serde_json::Value,
}

impl Item {
    pub fn meta_i64(&self, key: &str) -> Option<i64> {
        self.meta.get(key).and_then(serde_json::Value::as_i64)
    }

    pub fn meta_str(&self, key: &str) -> Option<String> {
        self.meta
            .get(key)
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
    }
}

pub async fn get(conn: &mut PgConnection, id: &str) -> Result<Option<Item>, WorldError> {
    let row = sqlx::query("SELECT id, kind, enabled, exclusive, meta FROM cf.items WHERE id = $1")
        .bind(id)
        .fetch_optional(conn)
        .await?;
    row.map(|r| {
        Ok(Item {
            id: r.try_get("id")?,
            kind: r.try_get("kind")?,
            enabled: r.try_get("enabled")?,
            exclusive: r.try_get("exclusive")?,
            meta: r.try_get("meta")?,
        })
    })
    .transpose()
}

/// Credit price of an item (`None` when it is not sold for credits).
pub async fn price_credits(conn: &mut PgConnection, id: &str) -> Result<Option<i64>, WorldError> {
    let price: Option<Option<i64>> = sqlx::query_scalar("SELECT price_credits FROM cf.items WHERE id = $1")
        .bind(id)
        .fetch_optional(conn)
        .await?;
    Ok(price.flatten())
}

pub async fn list_enabled(pool: &PgPool) -> Result<Vec<ItemView>, WorldError> {
    let rows = sqlx::query(
        "SELECT id, kind, price_usd_cents, price_credits, exclusive, meta FROM cf.items WHERE enabled ORDER BY kind, id",
    )
    .fetch_all(pool)
    .await?;
    rows.iter()
        .map(|r| {
            Ok(ItemView {
                id: r.try_get("id")?,
                kind: r.try_get("kind")?,
                price_usd_cents: r.try_get("price_usd_cents")?,
                price_credits: r.try_get("price_credits")?,
                exclusive: r.try_get("exclusive")?,
                meta: r.try_get("meta")?,
            })
        })
        .collect()
}
