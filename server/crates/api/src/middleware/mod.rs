//! Middleware order (rules/server-auth.md): request-id → body-limit → kill switches
//! → CSRF → session (extractor in handlers). Rate limits live in the auth handlers
//! because they key on wallet/IP that only the handler knows.

pub mod csrf;
pub mod kill_switch;
pub mod request_id;
pub mod session;

use axum::http::HeaderMap;
use cf_auth::session::ClientFingerprint;
use cf_common::hashing::keyed_sha256;
use time::OffsetDateTime;

/// Client IP as appended by our reverse proxy: the **last** `X-Forwarded-For`
/// entry (earlier entries are client-controlled). Without the header (no proxy,
/// local dev) everyone shares the "local" bucket.
pub fn client_ip(headers: &HeaderMap) -> String {
    headers
        .get("x-forwarded-for")
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.rsplit(',').next())
        .map(|v| v.trim().to_owned())
        .filter(|v| !v.is_empty() && v.len() <= 64)
        .unwrap_or_else(|| "local".to_owned())
}

/// Bucket for per-address caps that a single client must not multiply (guest islands, 0028): an IPv6 client
/// usually holds a whole /64, so the cap counts the /64; IPv4 (also IPv4-mapped IPv6) and "local" stay as they are.
pub fn ip_bucket(ip: &str) -> String {
    match ip.parse::<std::net::IpAddr>() {
        Ok(std::net::IpAddr::V6(v6)) => match v6.to_ipv4_mapped() {
            Some(v4) => v4.to_string(),
            None => {
                let s = v6.segments();
                format!("{:x}:{:x}:{:x}:{:x}::/64", s[0], s[1], s[2], s[3])
            }
        },
        _ => ip.to_owned(),
    }
}

/// Hashes IP and user agent with a salt that rotates daily (docs/SECURITY.md §8),
/// so logs and `sessions` never carry raw client identifiers.
/// Milliseconds since the epoch for the sliding-window limiter.
pub fn now_ms() -> i64 {
    let now = OffsetDateTime::now_utc();
    i64::try_from(now.unix_timestamp_nanos() / 1_000_000).unwrap_or(i64::MAX)
}

pub fn fingerprint(headers: &HeaderMap, secret: &str) -> ClientFingerprint {
    let today = OffsetDateTime::now_utc().date();
    let salt = format!("{secret}:{today}");
    let prev_salt = today.previous_day().map(|d| format!("{secret}:{d}"));
    let ua = headers
        .get("user-agent")
        .and_then(|v| v.to_str().ok())
        .unwrap_or("");
    ClientFingerprint {
        ip_hash: Some(keyed_sha256(salt.as_bytes(), client_ip(headers).as_bytes()).to_vec()),
        ua_hash: Some(keyed_sha256(salt.as_bytes(), ua.as_bytes()).to_vec()),
        ua_hash_prev: prev_salt.map(|s| keyed_sha256(s.as_bytes(), ua.as_bytes()).to_vec()),
    }
}

#[cfg(test)]
mod tests {
    use super::{fingerprint, ip_bucket};
    use axum::http::HeaderMap;

    #[test]
    fn a_refresh_just_after_midnight_still_knows_the_browser() {
        let mut headers = HeaderMap::new();
        headers.insert("user-agent", axum::http::HeaderValue::from_static("Firefox/131"));
        let now = fingerprint(&headers, "secret");
        // A token rotated before 00:00 UTC carries yesterday's hash: today it is `ua_hash_prev`.
        let yesterday = now.ua_hash_prev.clone();
        assert_ne!(yesterday, now.ua_hash, "the salt changes daily");
        assert!(now.same_browser(now.ua_hash.as_deref()));
        assert!(now.same_browser(yesterday.as_deref()));
        let mut other = HeaderMap::new();
        other.insert("user-agent", axum::http::HeaderValue::from_static("Chrome/129"));
        let theirs = fingerprint(&other, "secret");
        assert!(!now.same_browser(theirs.ua_hash.as_deref()), "another browser");
        assert!(!now.same_browser(theirs.ua_hash_prev.as_deref()));
        assert!(!now.same_browser(None), "a token without a hash never matches");
    }

    #[test]
    fn ipv6_counts_per_64_and_ipv4_per_address() {
        assert_eq!(ip_bucket("2001:db8:1:2:aaaa::1"), "2001:db8:1:2::/64");
        assert_eq!(
            ip_bucket("2001:db8:1:2:ffff:ffff:ffff:ffff"),
            ip_bucket("2001:db8:1:2::9"),
            "one /64 is one client"
        );
        assert_ne!(ip_bucket("2001:db8:1:3::1"), ip_bucket("2001:db8:1:2::1"));
        assert_eq!(ip_bucket("203.0.113.7"), "203.0.113.7");
        assert_eq!(
            ip_bucket("::ffff:203.0.113.7"),
            "203.0.113.7",
            "IPv4-mapped is IPv4"
        );
        assert_eq!(ip_bucket("local"), "local");
        assert_eq!(ip_bucket("not an ip"), "not an ip");
    }
}
