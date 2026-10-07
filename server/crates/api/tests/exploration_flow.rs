//! Exploration and expeditions (migrations 0014/0015, docs/MAINLAND.md §4):
//! the fog mask follows the presence position, landmarks stay hidden until
//! their chunk is explored, a discovery pays once and only up close,
//! expeditions roll per day, count gathers/explores/discoveries and pay once.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::net::SocketAddr;
use std::time::Duration;

use axum::http::StatusCode;
use common::{app, get, login, post, Session};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};
use uuid::Uuid;

type Ws = WebSocketStream<MaybeTlsStream<TcpStream>>;

async fn serve(app: axum::Router) -> SocketAddr {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    addr
}

/// Seeds the stored position (a reconnecting client) and opens a socket there.
async fn stand_at(addr: SocketAddr, session: &Session, zone: &str, x_dm: i32, z_dm: i32) -> Ws {
    let redis_url =
        std::env::var("CF_TEST_REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379/1".to_owned());
    let client = redis::Client::open(redis_url).unwrap();
    let mut con = client.get_multiplexed_async_connection().await.unwrap();
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let _: () = redis::cmd("SET")
        .arg(format!("presence:pos:{}", session.user_id))
        .arg(format!("{zone}:{x_dm}:{z_dm}:{now_ms}"))
        .arg("EX")
        .arg(120)
        .query_async(&mut con)
        .await
        .unwrap();
    let mut request = format!("ws://{addr}/api/v1/ws/presence")
        .into_client_request()
        .unwrap();
    request
        .headers_mut()
        .insert("origin", "http://localhost:5173".parse().unwrap());
    request
        .headers_mut()
        .insert("cookie", session.cookie().parse().unwrap());
    let (mut ws, _) = tokio_tungstenite::connect_async(request).await.unwrap();
    ws.next().await.unwrap().unwrap();
    let text = json!({ "t": "pos", "zone": zone, "x_dm": x_dm, "z_dm": z_dm, "yaw_deg": 0, "anim": "idle" })
        .to_string();
    ws.send(Message::Text(text.into())).await.unwrap();
    tokio::time::sleep(Duration::from_millis(150)).await;
    ws
}

fn landmark<'a>(view: &'a Value, id: &str) -> Option<&'a Value> {
    view["landmarks"]
        .as_array()
        .unwrap()
        .iter()
        .find(|l| l["id"] == id)
}

