//! The island house: sleeping grants XP once per UTC day and starts «Rested» (0026): +percent XP on
//! actions for a while, applied exactly with a carried remainder; the cap and the status are per
//! account. Against local Postgres + Memurai.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use axum::http::StatusCode;
use common::{app, get, login, post, Session};
use serde_json::{json, Value};
use sqlx::PgPool;

#[tokio::test]
async fn test_sleep_grants_xp_once_a_day() {
    let (app, _) = app().await;
    let session = login(&app).await;
    let (_, before) = get(&app, &session, "/island").await;
    let xp_before = before["island"]["xp"].as_i64().unwrap();
    let (status, body) = post(&app, &session, "/house/sleep", None, None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["xp_gain"], 10, "ECONOMY: SLEEP_XP=10");
    assert_eq!(body["xp"], xp_before + 10);
    assert_eq!(body["level"], 1);
    let (status, body) = post(&app, &session, "/house/sleep", None, None).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "{body}");
    assert_eq!(body["error"]["code"], "daily_cap");
    let (_, after) = get(&app, &session, "/island").await;
    assert_eq!(
        after["island"]["xp"],
        xp_before + 10,
        "the refused sleep changed nothing"
    );
    // Another account has its own budget.
    let other = login(&app).await;
    get(&app, &other, "/island").await;
    let (status, body) = post(&app, &other, "/house/sleep", None, None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
}

#[tokio::test]
async fn test_sleep_needs_a_session() {
    let (app, _) = app().await;
    let session = login(&app).await;
    get(&app, &session, "/island").await;
    let (status, _) = post(&app, &session, "/house/sleep", None, None).await;
    assert_eq!(status, StatusCode::OK);
    let anon = common::Session {
        user_id: session.user_id.clone(),
        wallet: session.wallet.clone(),
        access: String::new(),
        csrf: String::new(),
        ip: session.ip.clone(),
    };
    let (status, _) = post(&app, &anon, "/house/sleep", None, None).await;
    // CSRF is checked before the session, so either refusal is correct.
    assert!(
        matches!(status, StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN),
        "{status}"
    );
}

async fn xp(app: &axum::Router, session: &Session) -> i64 {
    let (_, island) = get(app, session, "/island").await;
    island["island"]["xp"].as_i64().unwrap()
}

