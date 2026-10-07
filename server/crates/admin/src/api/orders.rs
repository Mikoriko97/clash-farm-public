//! Payments: the review queue and the other order lists, manual approve/reject through
//! `cf_payments::review` (the real fulfilment path, so nothing pays out twice). Every
//! decision writes an attempt row and an outcome row to `audit_log`.

use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::Json;
use cf_payments::review::{self, ReviewOrder};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::api::{ApiAdmin, ApiError};
use crate::audit;
use crate::error::AdminError;
use crate::state::AdminState;

#[derive(Debug, Deserialize, Default)]
pub struct ListQuery {
    pub status: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct OrderRow {
    pub id: Uuid,
    pub user_id: Uuid,
    pub wallet: String,
    pub item_id: String,
    pub qty: i32,
    pub price_usd_cents: i32,
    pub amount_clash: i64,
    pub status: String,
    pub reason_code: Option<String>,
    pub signature: Option<String>,
    #[serde(with = "time::serde::rfc3339")]
    pub created_at: OffsetDateTime,
    #[serde(with = "time::serde::rfc3339")]
    pub updated_at: OffsetDateTime,
}

impl From<ReviewOrder> for OrderRow {
    fn from(o: ReviewOrder) -> Self {
        Self {
            id: o.id,
            user_id: o.user_id,
            wallet: o.wallet,
            item_id: o.item_id,
            qty: o.qty,
            price_usd_cents: o.price_usd_cents,
            amount_clash: o.amount_clash,
            status: o.status,
            reason_code: o.reason_code,
            signature: o.signature,
            created_at: o.created_at,
            updated_at: o.updated_at,
        }
    }
}

/// `GET /api/orders?status=` (default `needs_review`) → `{ status, statuses, rows }`.
pub async fn list(
    State(state): State<AdminState>,
    _admin: ApiAdmin,
    Query(q): Query<ListQuery>,
) -> Result<Json<Value>, ApiError> {
    let status = q.status.as_deref().unwrap_or(review::NEEDS_REVIEW);
    if !review::LISTABLE.contains(&status) {
        return Err(ApiError(AdminError::BadRequest("unknown status".to_owned())));
    }
    let rows: Vec<OrderRow> = review::list(&state.pool, status, 200)
        .await?
        .into_iter()
        .map(OrderRow::from)
        .collect();
    Ok(Json(
        serde_json::json!({ "status": status, "statuses": review::LISTABLE, "rows": rows }),
    ))
}

/// `POST /api/orders/{id}/approve` → `{ signature, amount_clash }`.
pub async fn approve(
    State(state): State<AdminState>,
    admin: ApiAdmin,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
) -> Result<Json<Value>, ApiError> {
    admin.guard(&state, &headers).await?;
    let target = id.to_string();
    let mut conn = state.pool.acquire().await?;
    audit::record(
        &mut conn,
        admin.wallet(),
        "order_approve_attempt",
        Some(&target),
        serde_json::json!({ "status": review::NEEDS_REVIEW }),
        Value::Null,
        None,
    )
    .await?;
    let outcome = review::approve(&state.pool, id).await;
    let (action, after) = match &outcome {
        Ok((signature, amount)) => (
            "order_approve",
            serde_json::json!({ "status": "fulfilled", "signature": signature, "amount_clash": amount }),
        ),
        Err(err) => (
            "order_approve_failed",
            serde_json::json!({ "error": err.to_string() }),
        ),
    };
    audit::record(
        &mut conn,
        admin.wallet(),
        action,
        Some(&target),
        serde_json::json!({ "status": review::NEEDS_REVIEW }),
        after,
        None,
    )
    .await?;
    let (signature, amount) = outcome?;
    tracing::warn!(actor = %admin.wallet(), order_id = %id, amount, "admin_order_approved");
    Ok(Json(
        serde_json::json!({ "signature": signature, "amount_clash": amount }),
    ))
}

#[derive(Debug, Deserialize)]
pub struct RejectBody {
    pub reason: String,
}

/// `POST /api/orders/{id}/reject { reason }`.
pub async fn reject(
    State(state): State<AdminState>,
    admin: ApiAdmin,
    headers: HeaderMap,
    Path(id): Path<Uuid>,
    Json(body): Json<RejectBody>,
) -> Result<Json<Value>, ApiError> {
    admin.guard(&state, &headers).await?;
    let reason: String = body.reason.trim().chars().take(100).collect();
    if reason.is_empty() {
        return Err(ApiError(AdminError::BadRequest("причина обовʼязкова".to_owned())));
    }
    let target = id.to_string();
    // The attempt is on record before the order changes: a lost outcome row still leaves a trace.
    let mut conn = state.pool.acquire().await?;
    audit::record(
        &mut conn,
        admin.wallet(),
        "order_reject_attempt",
        Some(&target),
        serde_json::json!({ "status": review::NEEDS_REVIEW }),
        serde_json::json!({ "reason": reason }),
        None,
    )
    .await?;
    let outcome = review::reject(&state.pool, id, &reason).await;
    let (action, after) = match &outcome {
        Ok(()) => (
            "order_reject",
            serde_json::json!({ "status": "rejected", "reason": reason }),
        ),
        Err(err) => (
            "order_reject_failed",
            serde_json::json!({ "error": err.to_string() }),
        ),
    };
    audit::record(
        &mut conn,
        admin.wallet(),
        action,
        Some(&target),
        serde_json::json!({ "status": review::NEEDS_REVIEW }),
        after,
        None,
    )
    .await?;
    outcome?;
    tracing::warn!(actor = %admin.wallet(), order_id = %id, "admin_order_rejected");
    Ok(Json(
        serde_json::json!({ "status": "rejected", "reason": reason }),
    ))
}