fn qty(inv: &Value, item: &str) -> i64 {
    inv["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|e| e["item_id"] == item)
        .map(|e| e["qty"].as_i64().unwrap())
        .unwrap_or(0)
}

#[tokio::test]
async fn test_explore_lifts_fog_from_presence_and_reveals_landmarks() {
    let (app, _) = app().await;
    let addr = serve(app.clone()).await;
    let session = login(&app).await;
    get(&app, &session, "/island").await;
    let (status, view) = get(&app, &session, "/mainland/explored").await;
    assert_eq!(status, StatusCode::OK, "{view}");
    assert_eq!(view["explored"], 0);
    assert_eq!(view["total_landmarks"], 12);
    assert!(
        view["landmarks"].as_array().unwrap().is_empty(),
        "nothing shown in the fog"
    );
    // Without presence, exploring is refused.
    let (status, body) = post(&app, &session, "/mainland/explore", None, None).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["error"]["code"], "too_far");
    // Standing at the dock: its chunk (8, 14) and neighbours open, the dock landmark shows.
    let _ws = stand_at(addr, &session, "mainland", 0, 4000).await;
    let (status, body) = post(&app, &session, "/mainland/explore", None, None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body["revealed"].as_array().unwrap().len(),
        9,
        "the 3 × 3 around the dock"
    );
    assert_eq!(body["view"]["explored"], 9);
    assert!(landmark(&body["view"], "dock").is_some_and(|l| l["discovered"] == false));
    assert!(
        landmark(&body["view"], "lighthouse").is_none(),
        "the far lighthouse stays hidden"
    );
    // Same spot again: nothing new.
    let (status, body) = post(&app, &session, "/mainland/explore", None, None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert!(body["revealed"].as_array().unwrap().is_empty());
    let (_, view) = get(&app, &session, "/mainland/explored").await;
    assert_eq!(view["mask_hex"].as_str().unwrap().len(), 64);
    assert_eq!(view["explored"], 9);
}

#[tokio::test]
async fn test_discover_pays_once_and_only_up_close() {
    let (app, pool) = app().await;
    let addr = serve(app.clone()).await;
    let session = login(&app).await;
    get(&app, &session, "/island").await;
    let body = json!({ "landmark_id": "old_mill" });
    // 40 m away.
    let _far = stand_at(addr, &session, "mainland", -3800 + 400, 600).await;
    let (status, resp) = post(&app, &session, "/mainland/discover", Some(body.clone()), None).await;
    assert_eq!(status, StatusCode::CONFLICT, "{resp}");
    assert_eq!(resp["error"]["code"], "too_far");
    // Next to the mill.
    sqlx::query("DELETE FROM cf.discoveries WHERE user_id = $1")
        .bind(Uuid::parse_str(&session.user_id).unwrap())
        .execute(&pool)
        .await
        .unwrap();
    let _near = stand_at(addr, &session, "mainland", -3800 + 50, 600 - 30).await;
    let (status, resp) = post(&app, &session, "/mainland/discover", Some(body.clone()), None).await;
    assert_eq!(status, StatusCode::OK, "{resp}");
    assert_eq!(resp["landmark"]["id"], "old_mill");
    assert_eq!(resp["landmark"]["discovered"], true);
    assert_eq!(resp["reward"][0]["item_id"], "herb");
    assert_eq!(resp["reward"][0]["qty"], 3);
    assert!(resp["xp"].as_i64().unwrap() >= 30);
    let (_, inv) = get(&app, &session, "/inventory").await;
    assert_eq!(qty(&inv, "herb"), 3);
    // Second visit: no second reward.
    let (status, resp) = post(&app, &session, "/mainland/discover", Some(body.clone()), None).await;
    assert_eq!(status, StatusCode::CONFLICT, "{resp}");
    assert_eq!(resp["error"]["code"], "already_discovered");
    let (_, inv) = get(&app, &session, "/inventory").await;
    assert_eq!(qty(&inv, "herb"), 3);
    // Discovered landmarks show even with the fog still down.
    let (_, view) = get(&app, &session, "/mainland/explored").await;
    assert!(landmark(&view, "old_mill").is_some_and(|l| l["discovered"] == true));
    // Unknown and malformed ids are 404.
    let (status, _) = post(
        &app,
        &session,
        "/mainland/discover",
        Some(json!({ "landmark_id": "atlantis" })),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = post(
        &app,
        &session,
        "/mainland/discover",
        Some(json!({ "landmark_id": "Old Mill; --" })),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_parallel_discover_and_claim_pay_once() {
    let (app, pool) = app().await;
    let addr = serve(app.clone()).await;
    let session = login(&app).await;
    get(&app, &session, "/island").await;
    let user_id = Uuid::parse_str(&session.user_id).unwrap();
    let _ws = stand_at(addr, &session, "mainland", -2200 + 30, 3900).await;
    let body = json!({ "landmark_id": "shipwreck" });
    let (a, b) = tokio::join!(
        post(&app, &session, "/mainland/discover", Some(body.clone()), None),
        post(&app, &session, "/mainland/discover", Some(body.clone()), None)
    );
    let ok = [a.0, b.0].iter().filter(|s| **s == StatusCode::OK).count();
    assert_eq!(ok, 1, "one of two parallel discoveries pays: {} {}", a.1, b.1);
    let (_, inv) = get(&app, &session, "/inventory").await;
    assert_eq!(qty(&inv, "reed"), 3, "reward paid once");
    // A finished expedition claimed twice at once pays once.
    sqlx::query(
        "UPDATE cf.expeditions SET tasks = '[{\"kind\":\"discover\",\"target\":\"\",\"count\":1,\"xp\":50},{\"kind\":\"explore\",\"target\":\"\",\"count\":4,\"xp\":30},{\"kind\":\"gather\",\"target\":\"herb\",\"count\":2,\"xp\":25}]'::jsonb,
             done = '{\"0\": {\"progress\": 1, \"claimed\": false}}'::jsonb
         WHERE user_id = $1",
    )
    .bind(user_id)
    .execute(&pool)
    .await
    .unwrap();
    let (_, before) = get(&app, &session, "/island").await;
    let xp_before = before["island"]["xp"].as_i64().unwrap();
    let (a, b) = tokio::join!(
        post(&app, &session, "/mainland/expeditions/0/claim", None, None),
        post(&app, &session, "/mainland/expeditions/0/claim", None, None)
    );
    let ok = [a.0, b.0].iter().filter(|s| **s == StatusCode::OK).count();
    assert_eq!(ok, 1, "one of two parallel claims pays: {} {}", a.1, b.1);
    let (_, after) = get(&app, &session, "/island").await;
    assert_eq!(after["island"]["xp"].as_i64().unwrap(), xp_before + 50);
}

#[tokio::test]
async fn test_expeditions_roll_count_and_pay_once() {
    let (app, pool) = app().await;
    let addr = serve(app.clone()).await;
    let session = login(&app).await;
    get(&app, &session, "/island").await;
    let user_id = Uuid::parse_str(&session.user_id).unwrap();
    let (status, today) = get(&app, &session, "/mainland/expeditions").await;
    assert_eq!(status, StatusCode::OK, "{today}");
    let tasks = today["tasks"].as_array().unwrap();
    assert_eq!(tasks.len(), 3);
    assert!(tasks.iter().all(|t| t["progress"] == 0 && t["claimed"] == false));
    let (_, again) = get(&app, &session, "/mainland/expeditions").await;
    assert_eq!(again["tasks"], today["tasks"], "the day's roll is stable");
    // Not done yet: nothing to claim; a task index past the day: 404.
    let (status, body) = post(&app, &session, "/mainland/expeditions/0/claim", None, None).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["error"]["code"], "not_ready");
    let (status, _) = post(&app, &session, "/mainland/expeditions/3/claim", None, None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    // Force a known task set: one explore task of 4 chunks at index 0.
    sqlx::query(
        "UPDATE cf.expeditions SET tasks = '[{\"kind\":\"explore\",\"target\":\"\",\"count\":4,\"xp\":30},{\"kind\":\"gather\",\"target\":\"herb\",\"count\":2,\"xp\":25},{\"kind\":\"discover\",\"target\":\"\",\"count\":1,\"xp\":50}]'::jsonb
         WHERE user_id = $1",
    )
    .bind(user_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("DELETE FROM cf.explored WHERE user_id = $1")
        .bind(user_id)
        .execute(&pool)
        .await
        .unwrap();
    // Exploring the dock reveals 6 chunks → the explore task (4) is complete.
    let _ws = stand_at(addr, &session, "mainland", 0, 4000).await;
    let (status, body) = post(&app, &session, "/mainland/explore", None, None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (_, today) = get(&app, &session, "/mainland/expeditions").await;
    assert_eq!(today["tasks"][0]["progress"], 4, "capped at the task count");
    assert_eq!(today["tasks"][1]["progress"], 0);
    let (status, body) = post(&app, &session, "/mainland/expeditions/0/claim", None, None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["task"]["claimed"], true);
    assert!(body["xp"].as_i64().unwrap() >= 30);
    let (status, body) = post(&app, &session, "/mainland/expeditions/0/claim", None, None).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["error"]["code"], "already_claimed");
    // Discovering the dock counts toward the discover task.
    let (status, body) = post(
        &app,
        &session,
        "/mainland/discover",
        Some(json!({ "landmark_id": "dock" })),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let (_, today) = get(&app, &session, "/mainland/expeditions").await;
    assert_eq!(today["tasks"][2]["progress"], 1);
    let (status, _) = post(&app, &session, "/mainland/expeditions/2/claim", None, None).await;
    assert_eq!(status, StatusCode::OK);
}
