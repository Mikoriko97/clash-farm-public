//! Jupiter price oracle with a 15 s cache and sanity checks
//! (rules/server-payments.md §Оракул): stale > 60 s or a jump > 30 % against
//! the 10-minute median pauses payments until an operator resumes them.

use std::collections::VecDeque;
use std::str::FromStr;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use rust_decimal::Decimal;

use crate::error::PaymentsError;

const CACHE_TTL: Duration = Duration::from_secs(15);
const STALE_AFTER: Duration = Duration::from_secs(60);
const HISTORY_WINDOW: Duration = Duration::from_secs(600);
const MAX_DEVIATION_BP: i64 = 3000;
const MAX_BODY_BYTES: usize = 64 * 1024;

#[derive(Debug, Default)]
struct OracleState {
    last: Option<(Instant, Decimal)>,
    history: VecDeque<(Instant, Decimal)>,
    paused: Option<String>,
}

#[derive(Clone)]
pub struct PriceOracle {
    http: reqwest::Client,
    url: String,
    mint: String,
    state: Arc<Mutex<OracleState>>,
    /// Fake payments (local only): a constant USD price instead of Jupiter.
    fixed: Option<Decimal>,
}

impl std::fmt::Debug for PriceOracle {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("PriceOracle")
            .field("mint", &self.mint)
            .finish_non_exhaustive()
    }
}

impl PriceOracle {
    pub fn new(url: &str, mint: &str) -> Result<Self, PaymentsError> {
        let http = reqwest::Client::builder()
            .timeout(Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|e| PaymentsError::NeedsReview(e.to_string()))?;
        Ok(Self {
            http,
            url: url.to_owned(),
            mint: mint.to_owned(),
            state: Arc::new(Mutex::new(OracleState::default())),
            fixed: None,
        })
    }

    /// Oracle that always answers `usd_per_clash` (fake payments, `CF_ENV=local` only).
    pub fn fixed(usd_per_clash: Decimal, mint: &str) -> Result<Self, PaymentsError> {
        let mut oracle = Self::new("http://127.0.0.1:9/unused", mint)?;
        oracle.fixed = Some(usd_per_clash);
        Ok(oracle)
    }

    /// Reason payments are paused, if any (operator resumes via `resume`).
    pub fn paused(&self) -> Option<String> {
        self.state
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .paused
            .clone()
    }

    pub fn resume(&self) {
        self.state.lock().unwrap_or_else(|e| e.into_inner()).paused = None;
    }

    /// USD per one whole CLASH. Never returns a stale or implausible value.
    pub async fn usd_per_clash(&self) -> Result<Decimal, PaymentsError> {
        if let Some(price) = self.fixed {
            return Ok(price);
        }
        if let Some(reason) = self.paused() {
            tracing::warn!(%reason, "oracle paused");
            return Err(PaymentsError::OracleUnavailable);
        }
        if let Some((at, price)) = self.state.lock().unwrap_or_else(|e| e.into_inner()).last {
            if at.elapsed() < CACHE_TTL {
                return Ok(price);
            }
        }
        match self.fetch().await {
            Ok(price) => self.accept(price),
            Err(err) => {
                tracing::warn!(error = %err, "oracle fetch failed");
                let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
                match state.last {
                    Some((at, price)) if at.elapsed() < STALE_AFTER => Ok(price),
                    Some(_) => {
                        // Stale > 60 s: pause until an operator resumes (rules/server-payments.md §Оракул).
                        let reason = "price stale for more than 60 s".to_owned();
                        tracing::error!(%reason, "oracle paused");
                        state.paused = Some(reason);
                        Err(PaymentsError::OracleUnavailable)
                    }
                    None => Err(PaymentsError::OracleUnavailable),
                }
            }
        }
    }

    fn accept(&self, price: Decimal) -> Result<Decimal, PaymentsError> {
        let now = Instant::now();
        let mut state = self.state.lock().unwrap_or_else(|e| e.into_inner());
        while state
            .history
            .front()
            .is_some_and(|(at, _)| now.duration_since(*at) > HISTORY_WINDOW)
        {
            state.history.pop_front();
        }
        if let Some(median) = median(&state.history) {
            if deviation_bp(price, median) > MAX_DEVIATION_BP {
                let reason =
                    format!("price deviates >30% from 10-minute median (median {median}, now {price})");
                tracing::error!(%reason, "oracle sanity check failed; payments paused");
                state.paused = Some(reason);
                return Err(PaymentsError::OracleUnavailable);
            }
        }
        state.history.push_back((now, price));
        state.last = Some((now, price));
        Ok(price)
    }

