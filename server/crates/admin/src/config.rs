//! Admin binary configuration (docs/RUNBOOK.md): its own bind address, the
//! allowlist of admin wallets, and the game's database/redis. It holds no
//! chain keys and no RPC. Fails closed on anything it cannot honour yet.

use std::collections::HashMap;
use std::net::SocketAddr;

use cf_auth::{WalletKind, WalletTag};
use cf_common::{ConfigError, Env, Secret};

use crate::totp::TotpSecret;

#[derive(Debug, Clone)]
pub struct AdminConfig {
    pub env: Env,
    /// Loopback only: the admin is reached through an SSH tunnel (RUNBOOK §1).
    pub bind: SocketAddr,
    pub database_url: Secret,
    pub redis_url: Secret,
    /// Keys the stored admin session identifiers; rotation signs every admin out.
    pub cookie_secret: Secret,
    pub siws_domain: String,
    /// Base58 Solana public keys allowed to sign in.
    pub admin_wallets: Vec<String>,
    pub log_level: String,
    /// Built admin app (`web/dist-admin`, ADR-022); `None` = JSON API only.
    pub static_dir: Option<std::path::PathBuf>,
    /// Origin of the admin page as the browser sees it (`http://localhost:9100` through
    /// the SSH tunnel). Wallets refuse to sign when the message's domain or URI differ
    /// from the page origin. `None` = `https://admin.<siws domain>`.
    pub admin_origin: Option<String>,
    /// Second factor per wallet (`CF_ADMIN_TOTP_SECRETS`, ADR-023), keyed `sol:<address>`;
    /// a wallet listed here must confirm a code after its signature.
    pub totp_secrets: HashMap<String, TotpSecret>,
    /// Every admin wallet must have a secret: always in prod, elsewhere with
    /// `CF_ADMIN_TOTP_REQUIRED=1`. Checked at startup so nobody is locked out silently.
    pub totp_required: bool,
}

impl AdminConfig {
    pub fn from_env() -> Result<Self, ConfigError> {
        let vars: HashMap<String, String> = std::env::vars().collect();
        Self::from_map(&vars)
    }

    pub fn from_map(vars: &HashMap<String, String>) -> Result<Self, ConfigError> {
        let get = |key: &'static str| vars.get(key).map(String::as_str).filter(|v| !v.is_empty());
        let required = |key: &'static str| get(key).map(str::to_owned).ok_or(ConfigError::Missing(key));
        let env = match get("CF_ENV").unwrap_or("local") {
            "local" => Env::Local,
            "staging" => Env::Staging,
            "prod" => Env::Prod,
            other => return Err(ConfigError::Invalid("CF_ENV", other.to_owned())),
        };
        let bind: SocketAddr = get("CF_ADMIN_BIND")
            .unwrap_or("127.0.0.1:9100")
            .parse()
            .map_err(|e: std::net::AddrParseError| ConfigError::Invalid("CF_ADMIN_BIND", e.to_string()))?;
        if !bind.ip().is_loopback() {
            return Err(ConfigError::Invalid(
                "CF_ADMIN_BIND",
                "must be a loopback address; reach the admin through an SSH tunnel (RUNBOOK)".to_owned(),
            ));
        }
        // The single-secret name of the deferred design must not pass for the real thing.
        if get("CF_ADMIN_TOTP_SECRET").is_some() {
            return Err(ConfigError::Invalid(
                "CF_ADMIN_TOTP_SECRET",
                "use CF_ADMIN_TOTP_SECRETS=sol:<wallet>=<base32>,… — one secret per admin wallet (ADR-023)"
                    .to_owned(),
            ));
        }
        let admin_wallets = parse_wallets(&required("CF_ADMIN_WALLETS")?)?;
        let (totp_secrets, totp_required) = parse_totp(
            get("CF_ADMIN_TOTP_SECRETS"),
            get("CF_ADMIN_TOTP_REQUIRED"),
            &admin_wallets,
            env,
        )?;
        let cookie_secret = required("CF_COOKIE_SECRET")?;
        if cookie_secret.len() < 32 {
            return Err(ConfigError::Invalid(
                "CF_COOKIE_SECRET",
                "must be at least 32 characters".to_owned(),
            ));
        }
        Ok(Self {
            env,
            bind,
            database_url: Secret::new(required("CF_DATABASE_URL")?),
            redis_url: Secret::new(required("CF_REDIS_URL")?),
            cookie_secret: Secret::new(cookie_secret),
            siws_domain: required("CF_SIWS_DOMAIN")?,
            admin_wallets,
            log_level: get("CF_LOG_LEVEL").unwrap_or("info").to_owned(),
            static_dir: get("CF_ADMIN_STATIC_DIR").map(std::path::PathBuf::from),
            admin_origin: parse_origin(get("CF_ADMIN_ORIGIN"))?,
            totp_secrets,
            totp_required,
        })
    }

