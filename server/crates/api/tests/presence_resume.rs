//! Presence across reconnects and respawns (docs/SECURITY.md §3, ADR-036): the stored position
//! outlives the socket, so a new socket resumes from it instead of being sent back to the zone's
//! entry; a jump onto the zone's respawn point (the game's own fall-off-the-world reset) is
//! accepted, any other jump is still refused. Real TCP + tokio-tungstenite against the app.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::net::SocketAddr;
use std::time::Duration;

use common::{app, login, Session};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

type Ws = WebSocketStream<MaybeTlsStream<TcpStream>>;

const ORIGIN: &str = "http://localhost:5173";

async fn serve(app: axum::Router) -> SocketAddr {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    addr
}

async fn connect(addr: SocketAddr, session: &Session) -> Ws {
    let mut request = format!("ws://{addr}/api/v1/ws/presence")
        .into_client_request()
        .unwrap();
    request.headers_mut().insert("origin", ORIGIN.parse().unwrap());
    request
        .headers_mut()
        .insert("cookie", session.cookie().parse().unwrap());
    let (mut ws, _) = tokio_tungstenite::connect_async(request).await.unwrap();
    assert_eq!(next_json(&mut ws, 2000).await.unwrap()["t"], "hello");
    ws
}

async fn next_json(ws: &mut Ws, timeout_ms: u64) -> Option<Value> {
    loop {
        let msg = tokio::time::timeout(Duration::from_millis(timeout_ms), ws.next())
            .await
            .ok()??
            .ok()?;
        match msg {
            Message::Text(t) => return serde_json::from_str(&t).ok(),
            Message::Close(_) => return None,
            _ => {}
        }
    }
}

async fn send_pos(ws: &mut Ws, zone: &str, x_dm: i32, z_dm: i32) {
    let text = json!({ "t": "pos", "zone": zone, "x_dm": x_dm, "z_dm": z_dm, "yaw_deg": 0, "anim": "idle" })
        .to_string();
    ws.send(Message::Text(text.into())).await.unwrap();
}

/// Reads frames for a moment and fails on any `reset`.
async fn expect_no_reset(ws: &mut Ws) {
    for _ in 0..3 {
        if let Some(v) = next_json(ws, 400).await {
            assert_ne!(v["t"], "reset", "the socket should have resumed: {v}");
        }
    }
}

async fn redis() -> redis::aio::MultiplexedConnection {
    let url = std::env::var("CF_TEST_REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379/1".to_owned());
    redis::Client::open(url)
        .unwrap()
        .get_multiplexed_async_connection()
        .await
        .unwrap()
}

/// `zone:x:z` of the stored position (the timestamp dropped), if any.
async fn stored(session: &Session) -> Option<String> {
    let raw: Option<String> = redis::cmd("GET")
        .arg(format!("presence:pos:{}", session.user_id))
        .query_async(&mut redis().await)
        .await
        .unwrap();
    raw.map(|r| r.rsplit_once(':').unwrap().0.to_owned())
}

/// Seeds the stored position like a client that was just there.
async fn seed(session: &Session, zone: &str, x_dm: i32, z_dm: i32) {
    let now_ms = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_millis();
    let _: () = redis::cmd("SET")
        .arg(format!("presence:pos:{}", session.user_id))
        .arg(format!("{zone}:{x_dm}:{z_dm}:{now_ms}"))
        .arg("EX")
        .arg(120)
        .query_async(&mut redis().await)
        .await
        .unwrap();
}

#[tokio::test]
async fn test_reconnect_far_from_the_dock_resumes_where_it_was() {
    let (app, _) = app().await;
    let addr = serve(app.clone()).await;
    let session = login(&app).await;
    seed(&session, "mainland", 1_000, 1_000).await;
    let mut ws = connect(addr, &session).await;
    send_pos(&mut ws, "mainland", 1_000, 1_000).await;
    expect_no_reset(&mut ws).await;
    // The socket ends (its session check, a network blip, a deploy): the position stays.
    ws.close(None).await.unwrap();
    tokio::time::sleep(Duration::from_millis(600)).await;
    assert_eq!(stored(&session).await.as_deref(), Some("mainland:1000:1000"));
    // The next socket picks up 1 m further on, 300 m from the dock: no reset back to the dock.
    let mut ws = connect(addr, &session).await;
    send_pos(&mut ws, "mainland", 1_010, 1_000).await;
    expect_no_reset(&mut ws).await;
    assert_eq!(stored(&session).await.as_deref(), Some("mainland:1010:1000"));
}

