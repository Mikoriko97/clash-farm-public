//! Analytics from `events` (docs/ECONOMY.md §5): the core-loop funnel, what players
//! plant and buy, how many actions of each kind per day, the level ladder, and weekly
//! sign-up cohorts with D1/D7/D30 activity. Read-only, last 30 days unless said otherwise.

use axum::extract::State;
use axum::Json;
use serde_json::Value;

use crate::api::{ApiAdmin, ApiError};
use crate::state::AdminState;

async fn json(pool: &sqlx::PgPool, sql: &'static str) -> Result<Value, ApiError> {
    Ok(sqlx::query_scalar(sql).fetch_one(pool).await?)
}

/// `GET /api/analytics`
pub async fn get(State(state): State<AdminState>, _admin: ApiAdmin) -> Result<Json<Value>, ApiError> {
    let pool = &state.pool;
    // Players who reached each step of the loop at least once in 30 days.
    let funnel = json(pool, "SELECT jsonb_build_object(
            'active',  (SELECT count(DISTINCT user_id) FROM cf.events WHERE created_at > now() - interval '30 days'),
            'plant',   (SELECT count(DISTINCT user_id) FROM cf.events WHERE name = 'plant' AND created_at > now() - interval '30 days'),
            'harvest', (SELECT count(DISTINCT user_id) FROM cf.events WHERE name = 'harvest' AND created_at > now() - interval '30 days'),
            'sell',    (SELECT count(DISTINCT user_id) FROM cf.events WHERE name = 'sell' AND created_at > now() - interval '30 days'),
            'buy',     (SELECT count(DISTINCT user_id) FROM cf.events WHERE name = 'shop_buy' AND created_at > now() - interval '30 days'),
            'craft',   (SELECT count(DISTINCT user_id) FROM cf.events WHERE name = 'craft' AND created_at > now() - interval '30 days'),
            'mainland',(SELECT count(DISTINCT user_id) FROM cf.events WHERE name IN ('mainland_gather', 'expedition_claim', 'discover') AND created_at > now() - interval '30 days'),
            'paid',    (SELECT count(DISTINCT user_id) FROM cf.orders WHERE status = 'fulfilled' AND updated_at > now() - interval '30 days'))").await?;
    let crops = json(pool, "SELECT coalesce(jsonb_agg(to_jsonb(t) ORDER BY plants DESC), '[]'::jsonb) FROM (
            SELECT props ->> 'item' AS item, count(*) AS plants
            FROM cf.events WHERE name = 'plant' AND created_at > now() - interval '30 days' GROUP BY 1 ORDER BY 2 DESC LIMIT 20) t").await?;
    let bought = json(pool, "SELECT coalesce(jsonb_agg(to_jsonb(t) ORDER BY credits DESC), '[]'::jsonb) FROM (
            SELECT props ->> 'item' AS item, sum((props ->> 'qty')::bigint) AS qty, sum((props ->> 'credits')::bigint) AS credits
            FROM cf.events WHERE name = 'shop_buy' AND created_at > now() - interval '30 days' GROUP BY 1 ORDER BY 3 DESC LIMIT 20) t").await?;
    let sold = json(pool, "SELECT coalesce(jsonb_agg(to_jsonb(t) ORDER BY credits DESC), '[]'::jsonb) FROM (
            SELECT props ->> 'item' AS item, sum((props ->> 'qty')::bigint) AS qty, sum((props ->> 'credits')::bigint) AS credits
            FROM cf.events WHERE name = 'sell' AND created_at > now() - interval '30 days' GROUP BY 1 ORDER BY 3 DESC LIMIT 20) t").await?;
    let actions = json(
        pool,
        "SELECT coalesce(jsonb_agg(to_jsonb(t) ORDER BY day, name), '[]'::jsonb) FROM (
            SELECT created_at::date AS day, name, count(*) AS n
            FROM cf.events WHERE created_at > date_trunc('day', now()) - interval '13 days'
              AND name IN ('plant', 'harvest', 'sell', 'shop_buy', 'craft', 'level_up', 'mainland_gather')
            GROUP BY 1, 2) t",
    )
    .await?;
    let levels = json(
        pool,
        "SELECT coalesce(jsonb_agg(to_jsonb(t) ORDER BY level), '[]'::jsonb) FROM (
            SELECT level, count(*) AS players FROM cf.islands GROUP BY level) t",
    )
    .await?;
    // Sign-up weeks (last 8) with the share active on day 1 / 7 / 30 after sign-up.
    let cohorts = json(pool, "SELECT coalesce(jsonb_agg(to_jsonb(t) ORDER BY week), '[]'::jsonb) FROM (
            SELECT date_trunc('week', u.created_at)::date AS week, count(*) AS signed_up,
                   count(*) FILTER (WHERE EXISTS (SELECT 1 FROM cf.events e WHERE e.user_id = u.id
                       AND e.created_at >= u.created_at + interval '1 day' AND e.created_at < u.created_at + interval '2 days')) AS d1,
                   count(*) FILTER (WHERE EXISTS (SELECT 1 FROM cf.events e WHERE e.user_id = u.id
                       AND e.created_at >= u.created_at + interval '7 days' AND e.created_at < u.created_at + interval '8 days')) AS d7,
                   count(*) FILTER (WHERE EXISTS (SELECT 1 FROM cf.events e WHERE e.user_id = u.id
                       AND e.created_at >= u.created_at + interval '30 days' AND e.created_at < u.created_at + interval '31 days')) AS d30,
                   bool_and(u.created_at < now() - interval '8 days') AS d7_complete,
                   bool_and(u.created_at < now() - interval '31 days') AS d30_complete
            FROM cf.users u WHERE u.created_at > now() - interval '8 weeks' GROUP BY 1) t").await?;
    Ok(Json(serde_json::json!({
        "funnel": funnel, "crops": crops, "bought": bought, "sold": sold, "actions": actions, "levels": levels, "cohorts": cohorts,
    })))
}
