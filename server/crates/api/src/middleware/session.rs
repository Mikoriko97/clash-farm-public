//! `AuthUser` extractor: resolves the `cf_access` cookie to a user id. For
//! mutating requests it also binds the CSRF header to the session's token.
//! `user_id` comes from here and nowhere else (rules/server-auth.md).

use axum::extract::FromRequestParts;
use axum::http::request::Parts;
use axum::http::Method;
use axum_extra::extract::cookie::CookieJar;
use subtle::ConstantTimeEq;
use uuid::Uuid;

use crate::error::ApiError;
use crate::middleware::csrf::CSRF_HEADER;
use crate::state::AppState;

pub const ACCESS_COOKIE: &str = "cf_access";
pub const REFRESH_COOKIE: &str = "cf_refresh";

#[derive(Clone)]
pub struct AuthUser {
    pub user_id: Uuid,
    pub family: Uuid,
    pub access_token: String,
}

impl std::fmt::Debug for AuthUser {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthUser")
            .field("user_id", &self.user_id)
            .field("family", &self.family)
            .finish_non_exhaustive()
    }
}

impl cf_auth::session::SessionRef for AuthUser {
    fn family(&self) -> Uuid {
        self.family
    }
    fn access_token(&self) -> &str {
        &self.access_token
    }
}

impl FromRequestParts<AppState> for AuthUser {
    type Rejection = ApiError;

    async fn from_request_parts(parts: &mut Parts, state: &AppState) -> Result<Self, Self::Rejection> {
        let jar = CookieJar::from_headers(&parts.headers);
        let access_token = jar
            .get(ACCESS_COOKIE)
            .map(|c| c.value().to_owned())
            .ok_or_else(|| ApiError::unauthorized("session_missing"))?;
        let principal = state
            .sessions
            .authenticate(&access_token)
            .await?
            .ok_or_else(|| ApiError::unauthorized("session_invalid"))?;
        let mutating = matches!(
            parts.method,
            Method::POST | Method::PUT | Method::PATCH | Method::DELETE
        );
        if mutating {
            let header = parts
                .headers
                .get(CSRF_HEADER)
                .and_then(|v| v.to_str().ok())
                .unwrap_or("");
            let bound: bool = principal.csrf_token.as_bytes().ct_eq(header.as_bytes()).into();
            if !bound {
                return Err(ApiError::new(
                    axum::http::StatusCode::FORBIDDEN,
                    "csrf",
                    "csrf token not bound to session",
                ));
            }
        }
        Ok(Self {
            user_id: principal.user_id,
            family: principal.family,
            access_token,
        })
    }
}
