//! Admin errors → HTML/JSON responses. Details stay in logs; the browser sees a code.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Redirect, Response};
use cf_auth::AuthError;
use cf_payments::PaymentsError;
use thiserror::Error;

use crate::html;

#[derive(Debug, Error)]
pub enum AdminError {
    #[error("sign in required")]
    Unauthorized,
    #[error("forbidden: {0}")]
    Forbidden(&'static str),
    #[error("bad request: {0}")]
    BadRequest(String),
    #[error("not found")]
    NotFound,
    /// The action does not fit the current state (already blocked, no character yet…).
    #[error("{0}")]
    Conflict(&'static str),
    #[error("too many requests")]
    RateLimited,
    #[error("auth: {0}")]
    Auth(AuthError),
    #[error("payments: {0}")]
    Payments(PaymentsError),
    #[error("database error")]
    Db(#[from] sqlx::Error),
    #[error("redis error")]
    Redis(#[from] redis::RedisError),
}

impl From<AuthError> for AdminError {
    fn from(err: AuthError) -> Self {
        match err {
            AuthError::RateLimited => Self::RateLimited,
            AuthError::Db(e) => Self::Db(e),
            AuthError::Redis(e) => Self::Redis(e),
            other => Self::Auth(other),
        }
    }
}

impl From<PaymentsError> for AdminError {
    fn from(err: PaymentsError) -> Self {
        match err {
            PaymentsError::NotFound => Self::NotFound,
            PaymentsError::Db(e) => Self::Db(e),
            other => Self::Payments(other),
        }
    }
}

impl AdminError {
    pub fn status(&self) -> StatusCode {
        match self {
            Self::Unauthorized | Self::Auth(_) => StatusCode::UNAUTHORIZED,
            Self::Forbidden(_) => StatusCode::FORBIDDEN,
            Self::BadRequest(_) => StatusCode::BAD_REQUEST,
            Self::NotFound => StatusCode::NOT_FOUND,
            Self::Conflict(_) => StatusCode::CONFLICT,
            Self::RateLimited => StatusCode::TOO_MANY_REQUESTS,
            Self::Payments(PaymentsError::BadSignature) => StatusCode::BAD_REQUEST,
            Self::Payments(_) => StatusCode::CONFLICT,
            Self::Db(_) | Self::Redis(_) => StatusCode::INTERNAL_SERVER_ERROR,
        }
    }
}

impl AdminError {
    /// Stable machine code for the JSON API.
    pub fn code(&self) -> &'static str {
        match self {
            Self::Unauthorized | Self::Auth(_) => "unauthorized",
            Self::Forbidden(_) => "forbidden",
            Self::BadRequest(_) => "bad_request",
            Self::NotFound => "not_found",
            Self::Conflict(_) => "conflict",
            Self::RateLimited => "rate_limited",
            Self::Payments(_) => "payments",
            Self::Db(_) | Self::Redis(_) => "internal",
        }
    }

    /// What the browser may see; database and Redis details go to the log only.
    pub fn public_message(&self) -> String {
        match self {
            Self::Db(err) => {
                tracing::error!(error = %err, "admin database error");
                "internal error".to_owned()
            }
            Self::Redis(err) => {
                tracing::error!(error = %err, "admin redis error");
                "internal error".to_owned()
            }
            other => other.to_string(),
        }
    }
}

impl IntoResponse for AdminError {
    fn into_response(self) -> Response {
        if matches!(self, Self::Unauthorized) {
            return Redirect::to("/login").into_response();
        }
        let status = self.status();
        let text = self.public_message();
        let body = format!(
            "<p class=\"err\">{}</p><p><a href=\"/\">← на головну</a></p>",
            html::esc(&text)
        );
        (status, html::page("Помилка", &body)).into_response()
    }
}