    /// The second-factor secret of a signed-in wallet (`sol:<address>`), if it has one.
    pub fn totp_secret(&self, wallet: &str) -> Option<&TotpSecret> {
        self.totp_secrets.get(wallet)
    }

    /// Only Solana wallets from the allowlist may sign in.
    pub fn is_admin(&self, wallet: &WalletTag) -> bool {
        wallet.kind == WalletKind::Sol && self.admin_wallets.contains(&wallet.address)
    }
}

fn parse_origin(raw: Option<&str>) -> Result<Option<String>, ConfigError> {
    match raw {
        None => Ok(None),
        Some(o) if (o.starts_with("http://") || o.starts_with("https://")) && !o.ends_with('/') => {
            Ok(Some(o.to_owned()))
        }
        Some(o) => Err(ConfigError::Invalid(
            "CF_ADMIN_ORIGIN",
            format!("expected http(s)://host[:port] without a trailing slash, got {o}"),
        )),
    }
}

/// `CF_ADMIN_TOTP_SECRETS`: comma-separated `sol:<wallet>=<base32>`. Every wallet must be
/// an admin; in prod (or with `CF_ADMIN_TOTP_REQUIRED=1`) every admin must have a secret.
fn parse_totp(
    raw: Option<&str>,
    required_flag: Option<&str>,
    wallets: &[String],
    env: Env,
) -> Result<(HashMap<String, TotpSecret>, bool), ConfigError> {
    let invalid = |why: String| ConfigError::Invalid("CF_ADMIN_TOTP_SECRETS", why);
    let mut secrets = HashMap::new();
    let entries = raw
        .unwrap_or("")
        .split(',')
        .map(str::trim)
        .filter(|e| !e.is_empty());
    for (i, entry) in entries.enumerate() {
        // Messages name the entry, never its content: a swapped `<base32>=sol:<wallet>`
        // would otherwise put the secret into the startup log.
        let (wallet, b32) = entry
            .split_once('=')
            .ok_or_else(|| invalid(format!("entry #{}: expected sol:<wallet>=<base32>", i + 1)))?;
        let address = wallet.trim().strip_prefix("sol:").unwrap_or(wallet.trim());
        let tag = WalletTag::parse(&format!("sol:{address}")).map_err(|_| {
            invalid(format!(
                "entry #{}: the part before '=' is not a Solana public key",
                i + 1
            ))
        })?;
        if !wallets.contains(&tag.address) {
            return Err(invalid(format!("{} is not in CF_ADMIN_WALLETS", tag.address)));
        }
        let secret = TotpSecret::from_base32(b32.trim()).ok_or_else(|| {
            invalid(format!(
                "secret for {} must be base32 of at least 16 bytes",
                tag.address
            ))
        })?;
        if secrets.insert(tag.to_string(), secret).is_some() {
            return Err(invalid(format!("{} is listed twice", tag.address)));
        }
    }
    let flag = match required_flag.map(str::to_ascii_lowercase).as_deref() {
        None | Some("0") | Some("false") => false,
        Some("1") | Some("true") => true,
        Some(other) => return Err(ConfigError::Invalid("CF_ADMIN_TOTP_REQUIRED", other.to_owned())),
    };
    let required = flag || env == Env::Prod;
    if required {
        if let Some(missing) = wallets
            .iter()
            .find(|w| !secrets.contains_key(&format!("sol:{w}")))
        {
            return Err(invalid(format!(
                "TOTP is required (prod or CF_ADMIN_TOTP_REQUIRED=1) but {missing} has no secret"
            )));
        }
    }
    Ok((secrets, required))
}

