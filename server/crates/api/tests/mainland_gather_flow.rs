//! Mainland gathering (migration 0012, docs/MAINLAND.md §5): a hit needs the
//! character next to the node according to presence, nodes are shared and
//! deplete for everyone, caps apply, the window shows live state, two
//! parallel last hits have one winner. Local Postgres + Memurai, real TCP for
//! the presence socket.
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

/// Opens the presence socket and parks the character at (x, z) in `zone`. A socket
/// may only start at the zone's entry or where a recent socket left off, so the
/// test plays a client that reconnects mid-walk: it seeds the stored position first.
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
    let hello = ws.next().await.unwrap().unwrap();
    assert!(matches!(hello, Message::Text(_)));
    let text = json!({ "t": "pos", "zone": zone, "x_dm": x_dm, "z_dm": z_dm, "yaw_deg": 0, "anim": "idle" })
        .to_string();
    ws.send(Message::Text(text.into())).await.unwrap();
    // The server stores the position asynchronously; give it a beat.
    tokio::time::sleep(Duration::from_millis(150)).await;
    ws
}

/// Node `i` of chunk (4, 3): pines in the forest. Tests in this binary run in
/// parallel, so each one takes its own node and resets its state first.
/// The i-th pine of forest chunk (4, 3): the baked table decides the order of kinds,
/// so the tests pick by kind rather than by position in the list.
async fn forest_node(app: &axum::Router, session: &Session, i: usize) -> Value {
    let (status, body) = get(app, session, "/mainland/chunks?cx=4&cz=3&r=0").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let pines: Vec<Value> = body["chunks"][0]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|n| n["kind"] == "tree_pine")
        .cloned()
        .collect();
    assert!(pines.len() > i, "forest chunk (4, 3) has pines: {body}");
    pines[i].clone()
}

async fn reset_node(pool: &sqlx::PgPool, id: &str) {
    sqlx::query("DELETE FROM cf.mainland_nodes WHERE id = $1")
        .bind(id)
        .execute(pool)
        .await
        .unwrap();
}

async fn gather(app: &axum::Router, session: &Session, id: &str) -> (StatusCode, Value) {
    post(app, session, &format!("/mainland/nodes/{id}/gather"), None, None).await
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
async fn test_gather_needs_presence_next_to_the_node() {
    let (app, pool) = app().await;
    let addr = serve(app.clone()).await;
    let session = login(&app).await;
    get(&app, &session, "/island").await;
    let node = forest_node(&app, &session, 1).await;
    let id = node["id"].as_str().unwrap().to_owned();
    reset_node(&pool, &id).await;
    let (x, z) = home(&id);

    // No socket at all.
    let (status, body) = gather(&app, &session, &id).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["error"]["code"], "too_far");
    // Standing on the island.
    let _island = stand_at(addr, &session, "island", 0, 0).await;
    let (status, _) = gather(&app, &session, &id).await;
    assert_eq!(status, StatusCode::CONFLICT);
    // 20 m away on the mainland (a newer socket replaces the older one).
    let _far = stand_at(addr, &session, "mainland", x + 200, z).await;
    let (status, _) = gather(&app, &session, &id).await;
    assert_eq!(status, StatusCode::CONFLICT);
    // Next to it: wood.
    let _near = stand_at(addr, &session, "mainland", x + 20, z - 10).await;
    let (status, body) = gather(&app, &session, &id).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["item_id"], "wood_pine");
    assert_eq!(body["qty"], 1);
    assert_eq!(body["node"]["hits_left"], 4);
    assert_eq!(body["node"]["kind"], "tree_pine");
    assert!(body["xp"].as_i64().unwrap() >= 2);
    // Unknown node ids are 404, not a hint.
    let (status, _) = gather(&app, &session, "4_3_99").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = gather(&app, &session, "7_8_0").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn test_shared_node_depletes_for_everyone_and_window_shows_it() {
    let (app, pool) = app().await;
    let addr = serve(app.clone()).await;
    let a = login(&app).await;
    let b = login(&app).await;
    get(&app, &a, "/island").await;
    get(&app, &b, "/island").await;
    let node = forest_node(&app, &a, 0).await;
    let id = node["id"].as_str().unwrap().to_owned();
    reset_node(&pool, &id).await;
    let (x, z) = home(&id);
    let _wa = stand_at(addr, &a, "mainland", x, z + 15).await;
    let _wb = stand_at(addr, &b, "mainland", x - 15, z).await;
    for expected in [4, 3, 2] {
        let (status, body) = gather(&app, &a, &id).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["node"]["hits_left"], expected);
    }
    for expected in [1, 0] {
        let (status, body) = gather(&app, &b, &id).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        assert_eq!(body["node"]["hits_left"], expected);
        assert_eq!(body["node"]["respawn_at"].is_null(), expected > 0);
    }
    // Used up, the node has moved on to its next spot (0024): standing at the old one is too far,
    // standing at the new one is a plain "depleted".
    let (status, body) = gather(&app, &a, &id).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    let next = cf_world::mainland::node_spec(&id).unwrap().at(1);
    let _wa = stand_at(addr, &a, "mainland", next.0, next.1 + 15).await;
    let (status, body) = gather(&app, &a, &id).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["error"]["code"], "depleted");
    let (_, inv_a) = get(&app, &a, "/inventory").await;
    let (_, inv_b) = get(&app, &b, "/inventory").await;
    assert_eq!(qty(&inv_a, "wood_pine"), 3);
    assert_eq!(qty(&inv_b, "wood_pine"), 2);
    // The window shows the used-up node; once regrown it is full again.
    let shown = forest_node(&app, &b, 0).await;
    assert_eq!(shown["hits_left"], 0);
    sqlx::query("UPDATE cf.mainland_nodes SET respawn_at = now() - interval '1 second' WHERE id = $1")
        .bind(&id)
        .execute(&pool)
        .await
        .unwrap();
    let back = forest_node(&app, &b, 0).await;
    assert_eq!(back["hits_left"], 5);
    // It returns on another reserved spot (0024), and the hit is measured against that one.
    let (nx, nz) = (
        back["x_dm"].as_i64().unwrap() as i32,
        back["z_dm"].as_i64().unwrap() as i32,
    );
    let _wb = stand_at(addr, &b, "mainland", nx, nz + 15).await;
    let (status, body) = gather(&app, &b, &id).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["node"]["hits_left"], 4);
}

