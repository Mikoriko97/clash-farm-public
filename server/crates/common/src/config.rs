//! Configuration is read once from the process environment (docs/RUNBOOK.md §2).
//! Secrets are wrapped in [`Secret`] so they never end up in `Debug` output or logs.

use std::collections::HashMap;
use std::fmt;
use std::net::SocketAddr;

use thiserror::Error;

/// A value that must never be printed. `Debug` prints `***`.
#[derive(Clone)]
pub struct Secret(String);

impl Secret {
    pub fn new(value: String) -> Self {
        Self(value)
    }

    pub fn expose(&self) -> &str {
        &self.0
    }
}

impl fmt::Debug for Secret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("***")
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Env {
    Local,
    Staging,
    Prod,
}

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("missing required env var {0}")]
    Missing(&'static str),
    #[error("invalid value for {0}: {1}")]
    Invalid(&'static str, String),
}

#[derive(Debug, Clone)]
pub struct Config {
    pub env: Env,
    pub bind: SocketAddr,
    pub database_url: Secret,
    pub redis_url: Secret,
    /// Key for HMAC-deriving stored session/refresh identifiers. Rotation revokes every session.
    pub cookie_secret: Secret,
    pub csrf_secret: Secret,
    pub siws_domain: String,
    pub clash_mint: String,
    pub clash_decimals: u8,
    /// EVM chain ids accepted for SIWE login (ADR-012).
    pub evm_chain_ids: Vec<u64>,
    pub log_level: String,
    /// Browser origins allowed to open the presence WebSocket (SECURITY §3);
    /// `CF_WS_ORIGINS` comma list, default `https://<siws_domain>` (+ Vite dev origins when local).
    pub ws_origins: Vec<String>,
    /// CLASH payments (ADR-016/017). `None` = payments disabled (no RPC configured).
    pub payments: Option<PaymentsConfig>,
    /// `CF_PAYMENTS_FAKE=1`: orders are quoted at a fixed price and any well-formed
    /// signature counts as paid, without touching a chain. Refused unless `CF_ENV=local`.
    pub payments_fake: bool,
    /// Telegram delivery for the `alerts` job (ADR-023); `None` = signals stay in the admin.
    pub alerts: Option<AlertsConfig>,
}

/// Both variables or none: a half-configured channel must not start.
#[derive(Debug, Clone)]
pub struct AlertsConfig {
    pub telegram_bot_token: Secret,
    /// A user or group id (groups are negative); Telegram treats it as text anyway.
    pub telegram_chat_id: String,
}

#[derive(Debug, Clone)]
pub struct PaymentsConfig {
    pub rpc_primary_url: Secret,
    pub rpc_secondary_url: Option<Secret>,
    /// May carry a provider key in the query string: never printed.
    pub jupiter_price_url: Secret,
    /// Treasury wallet (Squads multisig) and its CLASH token account; both verified at startup.
    pub treasury_wallet: String,
    pub treasury_token_account: String,
}

impl Config {
    /// Reads `CF_*` variables from the process environment.
    pub fn from_env() -> Result<Self, ConfigError> {
        let vars: HashMap<String, String> = std::env::vars().collect();
        Self::from_map(&vars)
    }

    /// Same as [`Config::from_env`] but from an explicit map (tests).
    pub fn from_map(vars: &HashMap<String, String>) -> Result<Self, ConfigError> {
        let get = |key: &'static str| vars.get(key).map(String::as_str).filter(|v| !v.is_empty());
        let required = |key: &'static str| get(key).map(str::to_owned).ok_or(ConfigError::Missing(key));

        let env = match get("CF_ENV").unwrap_or("local") {
            "local" => Env::Local,
            "staging" => Env::Staging,
            "prod" => Env::Prod,
            other => return Err(ConfigError::Invalid("CF_ENV", other.to_owned())),
        };
        let bind = get("CF_BIND")
            .unwrap_or("127.0.0.1:8080")
            .parse()
            .map_err(|e: std::net::AddrParseError| ConfigError::Invalid("CF_BIND", e.to_string()))?;
        let clash_decimals = get("CF_CLASH_DECIMALS")
            .unwrap_or("6")
            .parse()
            .map_err(|e: std::num::ParseIntError| ConfigError::Invalid("CF_CLASH_DECIMALS", e.to_string()))?;
        let evm_chain_ids = parse_chain_ids(get("CF_EVM_CHAIN_IDS").unwrap_or("1,8453,42161"))?;
        let payments_fake = matches!(get("CF_PAYMENTS_FAKE"), Some("1" | "true"));
        // Fail closed: the flag needs an explicit CF_ENV=local (the default does not count)
        // and cannot sit next to a real RPC, so a copied env file can never fake production.
        if payments_fake && (env != Env::Local || get("CF_ENV").is_none()) {
            return Err(ConfigError::Invalid(
                "CF_PAYMENTS_FAKE",
                "fake payments are only allowed with an explicit CF_ENV=local".to_owned(),
            ));
        }
        if payments_fake && get("CF_RPC_PRIMARY_URL").is_some() {
            return Err(ConfigError::Invalid(
                "CF_PAYMENTS_FAKE",
                "fake payments cannot be combined with CF_RPC_PRIMARY_URL".to_owned(),
            ));
        }

        Ok(Self {
            env,
            bind,
            database_url: Secret(required("CF_DATABASE_URL")?),
            redis_url: Secret(required("CF_REDIS_URL")?),
            cookie_secret: Secret(required_secret(
                &required("CF_COOKIE_SECRET")?,
                "CF_COOKIE_SECRET",
            )?),
            csrf_secret: Secret(required_secret(&required("CF_CSRF_SECRET")?, "CF_CSRF_SECRET")?),
            siws_domain: required("CF_SIWS_DOMAIN")?,
            clash_mint: required("CF_CLASH_MINT")?,
            clash_decimals,
            evm_chain_ids,
            log_level: get("CF_LOG_LEVEL").unwrap_or("info").to_owned(),
            ws_origins: ws_origins(get("CF_WS_ORIGINS"), get("CF_SIWS_DOMAIN").unwrap_or(""), env),
            payments_fake,
            payments: payments_config(vars)?,
            alerts: alerts_config(vars)?,
        })
    }
}

/// `CF_ALERT_TELEGRAM_BOT_TOKEN` + `CF_ALERT_TELEGRAM_CHAT_ID` (ADR-023).
pub fn alerts_config(vars: &HashMap<String, String>) -> Result<Option<AlertsConfig>, ConfigError> {
    let get = |key: &'static str| vars.get(key).map(String::as_str).filter(|v| !v.is_empty());
    match (
        get("CF_ALERT_TELEGRAM_BOT_TOKEN"),
        get("CF_ALERT_TELEGRAM_CHAT_ID"),
    ) {
        (None, None) => Ok(None),
        (Some(_), None) => Err(ConfigError::Missing("CF_ALERT_TELEGRAM_CHAT_ID")),
        (None, Some(_)) => Err(ConfigError::Missing("CF_ALERT_TELEGRAM_BOT_TOKEN")),
        (Some(token), Some(chat)) => {
            if token.len() < 20 || token.len() > 200 || token.chars().any(char::is_whitespace) {
                return Err(ConfigError::Invalid(
                    "CF_ALERT_TELEGRAM_BOT_TOKEN",
                    "expected the bot token from @BotFather (no spaces)".to_owned(),
                ));
            }
            let digits = chat.strip_prefix('-').unwrap_or(chat);
            if digits.is_empty() || digits.len() > 32 || !digits.bytes().all(|b| b.is_ascii_digit()) {
                return Err(ConfigError::Invalid(
                    "CF_ALERT_TELEGRAM_CHAT_ID",
                    "expected a numeric chat id".to_owned(),
                ));
            }
            Ok(Some(AlertsConfig {
                telegram_bot_token: Secret(token.to_owned()),
                telegram_chat_id: chat.to_owned(),
            }))
        }
    }
}

/// All-or-nothing: once `CF_RPC_PRIMARY_URL` is set, the rest of the payment
/// variables are required so a half-configured server cannot start.
fn payments_config(vars: &HashMap<String, String>) -> Result<Option<PaymentsConfig>, ConfigError> {
    let get = |key: &'static str| vars.get(key).map(String::as_str).filter(|v| !v.is_empty());
    let Some(primary) = get("CF_RPC_PRIMARY_URL") else {
        return Ok(None);
    };
    let required = |key: &'static str| get(key).map(str::to_owned).ok_or(ConfigError::Missing(key));
    Ok(Some(PaymentsConfig {
        rpc_primary_url: Secret(primary.to_owned()),
        rpc_secondary_url: get("CF_RPC_SECONDARY_URL").map(|v| Secret(v.to_owned())),
        jupiter_price_url: Secret(required("CF_JUPITER_PRICE_URL")?),
        treasury_wallet: required("CF_TREASURY_WALLET")?,
        treasury_token_account: required("CF_TREASURY_TOKEN_ACCOUNT")?,
    }))
}

/// Explicit list wins; otherwise the game's own origin, plus the Vite dev server locally.
fn ws_origins(raw: Option<&str>, siws_domain: &str, env: Env) -> Vec<String> {
    if let Some(list) = raw {
        return list
            .split(',')
            .map(str::trim)
            .filter(|s| !s.is_empty())
            .map(str::to_owned)
            .collect();
    }
    let mut origins = vec![format!("https://{siws_domain}")];
    if env == Env::Local {
        origins.push("http://localhost:5173".to_owned());
        origins.push("http://127.0.0.1:5173".to_owned());
    }
    origins
}

/// Secrets must be at least 32 bytes of entropy (64 hex chars per RUNBOOK).
fn required_secret(value: &str, key: &'static str) -> Result<String, ConfigError> {
    if value.len() < 32 {
        return Err(ConfigError::Invalid(
            key,
            "must be at least 32 characters".to_owned(),
        ));
    }
    Ok(value.to_owned())
}

fn parse_chain_ids(raw: &str) -> Result<Vec<u64>, ConfigError> {
    raw.split(',')
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(|s| {
            s.parse::<u64>()
                .map_err(|e| ConfigError::Invalid("CF_EVM_CHAIN_IDS", e.to_string()))
        })
        .collect()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;

    fn base() -> HashMap<String, String> {
        [
            ("CF_DATABASE_URL", "postgres://migrator@127.0.0.1/cf_test"),
            ("CF_REDIS_URL", "redis://127.0.0.1:6379/1"),
            ("CF_COOKIE_SECRET", "0123456789abcdef0123456789abcdef"),
            ("CF_CSRF_SECRET", "fedcba9876543210fedcba9876543210"),
            ("CF_SIWS_DOMAIN", "localhost"),
            ("CF_CLASH_MINT", "9mM1Mc4Ta9UJJ32v5qsHef91PiXi7EWyiSsqF5WXpump"),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .collect()
    }

    #[test]
    fn fake_payments_only_local() {
        let mut vars = base();
        vars.insert("CF_PAYMENTS_FAKE".to_owned(), "1".to_owned());
        assert!(
            Config::from_map(&vars).is_err(),
            "the default env does not count as local"
        );
        vars.insert("CF_ENV".to_owned(), "local".to_owned());
        assert!(Config::from_map(&vars).expect("explicit local").payments_fake);
        for env in ["staging", "prod"] {
            vars.insert("CF_ENV".to_owned(), env.to_owned());
            assert!(
                Config::from_map(&vars).is_err(),
                "fake payments must never start in {env}"
            );
        }
        vars.insert("CF_ENV".to_owned(), "local".to_owned());
        vars.insert("CF_RPC_PRIMARY_URL".to_owned(), "https://rpc.example".to_owned());
        assert!(
            Config::from_map(&vars).is_err(),
            "a real RPC next to the fake flag is a mistake"
        );
        assert!(!Config::from_map(&base()).unwrap().payments_fake);
    }

    #[test]
    fn parses_defaults() {
        let cfg = Config::from_map(&base()).unwrap();
        assert_eq!(cfg.env, Env::Local);
        assert_eq!(cfg.clash_decimals, 6);
        assert_eq!(cfg.evm_chain_ids, vec![1, 8453, 42161]);
        assert_eq!(format!("{:?}", cfg.cookie_secret), "***");
    }

    #[test]
    fn rejects_short_secret_and_missing_url() {
        let mut vars = base();
        vars.insert("CF_COOKIE_SECRET".into(), "short".into());
        assert!(matches!(
            Config::from_map(&vars),
            Err(ConfigError::Invalid("CF_COOKIE_SECRET", _))
        ));
        let mut vars = base();
        vars.remove("CF_DATABASE_URL");
        assert!(matches!(
            Config::from_map(&vars),
            Err(ConfigError::Missing("CF_DATABASE_URL"))
        ));
    }

    #[test]
    fn debug_output_never_contains_secrets() {
        let cfg = Config::from_map(&base()).unwrap();
        let dbg = format!("{cfg:?}");
        assert!(!dbg.contains("migrator@"));
        assert!(!dbg.contains("0123456789abcdef"));
    }
}