    async fn fetch(&self) -> Result<Decimal, String> {
        let url = if self.url.contains('?') {
            format!("{}&ids={}", self.url, self.mint)
        } else {
            format!("{}?ids={}", self.url, self.mint)
        };
        let mut response = self
            .http
            .get(&url)
            .send()
            .await
            .map_err(|e| redact(&e.to_string()))?;
        if !response.status().is_success() {
            return Err(format!("http {}", response.status()));
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response.chunk().await.map_err(|e| redact(&e.to_string()))? {
            if bytes.len() + chunk.len() > MAX_BODY_BYTES {
                return Err("response too large".to_owned());
            }
            bytes.extend_from_slice(&chunk);
        }
        let value: serde_json::Value = serde_json::from_slice(&bytes).map_err(|e| e.to_string())?;
        parse_price(&value, &self.mint).ok_or_else(|| "price missing".to_owned())
    }
}

/// Accepts Jupiter v2 (`data.<mint>.price`) and v3 (`<mint>.usdPrice`) shapes; strings preferred.
pub fn parse_price(value: &serde_json::Value, mint: &str) -> Option<Decimal> {
    let entry = value
        .get("data")
        .and_then(|d| d.get(mint))
        .or_else(|| value.get(mint))?;
    let raw = entry.get("price").or_else(|| entry.get("usdPrice"))?;
    let price = match raw {
        serde_json::Value::String(s) => Decimal::from_str(s).ok()?,
        serde_json::Value::Number(n) => Decimal::from_str(&n.to_string()).ok()?,
        _ => return None,
    };
    (price > Decimal::ZERO).then_some(price)
}

fn median(history: &VecDeque<(Instant, Decimal)>) -> Option<Decimal> {
    if history.is_empty() {
        return None;
    }
    let mut prices: Vec<Decimal> = history.iter().map(|(_, p)| *p).collect();
    prices.sort();
    Some(prices[prices.len() / 2])
}

fn deviation_bp(price: Decimal, median: Decimal) -> i64 {
    if median <= Decimal::ZERO {
        return i64::MAX;
    }
    let diff = (price - median).abs() / median * Decimal::from(10_000);
    rust_decimal::prelude::ToPrimitive::to_i64(&diff.round()).unwrap_or(i64::MAX)
}

fn redact(message: &str) -> String {
    message
        .split(" for url")
        .next()
        .unwrap_or("transport error")
        .to_owned()
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;
    use serde_json::json;

    #[test]
    fn parses_v2_and_v3_shapes() {
        let mint = "9mM1Mc4Ta9UJJ32v5qsHef91PiXi7EWyiSsqF5WXpump";
        assert_eq!(
            parse_price(&json!({ "data": { mint: { "price": "0.0031" } } }), mint),
            Some(Decimal::from_str("0.0031").unwrap())
        );
        assert_eq!(
            parse_price(&json!({ mint: { "usdPrice": 0.0031 } }), mint),
            Some(Decimal::from_str("0.0031").unwrap())
        );
        assert_eq!(
            parse_price(&json!({ "data": { mint: { "price": "0" } } }), mint),
            None
        );
        assert_eq!(parse_price(&json!({ "data": {} }), mint), None);
    }

    #[test]
    fn deviation_check_pauses_on_a_jump() {
        let oracle = PriceOracle::new("http://127.0.0.1:9/price", "m").unwrap();
        oracle.accept(Decimal::from_str("0.0030").unwrap()).unwrap();
        oracle.accept(Decimal::from_str("0.0031").unwrap()).unwrap();
        assert!(oracle.accept(Decimal::from_str("0.0032").unwrap()).is_ok());
        assert!(matches!(
            oracle.accept(Decimal::from_str("0.0050").unwrap()),
            Err(PaymentsError::OracleUnavailable)
        ));
        assert!(oracle.paused().is_some());
        oracle.resume();
        assert!(oracle.paused().is_none());
    }
}