/// A pine of forest chunk (4, 3) that the bake reserved alternate spots for. The first two pines
/// belong to the other tests of this binary (they run in parallel on one database).
fn pine_with_alts() -> cf_world::mainland::NodeSpec {
    cf_world::mainland::chunk_nodes(4, 3)
        .into_iter()
        .filter(|n| n.kind == "tree_pine")
        .skip(2)
        .find(|n| n.alts.len() >= 2)
        .expect("a pine with two reserved alternates in chunk (4, 3)")
}

/// Where the baked node stands before anybody used it up. Tests reset the row first, then read this:
/// the window shows the *current* spot, and an earlier run may have left the node on another one.
fn home(id: &str) -> (i32, i32) {
    let spec = cf_world::mainland::node_spec(id).unwrap();
    (spec.x_dm, spec.z_dm)
}

fn at(node: &Value) -> (i32, i32) {
    (
        node["x_dm"].as_i64().unwrap() as i32,
        node["z_dm"].as_i64().unwrap() as i32,
    )
}

#[tokio::test]
async fn test_used_up_node_returns_on_the_next_reserved_spot() {
    let (app, pool) = app().await;
    let addr = serve(app.clone()).await;
    let s = login(&app).await;
    get(&app, &s, "/island").await;
    let spec = pine_with_alts();
    let id = spec.id.clone();
    reset_node(&pool, &id).await;
    let (_, window) = get(&app, &s, "/mainland/chunks?cx=4&cz=3&r=0").await;
    let node = window["chunks"][0]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == id.as_str())
        .unwrap()
        .clone();
    assert_eq!(at(&node), (spec.x_dm, spec.z_dm), "a fresh node stands at home");
    // Cut it down completely, standing at home.
    let home = stand_at(addr, &s, "mainland", spec.x_dm, spec.z_dm + 10).await;
    let mut last = Value::Null;
    for _ in 0..5 {
        let (status, body) = gather(&app, &s, &id).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        last = body;
    }
    assert_eq!(last["node"]["hits_left"], 0);
    assert_eq!(
        at(&last["node"]),
        spec.at(1),
        "the answer already names the next spot"
    );
    assert_ne!(spec.at(1), (spec.x_dm, spec.z_dm));
    let respawns: i32 = sqlx::query_scalar("SELECT respawns FROM cf.mainland_nodes WHERE id = $1")
        .bind(&id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(respawns, 1, "one depletion, one step");
    // Regrown: the window lists it full, on the second spot.
    sqlx::query("UPDATE cf.mainland_nodes SET respawn_at = now() - interval '1 second' WHERE id = $1")
        .bind(&id)
        .execute(&pool)
        .await
        .unwrap();
    let (_, window) = get(&app, &s, "/mainland/chunks?cx=4&cz=3&r=0").await;
    let shown = window["chunks"][0]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == id.as_str())
        .unwrap()
        .clone();
    assert_eq!((shown["hits_left"].as_i64(), at(&shown)), (Some(5), spec.at(1)));
    // Reach follows the node: the old home is too far, the new spot is fine.
    drop(home);
    let (ax, az) = spec.at(1);
    let _far = stand_at(addr, &s, "mainland", ax + 200, az).await;
    let (status, body) = gather(&app, &s, &id).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["error"]["code"], "too_far");
    let _near = stand_at(addr, &s, "mainland", ax, az + 10).await;
    for _ in 0..5 {
        let (status, body) = gather(&app, &s, &id).await;
        assert_eq!(status, StatusCode::OK, "{body}");
        last = body;
    }
    assert_eq!(
        at(&last["node"]),
        spec.at(2),
        "the second depletion moves it on again"
    );
    assert_ne!(spec.at(2), spec.at(1));
}