/// Plants one wheat seed on plot `idx` (1 XP, `economy.plant_xp`).
async fn plant(app: &axum::Router, session: &Session, idx: usize) {
    let (_, island) = get(app, session, "/island").await;
    let plot_id = island["plots"][idx]["id"].as_str().unwrap().to_owned();
    let (status, body) = post(
        app,
        session,
        &format!("/plots/{plot_id}/plant"),
        Some(json!({ "item_id": "seed_wheat" })),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
}

/// Puts the island into a known «Rested» state: ends in `minutes` (negative = already over), `pct` percent
/// (the test owns the account, no setting is touched).
async fn set_rested(pool: &PgPool, session: &Session, minutes: i32, pct: i16) {
    sqlx::query(
        "UPDATE cf.islands SET rested_until = now() + make_interval(mins => $3), rested_bonus_pct = $2, rested_carry = 0 WHERE user_id = $1",
    )
    .bind(uuid::Uuid::parse_str(&session.user_id).unwrap())
    .bind(pct)
    .bind(minutes)
    .execute(pool)
    .await
    .unwrap();
}

#[tokio::test]
async fn test_sleep_starts_rested_for_three_hours() {
    let (app, _) = app().await;
    let session = login(&app).await;
    let (_, before) = get(&app, &session, "/island").await;
    assert_eq!(before["rested"], Value::Null, "not rested before the first sleep");
    let (status, body) = post(&app, &session, "/house/sleep", None, None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["rested"]["bonus_pct"], 10, "ECONOMY: RESTED_BONUS_PCT=10");
    let (_, after) = get(&app, &session, "/island").await;
    assert_eq!(after["rested"]["bonus_pct"], 10);
    let parse = |v: &Value| {
        time::OffsetDateTime::parse(
            v.as_str().unwrap(),
            &time::format_description::well_known::Rfc3339,
        )
        .unwrap()
    };
    let minutes = (parse(&after["rested"]["until"]) - parse(&after["now"])).whole_minutes();
    assert!(
        (178..=180).contains(&minutes),
        "ECONOMY: RESTED_MINUTES=180, got {minutes}"
    );
}

#[tokio::test]
async fn test_rested_raises_action_xp_exactly_with_a_carried_remainder() {
    let (app, pool) = app().await;
    let session = login(&app).await;
    let start = xp(&app, &session).await;
    set_rested(&pool, &session, 60, 50).await;
    // 50 %: the first +1 XP carries 50/100 (no whole bonus yet), the second completes it (+1 bonus).
    plant(&app, &session, 0).await;
    assert_eq!(
        xp(&app, &session).await,
        start + 1,
        "half an XP is carried, not granted"
    );
    plant(&app, &session, 1).await;
    assert_eq!(
        xp(&app, &session).await,
        start + 1 + 2,
        "the carried half completes: +1 plus +1 bonus"
    );
    // Another account is not rested: the status is per island.
    let other = login(&app).await;
    let other_start = xp(&app, &other).await;
    plant(&app, &other, 0).await;
    assert_eq!(xp(&app, &other).await, other_start + 1);
}

#[tokio::test]
async fn test_rested_ends_and_never_raises_the_sleep_itself() {
    let (app, pool) = app().await;
    let session = login(&app).await;
    let start = xp(&app, &session).await;
    set_rested(&pool, &session, -1, 100).await;
    plant(&app, &session, 0).await;
    assert_eq!(
        xp(&app, &session).await,
        start + 1,
        "an ended status adds nothing"
    );
    let (_, island) = get(&app, &session, "/island").await;
    assert_eq!(island["rested"], Value::Null, "an ended status is not reported");
    // A running status from an earlier sleep does not raise the sleep XP of the house.
    let sleeper = login(&app).await;
    let before = xp(&app, &sleeper).await;
    set_rested(&pool, &sleeper, 60, 100).await;
    let (status, body) = post(&app, &sleeper, "/house/sleep", None, None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["xp_gain"], 10);
    assert_eq!(body["xp"], before + 10, "100 % rested would have doubled it");
}

/// A fresh account with an island, and its user id.
async fn fresh(app: &axum::Router) -> (Session, uuid::Uuid) {
    let session = login(app).await;
    get(app, &session, "/island").await;
    let id = uuid::Uuid::parse_str(&session.user_id).unwrap();
    (session, id)
}

#[tokio::test]
async fn test_rested_is_off_when_a_setting_is_zero() {
    let (app, pool) = app().await;
    for (minutes, pct) in [(0, 10), (180, 0)] {
        let (session, user) = fresh(&app).await;
        let params = cf_world::params::Params {
            rested_minutes: minutes,
            rested_bonus_pct: pct,
            ..Default::default()
        };
        let rest = cf_world::house::sleep(&pool, user, params).await.unwrap();
        assert!(
            rest.rested.is_none(),
            "minutes {minutes}, pct {pct}: no status starts"
        );
        let (_, island) = get(&app, &session, "/island").await;
        assert_eq!(island["rested"], Value::Null);
    }
}

#[tokio::test]
async fn test_rested_settings_are_clamped_even_when_written_around_the_admin() {
    let (app, pool) = app().await;
    let (_, user) = fresh(&app).await;
    let params = cf_world::params::Params {
        rested_minutes: 99_999,
        rested_bonus_pct: 999,
        ..Default::default()
    };
    let rest = cf_world::house::sleep(&pool, user, params)
        .await
        .unwrap()
        .rested
        .unwrap();
    assert_eq!(rest.bonus_pct, 100, "the percent is capped at 100");
    let minutes: i64 = sqlx::query_scalar(
        "SELECT (extract(epoch FROM (rested_until - now())) / 60)::bigint FROM cf.islands WHERE user_id = $1",
    )
    .bind(user)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(
        (1438..=1440).contains(&minutes),
        "the status never lasts longer than a day, got {minutes}"
    );
}

#[tokio::test]
async fn test_rested_bonus_is_exact_when_actions_run_at_the_same_time() {
    let (app, pool) = app().await;
    let session = login(&app).await;
    let (_, island) = get(&app, &session, "/island").await;
    let plots = island["plots"].as_array().unwrap().len().min(4);
    assert!(plots >= 2, "the starter island has plots to plant on");
    let start = island["island"]["xp"].as_i64().unwrap();
    set_rested(&pool, &session, 60, 50).await;
    // Order does not matter: the carried remainder makes the total exactly floor(n * 50 / 100) bonus XP.
    match plots {
        2 => {
            tokio::join!(plant(&app, &session, 0), plant(&app, &session, 1));
        }
        3 => {
            tokio::join!(
                plant(&app, &session, 0),
                plant(&app, &session, 1),
                plant(&app, &session, 2)
            );
        }
        _ => {
            tokio::join!(
                plant(&app, &session, 0),
                plant(&app, &session, 1),
                plant(&app, &session, 2),
                plant(&app, &session, 3)
            );
        }
    }
    let n = i64::try_from(plots).unwrap();
    assert_eq!(
        xp(&app, &session).await,
        start + n + n / 2,
        "{n} plantings at 50 %: no XP lost or invented by the race"
    );
}
