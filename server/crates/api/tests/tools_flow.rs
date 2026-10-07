//! Tools with durability (migration 0013, docs/MAINLAND.md §5 П7): a hit
//! without the tool gives one unit, with it two and one wear; at the
//! durability the unit breaks; repair takes materials. Local Postgres +
//! Memurai, presence seeded like a reconnecting client.
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

async fn stand_at(addr: SocketAddr, session: &Session, x_dm: i32, z_dm: i32) -> Ws {
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
        .arg(format!("mainland:{x_dm}:{z_dm}:{now_ms}"))
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
    let text =
        json!({ "t": "pos", "zone": "mainland", "x_dm": x_dm, "z_dm": z_dm, "yaw_deg": 0, "anim": "idle" })
            .to_string();
    ws.send(Message::Text(text.into())).await.unwrap();
    tokio::time::sleep(Duration::from_millis(150)).await;
    ws
}

/// The i-th pine of chunk (5, 3); each test takes its own and resets it. The baked
/// table decides the order of kinds, so pick by kind rather than by position.
async fn pine(app: &axum::Router, session: &Session, pool: &sqlx::PgPool, i: usize) -> Value {
    let (status, body) = get(app, session, "/mainland/chunks?cx=5&cz=3&r=0").await;
    assert_eq!(status, StatusCode::OK, "{body}");
    let pines: Vec<Value> = body["chunks"][0]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|n| n["kind"] == "tree_pine")
        .cloned()
        .collect();
    assert!(pines.len() > i, "chunk (5, 3) has pines: {body}");
    let node = pines[i].clone();
    sqlx::query("DELETE FROM cf.mainland_nodes WHERE id = $1")
        .bind(node["id"].as_str().unwrap())
        .execute(pool)
        .await
        .unwrap();
    node
}

async fn give(pool: &sqlx::PgPool, session: &Session, item: &str, qty: i32) {
    sqlx::query(
        "INSERT INTO cf.inventory (user_id, item_id, qty) VALUES ($1, $2, $3)
         ON CONFLICT (user_id, item_id) DO UPDATE SET qty = cf.inventory.qty + $3",
    )
    .bind(Uuid::parse_str(&session.user_id).unwrap())
    .bind(item)
    .bind(qty)
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

fn tool<'a>(tools: &'a Value, id: &str) -> &'a Value {
    tools["tools"]
        .as_array()
        .unwrap()
        .iter()
        .find(|t| t["item_id"] == id)
        .unwrap()
}

#[tokio::test]
async fn test_axe_doubles_wood_and_wears_then_breaks() {
    let (app, pool) = app().await;
    let addr = serve(app.clone()).await;
    let session = login(&app).await;
    get(&app, &session, "/island").await;
    let node = pine(&app, &session, &pool, 0).await;
    let id = node["id"].as_str().unwrap();
    let _ws = stand_at(
        addr,
        &session,
        node["x_dm"].as_i64().unwrap() as i32 + 10,
        node["z_dm"].as_i64().unwrap() as i32,
    )
    .await;
    // Bare hands: one unit, no tool in the answer.
    let (status, body) = gather(&app, &session, id).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["qty"], 1);
    assert!(body["tool"].is_null());
    // Craft an axe from mainland materials, then hit: two units and one wear.
    give(&pool, &session, "wood_pine", 4).await;
    give(&pool, &session, "ore_copper", 2).await;
    let (status, body) = post(
        &app,
        &session,
        "/craft",
        Some(json!({ "recipe_id": "axe", "times": 1 })),
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["output_item"], "tool_axe");
    let (status, body) = gather(&app, &session, id).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["qty"], 2);
    assert_eq!(body["tool"]["item_id"], "tool_axe");
    assert_eq!(body["tool"]["wear"], 1);
    assert_eq!(body["tool"]["durability"], 100);
    assert_eq!(body["tool"]["broke"], false);
    let (_, tools) = get(&app, &session, "/tools").await;
    let axe = tool(&tools, "tool_axe");
    assert_eq!(axe["qty"], 1);
    assert_eq!(axe["wear"], 1);
    assert_eq!(tool(&tools, "tool_pickaxe")["qty"], 0);
    // One hit short of the durability: the next swing breaks the axe.
    sqlx::query("UPDATE cf.tool_wear SET wear = 99 WHERE user_id = $1 AND item_id = 'tool_axe'")
        .bind(Uuid::parse_str(&session.user_id).unwrap())
        .execute(&pool)
        .await
        .unwrap();
    let (status, body) = gather(&app, &session, id).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["qty"], 2, "the breaking swing still counts as a tool hit");
    assert_eq!(body["tool"]["broke"], true);
    assert_eq!(body["tool"]["wear"], 0);
    let (_, inv) = get(&app, &session, "/inventory").await;
    assert_eq!(qty(&inv, "tool_axe"), 0);
    assert_eq!(
        qty(&inv, "wood_pine"),
        5,
        "1 + 2 + 2 from three hits (the four crafted away)"
    );
    // Bare hands again.
    let (status, body) = gather(&app, &session, id).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["qty"], 1);
    assert!(body["tool"].is_null());
}