#[tokio::test]
async fn test_daily_caps_common_and_rare() {
    let (app, pool) = app().await;
    let addr = serve(app.clone()).await;
    let session = login(&app).await;
    get(&app, &session, "/island").await;
    let user_id = Uuid::parse_str(&session.user_id).unwrap();
    // Rare: the iron vein of a rock chunk (13, 3).
    let (status, body) = get(&app, &session, "/mainland/chunks?cx=13&cz=3&r=0").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let iron = body["chunks"][0]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["kind"] == "ore_iron")
        .unwrap()
        .clone();
    let iron_id = iron["id"].as_str().unwrap().to_owned();
    sqlx::query("DELETE FROM cf.mainland_nodes WHERE id = $1")
        .bind(&iron_id)
        .execute(&pool)
        .await
        .unwrap();
    let (ix, iz) = home(&iron_id);
    let _ws = stand_at(addr, &session, "mainland", ix, iz).await;
    sqlx::query(
        "INSERT INTO cf.rate_events (user_id, action, day, count) VALUES ($1, 'mainland_gather_rare', (now() AT TIME ZONE 'UTC')::date, 20)
         ON CONFLICT (user_id, action, day) DO UPDATE SET count = 20",
    )
    .bind(user_id)
    .execute(&pool)
    .await
    .unwrap();
    let (status, body) = gather(&app, &session, &iron_id).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "{body}");
    assert_eq!(body["error"]["code"], "daily_cap");
    let (_, inv) = get(&app, &session, "/inventory").await;
    assert_eq!(qty(&inv, "ore_iron"), 0, "capped hit did not land");
    // Common cap counts every hit, rare or not.
    sqlx::query(
        "INSERT INTO cf.rate_events (user_id, action, day, count) VALUES ($1, 'mainland_gather', (now() AT TIME ZONE 'UTC')::date, 60)
         ON CONFLICT (user_id, action, day) DO UPDATE SET count = 60",
    )
    .bind(user_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query("UPDATE cf.rate_events SET count = 0 WHERE user_id = $1 AND action = 'mainland_gather_rare'")
        .bind(user_id)
        .execute(&pool)
        .await
        .unwrap();
    let (status, body) = gather(&app, &session, &iron_id).await;
    assert_eq!(status, StatusCode::TOO_MANY_REQUESTS, "{body}");
}

#[tokio::test]
async fn test_parallel_last_hits_have_one_winner() {
    let (app, pool) = app().await;
    let addr = serve(app.clone()).await;
    let a = login(&app).await;
    let b = login(&app).await;
    get(&app, &a, "/island").await;
    get(&app, &b, "/island").await;
    // A herb in the meadow (single hit) in chunk (3, 8).
    let (_, body) = get(&app, &a, "/mainland/chunks?cx=3&cz=8&r=0").await;
    let herb = body["chunks"][0]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["kind"] == "herb")
        .unwrap()
        .clone();
    let id = herb["id"].as_str().unwrap().to_owned();
    sqlx::query("DELETE FROM cf.mainland_nodes WHERE id = $1")
        .bind(&id)
        .execute(&pool)
        .await
        .unwrap();
    let (x, z) = home(&id);
    let _wa = stand_at(addr, &a, "mainland", x + 10, z).await;
    let _wb = stand_at(addr, &b, "mainland", x - 10, z).await;
    let (ra, rb) = tokio::join!(gather(&app, &a, &id), gather(&app, &b, &id));
    let ok = [ra.0, rb.0].iter().filter(|s| **s == StatusCode::OK).count();
    assert_eq!(ok, 1, "exactly one of the two hits lands: {ra:?} {rb:?}");
    let (_, inv_a) = get(&app, &a, "/inventory").await;
    let (_, inv_b) = get(&app, &b, "/inventory").await;
    assert_eq!(qty(&inv_a, "herb") + qty(&inv_b, "herb"), 1);
}
