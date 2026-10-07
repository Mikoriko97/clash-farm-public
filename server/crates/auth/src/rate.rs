//! Sliding-window rate limit on a Redis sorted set (docs/SECURITY.md §3).

use redis::aio::ConnectionManager;

use crate::error::AuthError;

#[derive(Debug, Clone)]
pub struct RateLimiter {
    redis: ConnectionManager,
}

impl RateLimiter {
    pub fn new(redis: ConnectionManager) -> Self {
        Self { redis }
    }

    /// Records one hit for `subject` and returns `Err(RateLimited)` when more than
    /// `limit` hits happened in the last `window_secs`. Fails open: a Redis restart (nightly OS
    /// updates restart it) must not turn every game request into an error for a few seconds;
    /// money and daily caps are counted in Postgres, sessions, nonces and idempotency fail closed.
    pub async fn hit(
        &self,
        scope: &str,
        subject: &str,
        limit: u64,
        window_secs: u64,
        now_ms: i64,
    ) -> Result<(), AuthError> {
        match self.count(scope, subject, window_secs, now_ms).await {
            Ok(count) => over(count, limit),
            Err(err) => {
                tracing::warn!(error = %err, scope, "rate limiter unavailable, request let through");
                Ok(())
            }
        }
    }

    /// Like `hit`, but a Redis error refuses the request: for limits that are a security gate
    /// on their own (admin sign-in and TOTP guesses, admin mutations, guests per address).
    pub async fn hit_strict(
        &self,
        scope: &str,
        subject: &str,
        limit: u64,
        window_secs: u64,
        now_ms: i64,
    ) -> Result<(), AuthError> {
        let count = self.count(scope, subject, window_secs, now_ms).await?;
        over(count, limit)
    }

    async fn count(
        &self,
        scope: &str,
        subject: &str,
        window_secs: u64,
        now_ms: i64,
    ) -> Result<u64, redis::RedisError> {
        let key = format!("rate:{scope}:{subject}");
        let window_ms = i64::try_from(window_secs)
            .unwrap_or(i64::MAX)
            .saturating_mul(1000);
        let member = format!("{now_ms}-{}", rand_suffix());
        let mut con = self.redis.clone();
        let (_, _, count, _): ((), (), u64, ()) = redis::pipe()
            .atomic()
            .cmd("ZREMRANGEBYSCORE")
            .arg(&key)
            .arg("-inf")
            .arg(now_ms.saturating_sub(window_ms))
            .cmd("ZADD")
            .arg(&key)
            .arg(now_ms)
            .arg(member)
            .cmd("ZCARD")
            .arg(&key)
            .cmd("EXPIRE")
            .arg(&key)
            .arg(window_secs)
            .query_async(&mut con)
            .await?;
        Ok(count)
    }
}

fn over(count: u64, limit: u64) -> Result<(), AuthError> {
    if count > limit {
        Err(AuthError::RateLimited)
    } else {
        Ok(())
    }
}

fn rand_suffix() -> u64 {
    use rand::Rng;
    rand::rng().next_u64()
}
