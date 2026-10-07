//! One-shot login nonces in Redis: 32 CSPRNG bytes, TTL 5 minutes, consumed
//! atomically with `GETDEL` and bound to the wallet that requested them.

use rand::Rng;
use redis::aio::ConnectionManager;

use crate::error::AuthError;

pub const NONCE_TTL_SECS: u64 = 300;

#[derive(Debug, Clone)]
pub struct NonceStore {
    redis: ConnectionManager,
}

impl NonceStore {
    pub fn new(redis: ConnectionManager) -> Self {
        Self { redis }
    }

    /// Returns a fresh 64-hex-char nonce bound to `wallet_tag`.
    pub async fn issue(&self, wallet_tag: &str) -> Result<String, AuthError> {
        let mut bytes = [0u8; 32];
        rand::rng().fill_bytes(&mut bytes);
        let nonce = hex::encode(bytes);
        let mut con = self.redis.clone();
        let set: Option<String> = redis::cmd("SET")
            .arg(key(&nonce))
            .arg(wallet_tag)
            .arg("NX")
            .arg("EX")
            .arg(NONCE_TTL_SECS)
            .query_async(&mut con)
            .await?;
        if set.is_none() {
            // 256-bit collision: practically impossible, but never silently reuse.
            return Err(AuthError::NonceInvalid);
        }
        Ok(nonce)
    }

    /// Consumes the nonce. Succeeds at most once, and only for the same wallet.
    pub async fn consume(&self, nonce: &str, wallet_tag: &str) -> Result<(), AuthError> {
        if nonce.len() != 64 || !nonce.chars().all(|c| c.is_ascii_hexdigit()) {
            return Err(AuthError::NonceInvalid);
        }
        let mut con = self.redis.clone();
        let stored: Option<String> = redis::cmd("GETDEL").arg(key(nonce)).query_async(&mut con).await?;
        match stored {
            Some(owner) if owner == wallet_tag => Ok(()),
            _ => Err(AuthError::NonceInvalid),
        }
    }
}

fn key(nonce: &str) -> String {
    format!("auth:nonce:{nonce}")
}
