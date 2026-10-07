//! Cave #1 and storm-felled trees (migration 0016, docs/MAINLAND.md §3–4):
//! the cave lists its nodes, crystals need the character inside the cave zone,
//! a pinned storm lays trees in the forests that anyone strips without an axe.
//! Local Postgres + Memurai; the forced weather lives in the shared settings.
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

type Ws = WebSocketStream<MaybeTlsStream<TcpStream>>;

async fn serve(app: axum::Router) -> SocketAddr {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, app).await.unwrap();
    });
    addr
}

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
async fn test_cave_crystals_need_the_character_inside_the_cave() {
    let (app, pool) = app().await;
    let addr = serve(app.clone()).await;
    let session = login(&app).await;
    get(&app, &session, "/island").await;
    let (status, body) = get(&app, &session, "/cave/1").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let nodes = body["nodes"].as_array().unwrap();
    assert_eq!(nodes.len(), 14);
    let crystal = nodes.iter().find(|n| n["kind"] == "crystal").unwrap().clone();
    let id = crystal["id"].as_str().unwrap().to_owned();
    assert!(id.starts_with("99_1_"), "{id}");
    let (status, _) = get(&app, &session, "/cave/2").await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    sqlx::query("DELETE FROM cf.mainland_nodes WHERE id = $1")
        .bind(&id)
        .execute(&pool)
        .await
        .unwrap();
    let (x, z) = (
        crystal["x_dm"].as_i64().unwrap() as i32,
        crystal["z_dm"].as_i64().unwrap() as i32,
    );
    // Standing at the mine gate on the mainland does not reach into the cave.
    let _outside = stand_at(addr, &session, "mainland", 2800, -1900).await;
    let (status, body) = gather(&app, &session, &id).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["error"]["code"], "too_far");
    // Inside, next to the vein: a crystal, bare-handed (one unit).
    let _inside = stand_at(addr, &session, "cave", x + 20, z).await;
    let (status, body) = gather(&app, &session, &id).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["item_id"], "crystal");
    assert_eq!(body["qty"], 1);
    assert_eq!(body["node"]["hits_left"], 2);
    let (_, inv) = get(&app, &session, "/inventory").await;
    assert_eq!(qty(&inv, "crystal"), 1);
    // The cave view shows the live stock.
    let (_, body) = get(&app, &session, "/cave/1").await;
    let shown = body["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["id"] == id)
        .unwrap();
    assert_eq!(shown["hits_left"], 2);
}

#[tokio::test]
async fn test_storm_fells_trees_that_anyone_strips_without_an_axe() {
    let (app, pool) = app().await;
    // Pin the weather before the first request: the params cache loads once per 30 s.
    sqlx::query(
        "INSERT INTO cf.settings (key, value) VALUES ('weather.force', '\"storm\"'::jsonb)
         ON CONFLICT (key) DO UPDATE SET value = EXCLUDED.value",
    )
    .execute(&pool)
    .await
    .unwrap();
    let addr = serve(app.clone()).await;
    let session = login(&app).await;
    get(&app, &session, "/island").await;
    let (status, weather) = get(&app, &session, "/weather").await;
    assert_eq!(status, StatusCode::OK, "{weather}");
    let result: Result<(), String> = async {
        if weather["state"] != "storm" {
            return Err(format!("weather not pinned: {weather}"));
        }
        let slot = weather["slot"].as_i64().unwrap();
        let trees = cf_world::storm::fallen_trees(slot);
        let tree = &trees[0];
        let (cx, cz) = cf_world::exploration::chunk_of(tree.x_dm, tree.z_dm);
        let (status, body) = get(&app, &session, &format!("/mainland/chunks?cx={cx}&cz={cz}&r=0")).await;
        if status != StatusCode::OK {
            return Err(format!("chunks {status}: {body}"));
        }
        let listed = body["chunks"][0]["nodes"]
            .as_array()
            .unwrap()
            .iter()
            .any(|n| n["id"] == tree.id && n["kind"] == "tree_fallen");
        if !listed {
            return Err(format!("felled tree {} missing from its chunk: {body}", tree.id));
        }
        sqlx::query("DELETE FROM cf.mainland_nodes WHERE id = $1")
            .bind(&tree.id)
            .execute(&pool)
            .await
            .unwrap();
        let _ws = stand_at(addr, &session, "mainland", tree.x_dm + 20, tree.z_dm).await;
        let (status, body) = gather(&app, &session, &tree.id).await;
        if status != StatusCode::OK {
            return Err(format!("gather {status}: {body}"));
        }
        if body["item_id"] != "wood_pine" || !body["tool"].is_null() {
            return Err(format!("felled tree should give wood without a tool: {body}"));
        }
        // A tree of a storm that is not active is nobody's.
        let (status, _) = gather(&app, &session, &format!("98_{}_0", slot + 7)).await;
        if status != StatusCode::NOT_FOUND {
            return Err(format!("stale storm tree answered {status}"));
        }
        Ok(())
    }
    .await;
    sqlx::query("DELETE FROM cf.settings WHERE key = 'weather.force'")
        .execute(&pool)
        .await
        .unwrap();
    result.unwrap();
}