#[tokio::test]
async fn test_repair_needs_a_worn_tool_and_materials() {
    let (app, pool) = app().await;
    let session = login(&app).await;
    get(&app, &session, "/island").await;
    let user_id = Uuid::parse_str(&session.user_id).unwrap();
    // No pickaxe at all.
    let (status, _) = post(&app, &session, "/tools/tool_pickaxe/repair", None, None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (status, _) = post(&app, &session, "/tools/tool_hammer/repair", None, None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    give(&pool, &session, "tool_pickaxe", 1).await;
    // Fresh: nothing to repair.
    let (status, body) = post(&app, &session, "/tools/tool_pickaxe/repair", None, None).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["error"]["code"], "wrong_state");
    sqlx::query("INSERT INTO cf.tool_wear (user_id, item_id, wear) VALUES ($1, 'tool_pickaxe', 40)")
        .bind(user_id)
        .execute(&pool)
        .await
        .unwrap();
    // Worn but no copper.
    let (status, body) = post(&app, &session, "/tools/tool_pickaxe/repair", None, None).await;
    assert_eq!(status, StatusCode::CONFLICT, "{body}");
    assert_eq!(body["error"]["code"], "insufficient_items");
    give(&pool, &session, "ore_copper", 3).await;
    let (status, body) = post(&app, &session, "/tools/tool_pickaxe/repair", None, None).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(body["tool"]["wear"], 0);
    assert_eq!(body["tool"]["qty"], 1);
    let (_, inv) = get(&app, &session, "/inventory").await;
    assert_eq!(qty(&inv, "ore_copper"), 1, "two copper spent");
    let (_, tools) = get(&app, &session, "/tools").await;
    assert_eq!(tool(&tools, "tool_pickaxe")["wear"], 0);
    assert_eq!(tool(&tools, "tool_pickaxe")["repair"][0]["item_id"], "ore_copper");
}

#[tokio::test]
async fn test_repair_without_a_catalog_price_is_refused_not_free() {
    let (app, pool) = app().await;
    let session = login(&app).await;
    get(&app, &session, "/island").await;
    let user_id = Uuid::parse_str(&session.user_id).unwrap();
    give(&pool, &session, "tool_axe", 1).await;
    sqlx::query("INSERT INTO cf.tool_wear (user_id, item_id, wear) VALUES ($1, 'tool_axe', 10)")
        .bind(user_id)
        .execute(&pool)
        .await
        .unwrap();
    let original: serde_json::Value = sqlx::query_scalar("SELECT meta FROM cf.items WHERE id = 'tool_axe'")
        .fetch_one(&pool)
        .await
        .unwrap();
    sqlx::query("UPDATE cf.items SET meta = meta - 'repair' WHERE id = 'tool_axe'")
        .execute(&pool)
        .await
        .unwrap();
    let (status, body) = post(&app, &session, "/tools/tool_axe/repair", None, None).await;
    sqlx::query("UPDATE cf.items SET meta = $1 WHERE id = 'tool_axe'")
        .bind(original)
        .execute(&pool)
        .await
        .unwrap();
    assert_eq!(status, StatusCode::BAD_REQUEST, "{body}");
    let (_, tools) = get(&app, &session, "/tools").await;
    assert_eq!(tool(&tools, "tool_axe")["wear"], 10, "wear untouched");
}

#[tokio::test]
async fn test_parallel_hits_break_the_axe_once_and_never_below_zero() {
    let (app, pool) = app().await;
    let addr = serve(app.clone()).await;
    let session = login(&app).await;
    get(&app, &session, "/island").await;
    let user_id = Uuid::parse_str(&session.user_id).unwrap();
    let node = pine(&app, &session, &pool, 1).await;
    let id = node["id"].as_str().unwrap();
    let _ws = stand_at(
        addr,
        &session,
        node["x_dm"].as_i64().unwrap() as i32,
        node["z_dm"].as_i64().unwrap() as i32 + 10,
    )
    .await;
    give(&pool, &session, "tool_axe", 1).await;
    sqlx::query("INSERT INTO cf.tool_wear (user_id, item_id, wear) VALUES ($1, 'tool_axe', 99)")
        .bind(user_id)
        .execute(&pool)
        .await
        .unwrap();
    let (a, b) = tokio::join!(gather(&app, &session, id), gather(&app, &session, id));
    assert_eq!(a.0, StatusCode::OK, "{}", a.1);
    assert_eq!(b.0, StatusCode::OK, "{}", b.1);
    let broke = [&a.1, &b.1].iter().filter(|r| r["tool"]["broke"] == true).count();
    assert_eq!(broke, 1, "exactly one swing breaks the axe: {} {}", a.1, b.1);
    let (_, inv) = get(&app, &session, "/inventory").await;
    assert_eq!(qty(&inv, "tool_axe"), 0);
    assert_eq!(qty(&inv, "wood_pine"), 3, "2 with the axe, then 1 bare-handed");
    let (_, tools) = get(&app, &session, "/tools").await;
    assert_eq!(tool(&tools, "tool_axe")["wear"], 0);
}
