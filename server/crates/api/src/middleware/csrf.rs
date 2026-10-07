//! CSRF for mutating requests (docs/SECURITY.md §3): double-submit token
//! (`cf_csrf` cookie == `X-CSRF-Token` header, constant-time) plus Origin /
//! Sec-Fetch-Site checks. Session binding of the token happens in the
//! `AuthUser` extractor.

use axum::extract::{Request, State};
use axum::http::{HeaderMap, Method, StatusCode};
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum_extra::extract::cookie::CookieJar;
use subtle::ConstantTimeEq;

use crate::error::ApiError;
use crate::state::AppState;

pub const CSRF_COOKIE: &str = "cf_csrf";
pub const CSRF_HEADER: &str = "x-csrf-token";
/// Login endpoints run before any session exists, so they have no CSRF token yet;
/// they are protected by the nonce + signature instead (a guest island, 0028, by the
/// Origin check below and a per-IP daily cap). Paths are as seen inside the `/api/v1`
/// nest (axum strips the prefix before inner layers). `/auth/link` is NOT exempt.
const EXEMPT: &[&str] = &["/auth/nonce", "/auth/verify", "/auth/guest"];

pub async fn enforce(State(state): State<AppState>, request: Request, next: Next) -> Response {
    let mutating = matches!(
        *request.method(),
        Method::POST | Method::PUT | Method::PATCH | Method::DELETE
    );
    if !mutating {
        return next.run(request).await;
    }
    // Origin / Sec-Fetch-Site apply to every mutating request, login included
    // (blocks login-CSRF: forcing a victim into the attacker's wallet session).
    if let Err(err) = check_origin(request.headers(), &state.config.siws_domain) {
        return err.into_response();
    }
    if EXEMPT.contains(&request.uri().path()) {
        return next.run(request).await;
    }
    if let Err(err) = check_token(request.headers()) {
        return err.into_response();
    }
    next.run(request).await
}

fn check_origin(headers: &HeaderMap, domain: &str) -> Result<(), ApiError> {
    if let Some(site) = headers.get("sec-fetch-site").and_then(|v| v.to_str().ok()) {
        if !matches!(site, "same-origin" | "same-site" | "none") {
            return Err(forbidden("cross-site request"));
        }
    }
    if let Some(origin) = headers.get("origin").and_then(|v| v.to_str().ok()) {
        if origin_host(origin) != Some(domain) {
            return Err(forbidden("origin mismatch"));
        }
    }
    Ok(())
}

#[cfg(test)]
fn check(headers: &HeaderMap, domain: &str) -> Result<(), ApiError> {
    check_origin(headers, domain)?;
    check_token(headers)
}

fn check_token(headers: &HeaderMap) -> Result<(), ApiError> {
    let cookie = CookieJar::from_headers(headers)
        .get(CSRF_COOKIE)
        .map(|c| c.value().to_owned());
    let header = headers.get(CSRF_HEADER).and_then(|v| v.to_str().ok());
    match (cookie, header) {
        (Some(cookie), Some(header))
            if cookie.len() == 64 && cookie.as_bytes().ct_eq(header.as_bytes()).into() =>
        {
            Ok(())
        }
        _ => Err(forbidden("csrf token missing or mismatched")),
    }
}

/// `https://play.example:443` → `play.example`
fn origin_host(origin: &str) -> Option<&str> {
    let rest = origin.split("://").nth(1)?;
    let host = rest.split('/').next()?;
    Some(host.rsplit_once(':').map_or(host, |(h, _)| h))
}

fn forbidden(message: &'static str) -> ApiError {
    ApiError::new(StatusCode::FORBIDDEN, "csrf", message)
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;

    #[test]
    fn origin_host_strips_scheme_and_port() {
        assert_eq!(origin_host("https://play.example:443"), Some("play.example"));
        assert_eq!(origin_host("http://localhost:5173"), Some("localhost"));
        assert_eq!(origin_host("garbage"), None);
    }

    #[test]
    fn requires_matching_cookie_and_header() {
        let mut headers = HeaderMap::new();
        let token = "a".repeat(64);
        headers.insert("cookie", format!("cf_csrf={token}").parse().unwrap());
        assert!(check(&headers, "localhost").is_err());
        headers.insert(CSRF_HEADER, token.parse().unwrap());
        check(&headers, "localhost").unwrap();
        headers.insert("origin", "https://evil.example".parse().unwrap());
        assert!(check(&headers, "localhost").is_err());
        headers.insert("origin", "http://localhost:5173".parse().unwrap());
        headers.insert("sec-fetch-site", "cross-site".parse().unwrap());
        assert!(check(&headers, "localhost").is_err());
    }
}
