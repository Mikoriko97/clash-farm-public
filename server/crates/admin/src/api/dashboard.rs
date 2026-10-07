//! `GET /api/dashboard`: the numbers from `metrics.rs` plus the current kill switches.

use axum::extract::State;
use axum::Json;
use serde_json::Value;

use crate::api::{ApiAdmin, ApiError};
use crate::metrics;
use crate::spec::{self, Kind};
use crate::state::AdminState;

pub async fn get(State(state): State<AdminState>, _admin: ApiAdmin) -> Result<Json<Value>, ApiError> {
    let d = metrics::load(&state.pool, &state.redis).await?;
    let switches: Vec<Value> = spec::all(&state.pool)
        .await?
        .into_iter()
        .filter(|c| matches!(c.spec.kind, Kind::Switch))
        .map(|c| {
            let on = c.value.as_ref().and_then(Value::as_bool).unwrap_or(c.spec.key != "maintenance");
            serde_json::json!({ "key": c.spec.key, "on": on, "note": c.spec.note, "updated_by": c.updated_by })
        })
        .collect();
    Ok(Json(serde_json::json!({
        "players": d.players,
        "retention": d.retention,
        "payments": d.payments,
        "credits": d.credits,
        "days": d.days,
        "alerts": d.alerts,
        "ledger_mismatches": d.ledger_mismatches,
        "switches": switches,
    })))
}
