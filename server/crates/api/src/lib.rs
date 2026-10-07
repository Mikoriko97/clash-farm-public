//! HTTP API (axum). Router construction lives here so integration tests can
//! drive the app with `tower::ServiceExt::oneshot` without binding a port.
#![forbid(unsafe_code)]

pub mod error;
pub mod idempotency;
pub mod jobs;
pub mod middleware;
pub mod presence;
pub mod routes;
pub mod state;

use axum::http::header::CACHE_CONTROL;
use axum::http::HeaderValue;
use axum::routing::{get, post};
use axum::Router;
use tower_http::limit::RequestBodyLimitLayer;
use tower_http::request_id::{MakeRequestUuid, PropagateRequestIdLayer, SetRequestIdLayer};
use tower_http::set_header::SetResponseHeaderLayer;
use tower_http::trace::TraceLayer;

pub use state::AppState;

/// Max JSON body for every mutating endpoint (docs/API.md).
pub const BODY_LIMIT_BYTES: usize = 16 * 1024;

pub fn build_app(state: AppState) -> Router {
    let api = Router::new()
        .route("/auth/nonce", get(routes::auth::nonce))
        .route("/auth/verify", post(routes::auth::verify))
        .route("/auth/guest", post(routes::auth::guest))
        .route("/auth/link", post(routes::auth::link))
        .route("/auth/refresh", post(routes::auth::refresh))
        .route("/auth/logout", post(routes::auth::logout))
        .route("/me", get(routes::me::me))
        .route("/me/character", post(routes::character::create))
        .route("/me/look", post(routes::character::change_look))
        .route("/me/nickname", post(routes::character::change_nickname))
        .route("/me/nickname-check", get(routes::character::nickname_check))
        .route("/island", get(routes::world::island))
        .route("/island/expand", post(routes::world::expand))
        .route("/island/clear", post(routes::shop::clear))
        .route("/plots/{id}/plant", post(routes::world::plant))
        .route("/plots/{id}/fertilize", post(routes::world::fertilize))
        .route("/plots/{id}/harvest", post(routes::world::harvest))
        .route("/plots/place", post(routes::plots_place::place))
        .route("/plots/{id}/remove", post(routes::plots_place::remove))
        .route("/plots/{id}/bed/plant", post(routes::plots_place::bed_plant))
        .route(
            "/plots/{id}/bed/fertilize",
            post(routes::plots_place::bed_fertilize),
        )
        .route("/plots/{id}/bed/harvest", post(routes::plots_place::bed_harvest))
        .route("/resources/{idx}/gather", post(routes::resources::gather))
        .route("/recipes", get(routes::craft::recipes))
        .route("/market/sell", post(routes::craft::sell))
        .route("/craft", post(routes::craft::craft_one))
        .route("/items/{id}/eat", post(routes::craft::eat))
        .route("/house/sleep", post(routes::house::sleep))
        .route("/weather", get(routes::weather::current))
        .route("/weather/lightning", post(routes::weather::lightning))
        .route("/inventory", get(routes::world::inventory))
        .route("/catalog", get(routes::world::catalog))
        .route("/shop/buy-credits", post(routes::shop::buy_credits))
        .route("/decor/place", post(routes::shop::place))
        .route("/decor/remove", post(routes::shop::remove))
        .route("/orders", post(routes::orders::create))
        .route("/orders/{id}", get(routes::orders::status))
        .route("/orders/{id}/submit", post(routes::orders::submit))
        .route("/mainland/map", get(routes::mainland::map))
        .route("/mainland/chunks", get(routes::mainland::chunks))
        .route("/mainland/nodes/{id}/gather", post(routes::mainland::gather))
        .route("/cave/{id}", get(routes::mainland::cave))
        .route("/city", get(routes::mainland::city_buildings))
        .route("/mainland/explored", get(routes::exploration::explored))
        .route("/mainland/explore", post(routes::exploration::explore))
        .route("/mainland/discover", post(routes::exploration::discover))
        .route(
            "/mainland/expeditions",
            get(routes::exploration::expeditions_today),
        )
        .route(
            "/mainland/expeditions/{n}/claim",
            post(routes::exploration::claim),
        )
        .route("/tools", get(routes::tools::list))
        .route("/tools/{id}/repair", post(routes::tools::repair))
        .route("/ws/presence", get(presence::ws::presence))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            middleware::csrf::enforce,
        ))
        .layer(axum::middleware::from_fn_with_state(
            state.clone(),
            middleware::kill_switch::maintenance,
        ))
        .layer(RequestBodyLimitLayer::new(BODY_LIMIT_BYTES))
        .layer(SetResponseHeaderLayer::overriding(
            CACHE_CONTROL,
            HeaderValue::from_static("no-store"),
        ));

    Router::new()
        .route("/healthz", get(routes::health::healthz))
        .route("/readyz", get(routes::health::readyz))
        .nest("/api/v1", api)
        .layer(axum::middleware::from_fn(middleware::request_id::finalize_errors))
        .layer(TraceLayer::new_for_http())
        .layer(PropagateRequestIdLayer::x_request_id())
        .layer(SetRequestIdLayer::x_request_id(MakeRequestUuid))
        .with_state(state)
}
