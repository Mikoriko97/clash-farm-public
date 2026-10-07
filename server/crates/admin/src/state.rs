//! Shared state of the admin app.

use std::sync::Arc;

use cf_auth::rate::RateLimiter;
use cf_auth::session::SessionService;
use cf_auth::NonceStore;
use redis::aio::ConnectionManager;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use time::OffsetDateTime;

use crate::config::AdminConfig;
use crate::session::AdminSessions;

#[derive(Clone)]
pub struct AdminState {
    pub config: Arc<AdminConfig>,
    pub pool: PgPool,
    pub nonces: NonceStore,
    pub limiter: RateLimiter,
    pub sessions: AdminSessions,
    /// Job heartbeats and failure counters (`cf_common::jobs`), read for «Система» and the dashboard.
    pub redis: ConnectionManager,
    /// The players' sessions (same store as cf-api): a ban revokes them all at once.
    pub player_sessions: SessionService,
}

impl std::fmt::Debug for AdminState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AdminState")
            .field("env", &self.config.env)
            .finish_non_exhaustive()
    }
}

impl AdminState {
    pub async fn connect(config: AdminConfig) -> Result<Self, Box<dyn std::error::Error>> {
        let pool = PgPoolOptions::new()
            .max_connections(4)
            .connect(config.database_url.expose())
            .await?;
        let client = redis::Client::open(config.redis_url.expose())?;
        let redis = client.get_connection_manager().await?;
        Ok(Self::from_parts(config, pool, redis))
    }

    pub fn from_parts(config: AdminConfig, pool: PgPool, redis: ConnectionManager) -> Self {
        let sessions = AdminSessions::new(redis.clone(), config.cookie_secret.expose());
        let player_sessions = SessionService::new(pool.clone(), redis.clone(), config.cookie_secret.expose());
        Self {
            config: Arc::new(config),
            pool,
            nonces: NonceStore::new(redis.clone()),
            limiter: RateLimiter::new(redis.clone()),
            redis,
            sessions,
            player_sessions,
        }
    }

    /// (domain, uri) of the admin sign-in message: the page origin from `CF_ADMIN_ORIGIN`
    /// or `https://admin.<siws domain>`. Either way it differs from the game's domain, so a
    /// signature phished with the game's sign-in text can never open the admin.
    pub fn admin_origin(&self) -> (String, String) {
        match self.config.admin_origin.as_deref() {
            Some(origin) => {
                let host = origin.split_once("://").map(|(_, h)| h).unwrap_or(origin);
                (host.to_owned(), origin.to_owned())
            }
            None => {
                let domain = format!("admin.{}", self.config.siws_domain);
                let uri = format!("https://{domain}");
                (domain, uri)
            }
        }
    }
}

pub fn now_ms() -> i64 {
    let now = OffsetDateTime::now_utc();
    i64::try_from(now.unix_timestamp_nanos() / 1_000_000).unwrap_or(i64::MAX)
}
