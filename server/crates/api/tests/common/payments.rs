//! Shared fixtures for the payments tests: in-process fake Solana RPC (one or two
//! providers) and a fake Jupiter price endpoint, plus request helpers.
#![allow(dead_code, clippy::unwrap_used, clippy::expect_used, clippy::panic)]

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::{get, post};
use axum::{Json, Router};
use cf_api::state::build_payments;
use cf_api::{build_app, AppState};
use cf_chain::{COMPUTE_BUDGET_PROGRAM, MEMO_PROGRAM, TOKEN_PROGRAM};
use cf_common::{Config, PaymentsConfig};
use serde_json::{json, Value};

use super::{post as api_post, Session};

pub const MINT: &str = "9mM1Mc4Ta9UJJ32v5qsHef91PiXi7EWyiSsqF5WXpump";
pub const TREASURY_WALLET: &str = "TreasuryWa11et11111111111111111111111111111";
pub const TREASURY_ATA: &str = "TreasuryATA111111111111111111111111111111111";

#[derive(Clone, Default)]
pub struct FakeChain {
    pub txs: Arc<Mutex<HashMap<String, Value>>>,
    pub price_ok: Arc<Mutex<bool>>,
    /// Second provider that quietly changes the burn amount (cross-check must catch it).
    pub tamper: bool,
    /// Second provider that has not indexed anything yet (ordinary lag, not a disagreement).
    pub lag: bool,
}

pub async fn rpc(State(chain): State<FakeChain>, Json(req): Json<Value>) -> Json<Value> {
    let method = req["method"].as_str().unwrap_or("");
    let params = &req["params"];
    let result = match method {
        "getTransaction" => {
            let mut tx = chain
                .txs
                .lock()
                .unwrap()
                .get(params[0].as_str().unwrap_or(""))
                .cloned()
                .unwrap_or(Value::Null);
            if chain.lag {
                tx = Value::Null;
            }
            if chain.tamper && !tx.is_null() {
                tx["transaction"]["message"]["instructions"][1]["parsed"]["info"]["tokenAmount"]["amount"] =
                    json!("1");
            }
            tx
        }
        "getAccountInfo" => match params[0].as_str().unwrap_or("") {
            MINT => {
                json!({ "value": { "owner": TOKEN_PROGRAM, "data": { "parsed": { "type": "mint", "info": { "decimals": 6, "mintAuthority": null, "supply": "1" } } } } })
            }
            TREASURY_ATA => {
                json!({ "value": { "owner": TOKEN_PROGRAM, "data": { "parsed": { "type": "account", "info": { "mint": MINT, "owner": TREASURY_WALLET } } } } })
            }
            _ => json!({ "value": null }),
        },
        _ => Value::Null,
    };
    Json(json!({ "jsonrpc": "2.0", "id": 1, "result": result }))
}

/// Startup checks succeed, but every `getTransaction` fails with HTTP 500.
pub async fn rpc_broken(State(chain): State<FakeChain>, Json(req): Json<Value>) -> (StatusCode, Json<Value>) {
    if req["method"] == "getTransaction" {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "boom" })),
        );
    }
    (StatusCode::OK, rpc(State(chain), Json(req)).await)
}

pub async fn price(State(chain): State<FakeChain>) -> (StatusCode, Json<Value>) {
    if *chain.price_ok.lock().unwrap() {
        (
            StatusCode::OK,
            Json(json!({ "data": { MINT: { "price": "0.0031" } } })),
        )
    } else {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(json!({ "error": "down" })),
        )
    }
}

pub async fn fake_servers(chain: FakeChain) -> (String, String) {
    let app = Router::new()
        .route("/", post(rpc))
        .route("/broken", post(rpc_broken))
        .route("/price", get(price))
        .with_state(chain);
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let addr = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (format!("http://{addr}/"), format!("http://{addr}/price"))
}

pub async fn app_with_payments(price_ok: bool) -> (Router, sqlx::PgPool, FakeChain) {
    let (state, pool, chain) = state_with_payments(price_ok).await;
    (build_app(state), pool, chain)
}

pub async fn state_with_payments(price_ok: bool) -> (AppState, sqlx::PgPool, FakeChain) {
    state_with_options(price_ok, Providers::Single).await
}

pub enum Providers {
    Single,
    /// Primary + a second provider that tampers with amounts.
    Disagreeing,
    /// Primary returns HTTP 500.
    Broken,
    /// Secondary never sees the transaction.
    Lagging,
}

