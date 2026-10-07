//! Telegram delivery (`sendMessage`, HTML parse mode) for the alerts job. The bot token
//! is part of the URL, so the URL is a `Secret` and every error is stripped of it
//! (`reqwest::Error::without_url`). Without configuration `NoChannel` keeps signals in
//! the admin only and claims no delivery.

use std::fmt;
use std::time::Duration;

use cf_common::{AlertsConfig, Secret};
use serde_json::json;

use super::Notifier;

const TIMEOUT: Duration = Duration::from_secs(10);

pub struct Telegram {
    http: reqwest::Client,
    url: Secret,
    chat_id: String,
}

impl fmt::Debug for Telegram {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("Telegram(chat ***)")
    }
}

impl Telegram {
    pub fn new(cfg: &AlertsConfig) -> Result<Self, reqwest::Error> {
        let http = reqwest::Client::builder().timeout(TIMEOUT).build()?;
        Ok(Self {
            http,
            url: Secret::new(format!(
                "https://api.telegram.org/bot{}/sendMessage",
                cfg.telegram_bot_token.expose()
            )),
            chat_id: cfg.telegram_chat_id.clone(),
        })
    }
}

impl Notifier for Telegram {
    async fn send(&self, text: &str) -> Result<bool, String> {
        let response = self
            .http
            .post(self.url.expose())
            .json(&json!({
                "chat_id": self.chat_id,
                "text": text,
                "parse_mode": "HTML",
                "disable_web_page_preview": true,
            }))
            .send()
            .await
            .map_err(|e| e.without_url().to_string())?;
        let status = response.status();
        if status.is_success() {
            Ok(true)
        } else {
            Err(format!("telegram http {status}"))
        }
    }
}

/// No channel configured: the admin page is the only reader.
#[derive(Debug)]
pub struct NoChannel;

impl Notifier for NoChannel {
    async fn send(&self, _text: &str) -> Result<bool, String> {
        Ok(false)
    }
}

/// Telegram HTML needs only these three.
pub fn esc(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    for c in s.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn debug_hides_the_token_and_escaping_covers_html() {
        let cfg = AlertsConfig {
            telegram_bot_token: Secret::new("123456:ABC-secret-token".to_owned()),
            telegram_chat_id: "-100123".to_owned(),
        };
        let tg = Telegram::new(&cfg).map_err(|e| e.without_url().to_string());
        assert!(tg.is_ok(), "{tg:?}");
        let text = format!("{:?}", tg.map(|t| format!("{t:?}")));
        assert!(!text.contains("secret-token"), "{text}");
        assert_eq!(esc("a<b>&c"), "a&lt;b&gt;&amp;c");
    }
}