/// Comma-separated base58 keys (a `sol:` prefix is tolerated); each must be a real 32-byte key.
fn parse_wallets(raw: &str) -> Result<Vec<String>, ConfigError> {
    let mut wallets = Vec::new();
    for part in raw.split(',').map(str::trim).filter(|p| !p.is_empty()) {
        let address = part.strip_prefix("sol:").unwrap_or(part);
        let tag = WalletTag::parse(&format!("sol:{address}")).map_err(|_| {
            ConfigError::Invalid("CF_ADMIN_WALLETS", format!("not a Solana public key: {address}"))
        })?;
        wallets.push(tag.address);
    }
    if wallets.is_empty() {
        return Err(ConfigError::Missing("CF_ADMIN_WALLETS"));
    }
    Ok(wallets)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn base() -> HashMap<String, String> {
        [
            ("CF_DATABASE_URL", "postgres://x"),
            ("CF_REDIS_URL", "redis://x"),
            ("CF_COOKIE_SECRET", "test-cookie-secret-test-cookie-secret-0000"),
            ("CF_SIWS_DOMAIN", "localhost"),
            (
                "CF_ADMIN_WALLETS",
                "sol:11111111111111111111111111111111, 11111111111111111111111111111112",
            ),
        ]
        .into_iter()
        .map(|(k, v)| (k.to_owned(), v.to_owned()))
        .collect()
    }

    #[test]
    fn parses_allowlist_and_defaults() {
        let config = AdminConfig::from_map(&base()).map_err(|e| e.to_string());
        assert!(config.is_ok(), "{config:?}");
        let Ok(config) = config else { return };
        assert_eq!(config.admin_wallets.len(), 2);
        assert_eq!(config.bind.port(), 9100);
        assert!(config.bind.ip().is_loopback());
    }

    #[test]
    fn refuses_public_bind_totp_and_bad_wallets() {
        let mut vars = base();
        vars.insert("CF_ADMIN_BIND".into(), "0.0.0.0:9100".into());
        assert!(AdminConfig::from_map(&vars).is_err(), "public bind");
        let mut vars = base();
        vars.insert("CF_ADMIN_TOTP_SECRET".into(), "abc".into());
        assert!(AdminConfig::from_map(&vars).is_err(), "old single-secret name");
        let mut vars = base();
        vars.insert("CF_ADMIN_WALLETS".into(), "not-a-key".into());
        assert!(AdminConfig::from_map(&vars).is_err(), "bad wallet");
        let mut vars = base();
        vars.insert("CF_ADMIN_WALLETS".into(), " , ".into());
        assert!(AdminConfig::from_map(&vars).is_err(), "empty allowlist");
    }

    const B32: &str = "GEZDGNBVGY3TQOJQGEZDGNBVGY3TQOJQ";

    #[test]
    fn totp_secrets_are_per_admin_and_required_in_prod() {
        let with = |entries: &str, extra: &[(&str, &str)]| {
            let mut vars = base();
            vars.insert("CF_ADMIN_TOTP_SECRETS".into(), entries.to_owned());
            for (k, v) in extra {
                vars.insert((*k).to_owned(), (*v).to_owned());
            }
            AdminConfig::from_map(&vars).map_err(|e| e.to_string())
        };
        let one = with(&format!("sol:11111111111111111111111111111111={B32}"), &[]);
        assert!(one.is_ok(), "{one:?}");
        let Ok(one) = one else { return };
        assert_eq!(one.totp_secrets.len(), 1);
        assert!(!one.totp_required);
        assert!(one.totp_secret("sol:11111111111111111111111111111111").is_some());
        assert!(one.totp_secret("sol:11111111111111111111111111111112").is_none());
        assert!(!format!("{one:?}").contains(B32), "debug never shows a secret");

        let prod = with(
            &format!("11111111111111111111111111111111={B32}"),
            &[("CF_ENV", "prod")],
        );
        assert!(prod.is_err(), "prod: every admin needs a secret: {prod:?}");
        let required = with(
            &format!("11111111111111111111111111111111={B32}"),
            &[("CF_ADMIN_TOTP_REQUIRED", "1")],
        );
        assert!(required.is_err(), "required flag: {required:?}");
        let both = with(
            &format!("11111111111111111111111111111111={B32}, sol:11111111111111111111111111111112={B32}"),
            &[("CF_ENV", "prod")],
        );
        assert!(both.is_ok(), "{both:?}");
        assert!(both.map(|c| c.totp_required).unwrap_or(false));

        assert!(
            with(&format!("sol:11111111111111111111111111111113={B32}"), &[]).is_err(),
            "not an admin"
        );
        assert!(
            with("sol:11111111111111111111111111111111=GEZDGNBV", &[]).is_err(),
            "too short"
        );
        assert!(
            with("sol:11111111111111111111111111111111", &[]).is_err(),
            "no secret"
        );
        assert!(
            with(
                &format!("sol:11111111111111111111111111111111={B32},11111111111111111111111111111111={B32}"),
                &[]
            )
            .is_err(),
            "twice"
        );
        assert!(
            with("", &[("CF_ADMIN_TOTP_REQUIRED", "maybe")]).is_err(),
            "bad flag"
        );
        let swapped = with(&format!("{B32}=sol:11111111111111111111111111111111"), &[]);
        assert!(
            matches!(&swapped, Err(msg) if !msg.contains(B32)),
            "a swapped entry fails without echoing the secret: {swapped:?}"
        );
    }
}
