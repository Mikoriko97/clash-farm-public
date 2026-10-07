//! Mainland (docs/MAINLAND.md, docs/API.md §Mainland): what stands in a window
//! of chunks, and one hit on a shared node. The hit needs the character next
//! to the node according to the presence socket (no socket → no gathering).

use axum::extract::{Path, Query, State};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use axum::Json;
use cf_world::{cave, city, mainland, mainland_gather, storm, weather};
use serde::Deserialize;

use crate::error::ApiError;
use crate::idempotency::{self, IdemKey};
use crate::middleware::session::AuthUser;
use crate::presence;
use crate::routes::world::limit_world;
use crate::state::AppState;

#[derive(Debug, Deserialize)]
pub struct ChunksQuery {
    pub cx: i32,
    pub cz: i32,
    #[serde(default = "default_radius")]
    pub r: i32,
}

fn default_radius() -> i32 {
    mainland::MAX_RADIUS
}

/// `GET /mainland/map` → `{ chunk_m, chunks, origin_m, rows }`: the zone table for the map panel.
pub async fn map(State(_state): State<AppState>, _user: AuthUser) -> Result<Response, ApiError> {
    Ok(Json(mainland::map_view()).into_response())
}

/// `GET /mainland/chunks?cx&cz&r` → `{ chunks: [{ cx, cz, biome, nodes: [...] }] }` with live state.
pub async fn chunks(
    State(state): State<AppState>,
    headers: HeaderMap,
    user: AuthUser,
    Query(q): Query<ChunksQuery>,
) -> Result<Response, ApiError> {
    limit_world(&state, &user, &headers).await?;
    let mut view = mainland::chunks(q.cx, q.cz, q.r)?;
    // Storm-felled trees join the chunks they lie in while the storm window is open.
    let params = state.params.get(&state.pool).await?;
    let now = weather::db_now(&state.pool).await?;
    if let Some(slot) = storm::active_slot(now, params) {
        for tree in storm::fallen_trees(slot) {
            let (cx, cz) = cf_world::exploration::chunk_of(tree.x_dm, tree.z_dm);
            if let Some(chunk) = view.chunks.iter_mut().find(|c| c.cx == cx && c.cz == cz) {
                chunk.nodes.push(tree);
            }
        }
    }
    mainland_gather::overlay(&state.pool, &mut view).await?;
    Ok(Json(view).into_response())
}

/// `GET /city` → `{ buildings: [{ id, kind, name, tagline, partner }] }` — the square as the HUD names it.
pub async fn city_buildings(State(_state): State<AppState>, _user: AuthUser) -> Result<Response, ApiError> {
    Ok(Json(serde_json::json!({ "buildings": city::BUILDINGS })).into_response())
}

/// `GET /cave/{id}` → `{ nodes: [...] }` of a cave (only 1 for now) with live state.
pub async fn cave(
    State(state): State<AppState>,
    headers: HeaderMap,
    user: AuthUser,
    Path(id): Path<i32>,
) -> Result<Response, ApiError> {
    limit_world(&state, &user, &headers).await?;
    if !cave::exists(id) {
        return Err(ApiError::not_found("no such cave"));
    }
    let mut view = mainland::ChunksView {
        chunks: vec![mainland::ChunkView {
            cx: cave::CAVE_CX,
            cz: id,
            biome: "cave".to_owned(),
            nodes: cave::nodes(id),
        }],
    };
    mainland_gather::overlay(&state.pool, &mut view).await?;
    let nodes = view.chunks.pop().map(|c| c.nodes).unwrap_or_default();
    Ok(Json(serde_json::json!({ "nodes": nodes })).into_response())
}

/// `POST /mainland/nodes/{id}/gather` → `{ node, item_id, qty, xp, level }`;
/// `409 too_far` unless presence puts the character within `mainland_reach_dm`.
pub async fn gather(
    State(state): State<AppState>,
    headers: HeaderMap,
    user: AuthUser,
    key: IdemKey,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    limit_world(&state, &user, &headers).await?;
    let params = state.params.get(&state.pool).await?;
    let spec = if id.starts_with("98_") {
        let now = weather::db_now(&state.pool).await?;
        storm::node_spec(&id, storm::active_slot(now, params))
    } else {
        mainland::node_spec(&id)
    }
    .ok_or_else(|| ApiError::not_found("node not found"))?;
    let pool = state.pool.clone();
    let redis = state.redis.clone();
    let scope = format!("mainland_gather:{}", spec.id);
    // The reach check lives inside the idempotent action: a replay of an earlier
    // hit answers with that hit even if the character has walked away since.
    idempotency::run(&state, user.user_id, key, &scope, || async move {
        let pos = position_in(&redis, user.user_id, &spec.zone).await?;
        let reach = i64::from(params.mainland_reach_dm.max(0));
        // A node that was used up stands on another reserved spot (0024): measure to that one.
        let (nx, nz) = mainland_gather::position_of(&pool, &spec).await?;
        if presence::dist2(pos.x_dm, pos.z_dm, nx, nz) > reach * reach {
            return Err(too_far());
        }
        Ok(mainland_gather::gather(&pool, user.user_id, &spec, params).await?)
    })
    .await
}

/// Where presence last saw the character on the mainland, fresh enough to act on;
/// `409 too_far` otherwise (no socket, wrong zone, stale).
pub(crate) async fn mainland_position(
    redis: &redis::aio::ConnectionManager,
    user_id: uuid::Uuid,
) -> Result<presence::LastPosition, ApiError> {
    position_in(redis, user_id, "mainland").await
}

/// Same, for any presence zone (the cave has its own coordinates).
pub(crate) async fn position_in(
    redis: &redis::aio::ConnectionManager,
    user_id: uuid::Uuid,
    zone: &str,
) -> Result<presence::LastPosition, ApiError> {
    match presence::last_position(redis, user_id).await {
        Some(pos) if pos.zone == zone && pos.age_ms <= presence::POSITION_FRESH_MS => Ok(pos),
        _ => Err(too_far()),
    }
}

pub(crate) fn too_far() -> ApiError {
    ApiError::new(
        axum::http::StatusCode::CONFLICT,
        "too_far",
        "stand next to it (presence) first",
    )
}