#[tokio::test]
async fn test_a_fall_back_to_the_dock_is_accepted_a_teleport_elsewhere_is_not() {
    let (app, _) = app().await;
    let addr = serve(app.clone()).await;
    let session = login(&app).await;
    seed(&session, "mainland", 1_000, 1_000).await;
    let mut ws = connect(addr, &session).await;
    send_pos(&mut ws, "mainland", 1_000, 1_000).await;
    expect_no_reset(&mut ws).await;
    // 100 m away in a moment, not the dock: refused, the stored position does not move.
    send_pos(&mut ws, "mainland", 0, 1_000).await;
    tokio::time::sleep(Duration::from_millis(600)).await;
    assert_eq!(stored(&session).await.as_deref(), Some("mainland:1000:1000"));
    // The game drops the character back at the dock (WorldBounds): accepted, and walking on from there too.
    send_pos(&mut ws, "mainland", 0, 4_000).await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(stored(&session).await.as_deref(), Some("mainland:0:4000"));
    tokio::time::sleep(Duration::from_millis(500)).await;
    send_pos(&mut ws, "mainland", 15, 4_000).await;
    tokio::time::sleep(Duration::from_millis(100)).await;
    assert_eq!(stored(&session).await.as_deref(), Some("mainland:15:4000"));
    expect_no_reset(&mut ws).await;
}

/// Reads `peers` frames for `ms`; the peer lists seen, in order.
async fn views_for(ws: &mut Ws, ms: u64) -> Vec<usize> {
    let deadline = tokio::time::Instant::now() + Duration::from_millis(ms);
    let mut seen = Vec::new();
    while tokio::time::Instant::now() < deadline {
        if let Some(v) = next_json(ws, 250).await {
            if v["t"] == "peers" {
                seen.push(v["peers"].as_array().map_or(0, Vec::len));
            }
        }
    }
    seen
}

/// Two players near the mine gate on the mainland, each seeing the other.
async fn pair_at_the_gate(addr: SocketAddr, a: &Session, b: &Session) -> (Ws, Ws) {
    seed(a, "mainland", 2_800, -1_900).await;
    seed(b, "mainland", 2_810, -1_900).await;
    let mut wa = connect(addr, a).await;
    let mut wb = connect(addr, b).await;
    send_pos(&mut wa, "mainland", 2_800, -1_900).await;
    send_pos(&mut wb, "mainland", 2_810, -1_900).await;
    assert!(views_for(&mut wb, 1_000).await.contains(&1), "B sees A");
    (wa, wb)
}

#[tokio::test]
async fn test_leaving_a_zone_drops_the_character_there_at_once() {
    let (app, _) = app().await;
    let addr = serve(app.clone()).await;
    let (a, b) = (login(&app).await, login(&app).await);
    let (mut wa, mut wb) = pair_at_the_gate(addr, &a, &b).await;
    // A sails home: island poses are never fanned out, so without a "gone" B would keep a
    // standing ghost for the 5 s staleness. B's view must empty within a second.
    send_pos(&mut wa, "island", -20, 0).await;
    let views = views_for(&mut wb, 1_000).await;
    assert_eq!(
        views.last(),
        Some(&0),
        "A is gone from the mainland at once: {views:?}"
    );
}

#[tokio::test]
async fn test_a_reconnect_does_not_blink_the_character_out() {
    let (app, _) = app().await;
    let addr = serve(app.clone()).await;
    let (a, b) = (login(&app).await, login(&app).await);
    let (_wa, mut wb) = pair_at_the_gate(addr, &a, &b).await;
    // A's page reconnects (a new socket replaces the old one) and carries on from where it stood.
    let mut wa2 = connect(addr, &a).await;
    // A slow page: its first frame comes a moment after the handshake (over a view tick or two).
    tokio::time::sleep(Duration::from_millis(500)).await;
    send_pos(&mut wa2, "mainland", 2_801, -1_900).await;
    let views = views_for(&mut wb, 1_500).await;
    assert!(!views.contains(&0), "B never saw A vanish: {views:?}");
}
