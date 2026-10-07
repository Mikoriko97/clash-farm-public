//! Presence WebSocket (docs/MAINLAND.md §8, M1): the cookie session and our Origin
//! open it, two players see each other, teleports and garbage close the socket,
//! leaving removes the peer. Real TCP + tokio-tungstenite against the app.
#![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]

mod common;

use std::net::SocketAddr;
use std::time::Duration;

use common::{app, login, Session};
use futures_util::{SinkExt, StreamExt};
use serde_json::{json, Value};
use tokio::net::TcpStream;
use tokio_tungstenite::tungstenite::client::IntoClientRequest;
use tokio_tungstenite::tungstenite::http::StatusCode;
use tokio_tungstenite::tungstenite::Message;
use tokio_tungstenite::{MaybeTlsStream, WebSocketStream};

type Ws = WebSocketStream<MaybeTlsStream<TcpStream>>;

async fn serve(app: axum::Router) -> SocketAddr {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    addr
}

async fn connect(addr: SocketAddr, cookie: Option<&str>, origin: &str) -> Result<Ws, StatusCode> {
    let mut request = format!("ws://{addr}/api/v1/ws/presence")
        .into_client_request()
        .unwrap();
    request.headers_mut().insert("origin", origin.parse().unwrap());
    if let Some(c) = cookie {
        request.headers_mut().insert("cookie", c.parse().unwrap());
    }
    match tokio_tungstenite::connect_async(request).await {
        Ok((ws, _)) => Ok(ws),
        Err(tokio_tungstenite::tungstenite::Error::Http(resp)) => Err(resp.status()),
        Err(e) => panic!("{e}"),
    }
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

/// Waits until a `peers` frame satisfies `pred` (or gives up after `tries`).
async fn wait_peers(ws: &mut Ws, tries: usize, pred: impl Fn(&Value) -> bool) -> Option<Value> {
    for _ in 0..tries {
        let v = next_json(ws, 1500).await?;
        if v["t"] == "peers" && pred(&v["peers"]) {
            return Some(v["peers"].clone());
        }
    }
    None
}

/// True when no `reset` frame arrives within `ms` (peer frames are read and ignored).
async fn no_reset(ws: &mut Ws, ms: u64) -> bool {
    let deadline = tokio::time::Instant::now() + Duration::from_millis(ms);
    while tokio::time::Instant::now() < deadline {
        if let Some(frame) = next_json(ws, 250).await {
            if frame["t"] == "reset" {
                return false;
            }
        }
    }
    true
}

async fn send_pos(ws: &mut Ws, x_dm: i32, z_dm: i32, anim: &str) {
    let text =
        json!({ "t": "pos", "zone": "island", "x_dm": x_dm, "z_dm": z_dm, "yaw_deg": 90, "anim": anim })
            .to_string();
    ws.send(Message::Text(text.into())).await.unwrap();
}

fn cookie(session: &Session) -> String {
    session.cookie()
}

#[tokio::test]
async fn test_presence_needs_session_and_our_origin() {
    let (app, _) = app().await;
    let addr = serve(app.clone()).await;
    let session = login(&app).await;
    assert_eq!(
        connect(addr, None, "http://localhost:5173").await.err(),
        Some(StatusCode::UNAUTHORIZED)
    );
    assert_eq!(
        connect(addr, Some(&cookie(&session)), "https://evil.example")
            .await
            .err(),
        Some(StatusCode::FORBIDDEN)
    );
    let mut ws = connect(addr, Some(&cookie(&session)), "http://localhost:5173")
        .await
        .expect("handshake");
    let hello = next_json(&mut ws, 2000).await.expect("hello");
    assert_eq!(hello["t"], "hello");
    assert!(hello["nick"].as_str().unwrap().starts_with("Farmer #"));
    assert!(hello["look"].is_null(), "no character yet, no look: {hello}");
    assert!(hello["id"].as_str().unwrap().len() == 12);
}

#[tokio::test]
async fn test_one_socket_per_user_and_handshake_limit() {
    let (app, _) = app().await;
    let addr = serve(app.clone()).await;
    let session = login(&app).await;
    let mut first = connect(addr, Some(&cookie(&session)), "http://localhost:5173")
        .await
        .unwrap();
    assert_eq!(next_json(&mut first, 2000).await.unwrap()["t"], "hello");
    let mut second = connect(addr, Some(&cookie(&session)), "http://localhost:5173")
        .await
        .unwrap();
    assert_eq!(next_json(&mut second, 2000).await.unwrap()["t"], "hello");
    // The older socket is closed by the server, the newer one keeps working.
    assert!(next_json(&mut first, 2000).await.is_none(), "first socket closed");
    send_pos(&mut second, 0, 0, "idle").await;
    assert!(wait_peers(&mut second, 5, |p| p.as_array().is_some())
        .await
        .is_some());
    // 10 handshakes a minute per user; the next one is refused.
    for _ in 2..10 {
        connect(addr, Some(&cookie(&session)), "http://localhost:5173")
            .await
            .expect("within the limit");
    }
    assert_eq!(
        connect(addr, Some(&cookie(&session)), "http://localhost:5173")
            .await
            .err(),
        Some(StatusCode::TOO_MANY_REQUESTS)
    );
}

#[tokio::test]
async fn test_oversized_frame_closes_socket() {
    let (app, _) = app().await;
    let addr = serve(app.clone()).await;
    let session = login(&app).await;
    let mut ws = connect(addr, Some(&cookie(&session)), "http://localhost:5173")
        .await
        .unwrap();
    assert_eq!(next_json(&mut ws, 2000).await.unwrap()["t"], "hello");
    let big = json!({ "t": "pos", "zone": "island", "x_dm": 0, "z_dm": 0, "yaw_deg": 0, "anim": "idle", "pad": "x".repeat(2000) }).to_string();
    let _ = ws.send(Message::Text(big.into())).await;
    assert!(
        next_json(&mut ws, 3000).await.is_none(),
        "socket closed after a 2 KB frame"
    );
}

#[tokio::test]
async fn test_first_frame_must_start_at_the_entry_or_resume_a_recent_position() {
    let (app, _) = app().await;
    let addr = serve(app.clone()).await;
    let a = login(&app).await;
    let b = login(&app).await;
    let mut wb = connect(addr, Some(&cookie(&b)), "http://localhost:5173")
        .await
        .unwrap();
    next_json(&mut wb, 2000).await.unwrap();
    send_pos(&mut wb, 0, 0, "idle").await;
    let mut wa = connect(addr, Some(&cookie(&a)), "http://localhost:5173")
        .await
        .unwrap();
    next_json(&mut wa, 2000).await.unwrap();
    // Far from the island spawn on a fresh socket: refused, and told where the entry is.
    send_pos(&mut wa, 500, 0, "idle").await;
    let reset = next_json(&mut wa, 2000).await.expect("reset frame");
    assert_eq!(reset["t"], "reset");
    assert_eq!(reset["zone"], "island");
    assert_eq!(reset["x_dm"], -20);
    assert!(
        wait_peers(&mut wb, 3, |p| p.as_array().is_some_and(|l| !l.is_empty()))
            .await
            .is_none(),
        "B never sees the refused frame"
    );
    // At the spawn: accepted (no reset comes back; the island itself shows nobody to B).
    send_pos(&mut wa, -20, 0, "idle").await;
    assert!(no_reset(&mut wa, 800).await, "accepted at the spawn: no reset");
    // A reconnects a moment later 2 m further: resumed from the stored position, no reset.
    wa.close(None).await.unwrap();
    tokio::time::sleep(Duration::from_millis(600)).await;
    let mut wa2 = connect(addr, Some(&cookie(&a)), "http://localhost:5173")
        .await
        .unwrap();
    next_json(&mut wa2, 2000).await.unwrap();
    send_pos(&mut wa2, 0, 0, "walk_forward").await;
    assert!(no_reset(&mut wa2, 800).await, "resumed 2 m further: no reset");
    // Jumping zones lands at the other zone's entry, nowhere else.
    send_pos(&mut wa2, 0, 0, "idle").await;
    let text =
        json!({ "t": "pos", "zone": "mainland", "x_dm": 3000, "z_dm": -3000, "yaw_deg": 0, "anim": "idle" })
            .to_string();
    wa2.send(Message::Text(text.into())).await.unwrap();
    let reset = next_json(&mut wa2, 2000).await.expect("reset frame");
    assert_eq!(reset["t"], "reset");
    assert_eq!(reset["zone"], "mainland");
    assert_eq!(reset["z_dm"], 4000);
}

/// Seeds the stored position like a client that was just there.
async fn seed_position(session: &Session, zone: &str, x_dm: i32, z_dm: i32) {
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
}

async fn send_zone_pos(ws: &mut Ws, zone: &str, x_dm: i32, z_dm: i32) {
    let text = json!({ "t": "pos", "zone": zone, "x_dm": x_dm, "z_dm": z_dm, "yaw_deg": 0, "anim": "idle" })
        .to_string();
    ws.send(Message::Text(text.into())).await.unwrap();
}

/// The next `reset` frame, skipping the periodic `peers` frames.
async fn expect_reset(ws: &mut Ws) -> Value {
    for _ in 0..8 {
        let v = next_json(ws, 1500)
            .await
            .expect("a frame before the socket closed");
        if v["t"] == "reset" {
            return v;
        }
    }
    panic!("no reset frame arrived");
}

/// Reads frames for a moment and fails on any `reset`.
async fn expect_no_reset(ws: &mut Ws) {
    for _ in 0..3 {
        if let Some(v) = next_json(ws, 400).await {
            assert_ne!(v["t"], "reset", "the door should have opened: {v}");
        }
    }
}

#[tokio::test]
async fn test_zone_changes_only_through_their_doors() {
    let (app, pool) = app().await;
    let addr = serve(app.clone()).await;
    let session = login(&app).await;
    common::get(&app, &session, "/island").await;
    let user_id = uuid::Uuid::parse_str(&session.user_id).unwrap();
    // From the island, claiming the mine gate is not a way onto the mainland: reset to the dock.
    let mut ws = connect(addr, Some(&cookie(&session)), "http://localhost:5173")
        .await
        .unwrap();
    next_json(&mut ws, 2000).await.unwrap();
    send_zone_pos(&mut ws, "island", -20, 0).await;
    send_zone_pos(&mut ws, "mainland", 2800, -1900).await;
    let reset = expect_reset(&mut ws).await;
    assert_eq!(reset["t"], "reset");
    assert_eq!(reset["zone"], "mainland");
    assert_eq!(reset["z_dm"], 4000);
    // Far from the gate on the mainland, the cave does not open: back to the mainland dock.
    send_zone_pos(&mut ws, "mainland", 0, 4000).await;
    tokio::time::sleep(Duration::from_millis(600)).await;
    send_zone_pos(&mut ws, "cave", 0, 80).await;
    let reset = expect_reset(&mut ws).await;
    assert_eq!(reset["t"], "reset");
    assert_eq!(reset["zone"], "mainland");
    assert_eq!(reset["z_dm"], 4000);
    ws.close(None).await.unwrap();
    // At the gate but never having found it: still no.
    seed_position(&session, "mainland", 2800, -1900).await;
    let mut ws = connect(addr, Some(&cookie(&session)), "http://localhost:5173")
        .await
        .unwrap();
    next_json(&mut ws, 2000).await.unwrap();
    send_zone_pos(&mut ws, "cave", 0, 80).await;
    let reset = expect_reset(&mut ws).await;
    assert_eq!(reset["t"], "reset");
    assert_eq!(reset["zone"], "mainland");
    ws.close(None).await.unwrap();
    // Found the gate and standing at it: the cave opens, and it lets out at the gate.
    sqlx::query(
        "INSERT INTO cf.discoveries (user_id, landmark_id) VALUES ($1, 'mine_gate') ON CONFLICT DO NOTHING",
    )
    .bind(user_id)
    .execute(&pool)
    .await
    .unwrap();
    seed_position(&session, "mainland", 2800, -1900).await;
    let mut ws = connect(addr, Some(&cookie(&session)), "http://localhost:5173")
        .await
        .unwrap();
    next_json(&mut ws, 2000).await.unwrap();
    send_zone_pos(&mut ws, "cave", 0, 80).await;
    let first = next_json(&mut ws, 2000).await.expect("a frame");
    assert_eq!(
        first["t"], "peers",
        "no reset: the cave accepted the entry ({first})"
    );
    send_zone_pos(&mut ws, "mainland", 2800, -1900).await;
    expect_no_reset(&mut ws).await;
    // …but not straight to the dock from inside the cave.
    tokio::time::sleep(Duration::from_millis(600)).await;
    send_zone_pos(&mut ws, "cave", 0, 80).await;
    tokio::time::sleep(Duration::from_millis(600)).await;
    send_zone_pos(&mut ws, "mainland", 0, 4000).await;
    let reset = expect_reset(&mut ws).await;
    assert_eq!(reset["t"], "reset");
    assert_eq!(reset["x_dm"], 2800);
}

/// Every island is its owner's: poses there are accepted (zone doors, reach) but never
/// shown to anyone else, however close the coordinates are.
#[tokio::test]
async fn test_island_is_private_nobody_sees_anybody_there() {
    let (app, _) = app().await;
    let addr = serve(app.clone()).await;
    let a = login(&app).await;
    let b = login(&app).await;
    let mut wa = connect(addr, Some(&cookie(&a)), "http://localhost:5173")
        .await
        .unwrap();
    let mut wb = connect(addr, Some(&cookie(&b)), "http://localhost:5173")
        .await
        .unwrap();
    next_json(&mut wa, 2000).await.unwrap();
    next_json(&mut wb, 2000).await.unwrap();
    send_pos(&mut wa, 100, -50, "walk_forward").await;
    send_pos(&mut wb, 120, -40, "idle").await;
    tokio::time::sleep(Duration::from_millis(700)).await;
    send_pos(&mut wa, 130, -50, "run_forward").await;
    let seen = wait_peers(&mut wb, 6, |p| p.as_array().is_some_and(|l| !l.is_empty())).await;
    assert!(seen.is_none(), "island poses are never fanned out: {seen:?}");
    wa.close(None).await.unwrap();
}

#[tokio::test]
async fn test_two_players_see_each_other_and_leaving_clears() {
    let (app, _) = app().await;
    let addr = serve(app.clone()).await;
    let a = login(&app).await;
    let b = login(&app).await;
    // A has created a character (0025): neighbours see its name and look; B has not.
    let name = format!("Peer{}", &uuid::Uuid::new_v4().simple().to_string()[..8]);
    let look = serde_json::json!({ "skin": 3, "shirt": 5, "pants": 7, "shoes": 1, "hair_color": 4, "hair": 6, "face": 2, "beard": 1, "hat": 3, "glasses": 2, "outfit": 5, "outfit_color": 9, "body": 1, "build": 1 });
    let (status, created) = common::post(
        &app,
        &a,
        "/me/character",
        Some(serde_json::json!({ "nickname": name, "look": look })),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{created}");
    let mut wa = connect(addr, Some(&cookie(&a)), "http://localhost:5173")
        .await
        .unwrap();
    let mut wb = connect(addr, Some(&cookie(&b)), "http://localhost:5173")
        .await
        .unwrap();
    let hello_a = next_json(&mut wa, 2000).await.unwrap();
    assert_eq!(
        (&hello_a["nick"], &hello_a["look"]),
        (&serde_json::json!(name), &look)
    );
    let id_a = hello_a["id"].as_str().unwrap().to_owned();
    next_json(&mut wb, 2000).await.unwrap();
    // The mainland is shared: both arrive at the dock (the only door from nowhere).
    send_zone_pos(&mut wa, "mainland", 100, 3950).await;
    send_zone_pos(&mut wb, "mainland", 120, 3960).await;
    // B must report a position before it gets a view; then A shows up in it.
    let peers = wait_peers(&mut wb, 10, |p| p.as_array().is_some_and(|l| !l.is_empty()))
        .await
        .expect("B sees A");
    let seen = &peers[0];
    assert_eq!(seen["id"], id_a);
    assert_eq!(seen["x_dm"], 100);
    assert_eq!(seen["z_dm"], 3950);
    assert_eq!((&seen["nick"], &seen["look"]), (&serde_json::json!(name), &look));
    assert!(seen.get("wallet").is_none() && seen.get("user_id").is_none());
    // A walks 3 m: at 6 m/s that needs half a second, so wait before reporting it.
    tokio::time::sleep(Duration::from_millis(700)).await;
    send_zone_pos(&mut wa, "mainland", 130, 3950).await;
    let moved = wait_peers(&mut wb, 10, |p| p[0]["x_dm"] == 130).await;
    assert!(moved.is_some(), "B follows A's movement");
    // A leaves: B's view empties.
    wa.close(None).await.unwrap();
    let empty = wait_peers(&mut wb, 15, |p| p.as_array().is_some_and(|l| l.is_empty())).await;
    assert!(empty.is_some(), "peer removed after leaving");
}

#[tokio::test]
async fn test_teleports_and_garbage_close_the_socket() {
    let (app, _) = app().await;
    let addr = serve(app.clone()).await;
    let session = login(&app).await;
    let mut ws = connect(addr, Some(&cookie(&session)), "http://localhost:5173")
        .await
        .unwrap();
    next_json(&mut ws, 2000).await.unwrap();
    send_pos(&mut ws, 0, 0, "idle").await;
    // Out of bounds, bad anim, other zone, garbage: each a violation, none fatal alone.
    send_pos(&mut ws, 9_000, 0, "idle").await;
    send_pos(&mut ws, 0, 0, "Bad Anim!").await;
    ws.send(Message::Text(
        "{\"t\":\"pos\",\"zone\":\"moon\",\"x_dm\":0,\"z_dm\":0,\"yaw_deg\":0,\"anim\":\"idle\"}".into(),
    ))
    .await
    .unwrap();
    ws.send(Message::Text("not json".into())).await.unwrap();
    // Teleporting 40 m every few ms: speed violations pile up past the limit → close.
    let mut closed = false;
    for i in 0..40 {
        let x = if i % 2 == 0 { 400 } else { -400 };
        if ws
            .send(Message::Text(
                json!({ "t": "pos", "zone": "island", "x_dm": x, "z_dm": 0, "yaw_deg": 0, "anim": "idle" })
                    .to_string()
                    .into(),
            ))
            .await
            .is_err()
        {
            closed = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(5)).await;
    }
    if !closed {
        // Drain until the server's close frame (or the stream ending).
        let deadline = tokio::time::Instant::now() + Duration::from_secs(5);
        while tokio::time::Instant::now() < deadline {
            match tokio::time::timeout(Duration::from_millis(500), ws.next()).await {
                Ok(Some(Ok(Message::Close(_)))) | Ok(None) | Ok(Some(Err(_))) => {
                    closed = true;
                    break;
                }
                _ => {}
            }
        }
    }
    assert!(closed, "the server closed the socket after too many violations");
}
