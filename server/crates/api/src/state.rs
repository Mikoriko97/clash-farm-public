use std::sync::Arc;
use std::time::Instant;

use cf_auth::rate::RateLimiter;
use cf_auth::{NonceStore, SessionService};
use cf_chain::{RpcClient, RpcConfig};
use cf_common::{Config, PaymentsConfig};
use cf_payments::{PaymentsRuntime, PriceOracle};
use redis::aio::ConnectionManager;
use sqlx::postgres::PgPoolOptions;
use sqlx::PgPool;
use tokio::sync::RwLock;

/// Cached kill switches (refreshed every 5 s by the middleware).
#[derive(Debug, Clone, Copy)]
pub struct SwitchCache {
    pub checked_at: Instant,
    pub maintenance: bool,
    pub shop_enabled: bool,
    pub payments_enabled: bool,
    /// `renames_enabled` (0030, ADR-039): paid nickname changes.
    pub renames_enabled: bool,
}

#[derive(Clone)]
pub struct AppState {
    pub config: Arc<Config>,
    pub pool: PgPool,
    pub redis: ConnectionManager,
    pub nonces: NonceStore,
    pub sessions: SessionService,
    pub limiter: RateLimiter,
    pub switches: Arc<RwLock<Option<SwitchCache>>>,
    pub params: cf_world::ParamsCache,
    /// `None` = payments disabled (no RPC configured or startup checks skipped).
    pub payments: Option<Arc<PaymentsRuntime>>,
    /// Player presence fan-out (docs/MAINLAND.md §8).
    pub presence: crate::presence::PresenceHub,
}

impl std::fmt::Debug for AppState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AppState")
            .field("env", &self.config.env)
            .field("payments", &self.payments.is_some())
            .finish_non_exhaustive()
    }
}

impl AppState {
    pub async fn connect(config: Config) -> Result<Self, Box<dyn std::error::Error>> {
        // 24 connections (Postgres allows 100; cf-admin and jobs take their own): 8 queued every
        // farm action at peak. A request waits 5 s for one, then gets a 503 and the client retries.
        let pool = PgPoolOptions::new()
            .max_connections(24)
            .acquire_timeout(std::time::Duration::from_secs(5))
            .connect(config.database_url.expose())
            .await?;
        let client = redis::Client::open(config.redis_url.expose())?;
        let redis = client.get_connection_manager().await?;
        let payments = if config.payments_fake {
            tracing::error!(
                "CF_PAYMENTS_FAKE=1: CLASH payments are FAKE (no chain, fixed price). Local testing only."
            );
            Some(Arc::new(PaymentsRuntime::fake(
                &config.clash_mint,
                config.clash_decimals,
            )?))
        } else {
            match &config.payments {
                Some(pc) => Some(Arc::new(build_payments(pc, &config).await?)),
                None => {
                    tracing::warn!("CF_RPC_PRIMARY_URL not set: CLASH payments disabled");
                    None
                }
            }
        };
        Ok(Self::from_parts_with_payments(config, pool, redis, payments))
    }

    pub fn from_parts(config: Config, pool: PgPool, redis: ConnectionManager) -> Self {
        Self::from_parts_with_payments(config, pool, redis, None)
    }

    pub fn from_parts_with_payments(
        config: Config,
        pool: PgPool,
        redis: ConnectionManager,
        payments: Option<Arc<PaymentsRuntime>>,
    ) -> Self {
        let sessions = SessionService::new(pool.clone(), redis.clone(), config.cookie_secret.expose());
        Self {
            presence: crate::presence::PresenceHub::new(redis.clone()),
            nonces: NonceStore::new(redis.clone()),
            limiter: RateLimiter::new(redis.clone()),
            sessions,
            config: Arc::new(config),
            pool,
            redis,
            switches: Arc::new(RwLock::new(None)),
            params: cf_world::ParamsCache::default(),
            payments,
        }
    }
}

/// Startup checks (docs/ARCHITECTURE.md §5): the server refuses to run payments
/// unless the mint has the configured decimals and no mint authority, and the
/// treasury token account holds our mint for the treasury wallet.
pub async fn build_payments(
    pc: &PaymentsConfig,
    config: &Config,
) -> Result<PaymentsRuntime, Box<dyn std::error::Error>> {
    let chain = RpcClient::new(RpcConfig {
        primary_url: pc.rpc_primary_url.expose().to_owned(),
        secondary_url: pc.rpc_secondary_url.as_ref().map(|s| s.expose().to_owned()),
    })?;
    let mint = chain
        .get_mint(&config.clash_mint)
        .await?
        .ok_or("CLASH mint not found on chain")?;
    if mint.decimals != config.clash_decimals {
        return Err(format!(
            "CF_CLASH_DECIMALS={} but chain says {}",
            config.clash_decimals, mint.decimals
        )
        .into());
    }
    if mint.mint_authority.is_some() {
        return Err("CLASH mint still has a mint authority; refusing to start payments".into());
    }
    let treasury = chain
        .get_token_account(&pc.treasury_token_account)
        .await?
        .ok_or("treasury token account not found")?;
    if treasury.mint != config.clash_mint || treasury.owner != pc.treasury_wallet {
        return Err("treasury token account does not belong to CF_TREASURY_WALLET / CLASH mint".into());
    }
    let oracle = PriceOracle::new(pc.jupiter_price_url.expose(), &config.clash_mint)?;
    tracing::info!("payments enabled: mint verified, treasury account verified");
    Ok(PaymentsRuntime {
        chain,
        oracle,
        mint: config.clash_mint.clone(),
        decimals: config.clash_decimals,
        treasury_token_account: pc.treasury_token_account.clone(),
        fake: false,
    })
}
