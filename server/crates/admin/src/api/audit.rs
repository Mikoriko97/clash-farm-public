//! `audit_log` browsing: newest first, parameterised search over actor/action/target.

use axum::extract::{Query, State};
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use time::OffsetDateTime;

use crate::api::{ApiAdmin, ApiError};
use crate::audit::{self, AuditRow};
use crate::state::AdminState;

#[derive(Debug, Deserialize, Default)]
pub struct AuditQuery {
    pub q: Option<String>,
    pub limit: Option<i64>,
}

#[derive(Debug, Serialize)]
pub struct AuditView {
    pub id: i64,
    pub actor: String,
    pub action: String,
    pub target: Option<String>,
    pub before: Option<Value>,
    pub after: Option<Value>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
}

impl From<AuditRow> for AuditView {
    fn from(r: AuditRow) -> Self {
        Self {
            id: r.id,
            actor: r.actor,
            action: r.action,
            target: r.target,
            before: r.before,
            after: r.after,
            created_at: r.created_at,
        }
    }
}

/// `GET /api/audit?q=&limit=`
pub async fn list(
    State(state): State<AdminState>,
    _admin: ApiAdmin,
    Query(q): Query<AuditQuery>,
) -> Result<Json<Vec<AuditView>>, ApiError> {
    let limit = q.limit.unwrap_or(100).clamp(1, 500);
    let needle: String = q.q.unwrap_or_default().trim().chars().take(64).collect();
    let rows = if needle.is_empty() {
        audit::recent(&state.pool, limit).await?
    } else {
        audit::search(&state.pool, &needle, limit).await?
    };
    Ok(Json(rows.into_iter().map(AuditView::from).collect()))
}