pub async fn state_with_options(price_ok: bool, providers: Providers) -> (AppState, sqlx::PgPool, FakeChain) {
    let chain = FakeChain {
        txs: Arc::default(),
        price_ok: Arc::new(Mutex::new(price_ok)),
        tamper: false,
        lag: false,
    };
    let (mut rpc_url, price_url) = fake_servers(chain.clone()).await;
    let mut secondary: Option<String> = None;
    match providers {
        Providers::Single => {}
        Providers::Disagreeing => {
            let (url, _) = fake_servers(FakeChain {
                tamper: true,
                ..chain.clone()
            })
            .await;
            secondary = Some(url);
        }
        Providers::Broken => rpc_url = format!("{rpc_url}broken"),
        Providers::Lagging => {
            let (url, _) = fake_servers(FakeChain {
                lag: true,
                ..chain.clone()
            })
            .await;
            secondary = Some(url);
        }
    }
    let (_, pool) = super::app().await;
    let redis_url =
        std::env::var("CF_TEST_REDIS_URL").unwrap_or_else(|_| "redis://127.0.0.1:6379/1".to_owned());
    let vars: HashMap<String, String> = [
        ("CF_DATABASE_URL", super::database_url()),
        ("CF_REDIS_URL", redis_url.clone()),
        (
            "CF_COOKIE_SECRET",
            "test-cookie-secret-test-cookie-secret-0000".to_owned(),
        ),
        (
            "CF_CSRF_SECRET",
            "test-csrf-secret-test-csrf-secret-000000".to_owned(),
        ),
        ("CF_SIWS_DOMAIN", super::DOMAIN.to_owned()),
        ("CF_CLASH_MINT", MINT.to_owned()),
        ("CF_RPC_PRIMARY_URL", rpc_url),
        ("CF_RPC_SECONDARY_URL", secondary.unwrap_or_default()),
        ("CF_JUPITER_PRICE_URL", price_url),
        ("CF_TREASURY_WALLET", TREASURY_WALLET.to_owned()),
        ("CF_TREASURY_TOKEN_ACCOUNT", TREASURY_ATA.to_owned()),
    ]
    .into_iter()
    .map(|(k, v)| (k.to_owned(), v))
    .collect();
    let config = Config::from_map(&vars).expect("config");
    let pc: PaymentsConfig = config.payments.clone().expect("payments config");
    let runtime = build_payments(&pc, &config).await.expect("startup checks");
    let client = redis::Client::open(redis_url).unwrap();
    let redis = client.get_connection_manager().await.unwrap();
    let state = AppState::from_parts_with_payments(config, pool.clone(), redis, Some(Arc::new(runtime)));
    (state, pool, chain)
}

pub fn token_ix(kind: &str, amount: i64, owner: &str, extra: Value) -> Value {
    let mut info = json!({ "mint": MINT, "authority": owner, "tokenAmount": { "amount": amount.to_string(), "decimals": 6, "uiAmount": 0.0, "uiAmountString": "0" } });
    for (k, v) in extra.as_object().unwrap() {
        info[k] = v.clone();
    }
    json!({ "program": "spl-token", "programId": TOKEN_PROGRAM, "parsed": { "type": kind, "info": info } })
}

pub fn payment_tx(owner: &str, burn: i64, transfer: i64, destination: &str, memo: &str, err: Value) -> Value {
    json!({
        "slot": 250_000_000u64,
        "blockTime": time::OffsetDateTime::now_utc().unix_timestamp(),
        "meta": { "err": err, "innerInstructions": [] },
        "transaction": {
            "signatures": ["x"],
            "message": { "instructions": [
                { "programId": COMPUTE_BUDGET_PROGRAM, "accounts": [], "data": "3gJqkocMWaMm" },
                token_ix("burnChecked", burn, owner, json!({ "account": "PlayerATA" })),
                token_ix("transferChecked", transfer, owner, json!({ "source": "PlayerATA", "destination": destination })),
                { "program": "spl-memo", "programId": MEMO_PROGRAM, "parsed": memo }
            ] }
        }
    })
}

pub fn signature() -> String {
    bs58::encode(
        super::rand_bytes()
            .iter()
            .chain(super::rand_bytes().iter())
            .copied()
            .collect::<Vec<u8>>(),
    )
    .into_string()
}

pub async fn quote(app: &Router, session: &Session, item: &str, qty: i32) -> (StatusCode, Value) {
    api_post(
        app,
        session,
        "/orders",
        Some(json!({ "item_id": item, "qty": qty })),
        None,
    )
    .await
}

pub async fn submit(app: &Router, session: &Session, order_id: &str, sig: &str) -> (StatusCode, Value) {
    api_post(
        app,
        session,
        &format!("/orders/{order_id}/submit"),
        Some(json!({ "signature": sig })),
        None,
    )
    .await
}

pub type TxBuilder<'a> = Box<dyn Fn(&str, i64, i64) -> Value + 'a>;

pub fn wallet_of(session: &Session) -> String {
    session.wallet.trim_start_matches("sol:").to_owned()
}

pub fn good_tx(session: &Session, order: &Value) -> Value {
    payment_tx(
        &wallet_of(session),
        order["burn_amount"].as_i64().unwrap(),
        order["transfer_amount"].as_i64().unwrap(),
        TREASURY_ATA,
        &format!("cf:{}", order["order_id"].as_str().unwrap()),
        Value::Null,
    )
}
