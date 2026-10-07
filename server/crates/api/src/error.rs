//! Error envelope `{ "error": { "code", "message", "request_id" } }` (docs/API.md).
//! Codes are stable; messages are not for parsing. Internal details never leak.

use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::Json;
use cf_auth::AuthError;
use serde::Serialize;

#[derive(Debug, Clone)]
pub struct ApiError {
    pub status: StatusCode,
    pub code: &'static str,
    pub message: String,
    /// `Retry-After` in seconds for 429/503 (60 for a 429 when unset).
    pub retry_after: Option<u64>,
}

/// Attached to error responses so the outer middleware can add `request_id`.
#[derive(Debug, Clone)]
pub struct ErrorEnvelope {
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug, Serialize)]
pub struct ErrorBody<'a> {
    pub error: ErrorInner<'a>,
}

#[derive(Debug, Serialize)]
pub struct ErrorInner<'a> {
    pub code: &'a str,
    pub message: &'a str,
    pub request_id: Option<&'a str>,
}

impl ApiError {
    pub fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
            retry_after: None,
        }
    }

    /// A database error the player can do something about: a deadlock or serialization
    /// failure is "try again" (409 `busy`), an exhausted pool or a lost connection is a short
    /// outage (503 `unavailable`); anything else stays an internal error.
    pub fn from_db(err: &sqlx::Error) -> Self {
        match err {
            sqlx::Error::Database(db) if matches!(db.code().as_deref(), Some("40P01" | "40001")) => {
                tracing::warn!(error = %err, "database conflict, client retries");
                Self::new(StatusCode::CONFLICT, "busy", "busy, try again")
            }
            sqlx::Error::PoolTimedOut | sqlx::Error::PoolClosed | sqlx::Error::Io(_) => {
                tracing::error!(error = %err, "database unavailable");
                Self::unavailable()
            }
            _ => {
                tracing::error!(error = %err, "database error");
                Self::internal()
            }
        }
    }

    /// A short outage of Postgres or Redis (a restart, a full pool): the client retries.
    pub fn unavailable() -> Self {
        let mut e = Self::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "unavailable",
            "the server is busy for a moment, try again",
        );
        e.retry_after = Some(2);
        e
    }

    /// A daily cap: it resets at 00:00 UTC, and the message says when.
    pub fn daily_cap() -> Self {
        let left = seconds_until_utc_midnight();
        let mut e = Self::new(
            StatusCode::TOO_MANY_REQUESTS,
            "daily_cap",
            format!("Daily limit reached, resets in {}.", human_duration(left)),
        );
        e.retry_after = Some(left);
        e
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self::new(StatusCode::NOT_FOUND, "not_found", message)
    }

    pub fn bad_request(code: &'static str, message: impl Into<String>) -> Self {
        Self::new(StatusCode::BAD_REQUEST, code, message)
    }

    pub fn unauthorized(code: &'static str) -> Self {
        Self::new(StatusCode::UNAUTHORIZED, code, "authentication required")
    }

    pub fn internal() -> Self {
        Self::new(StatusCode::INTERNAL_SERVER_ERROR, "internal", "internal error")
    }
}

impl From<AuthError> for ApiError {
    fn from(err: AuthError) -> Self {
        use AuthError as E;
        match err {
            E::InvalidWallet => {
                Self::bad_request("invalid_wallet", "wallet must be sol:<base58> or evm:0x<hex>")
            }
            E::InvalidMessage(what) => {
                Self::bad_request("invalid_message", format!("sign-in message invalid: {what}"))
            }
            E::NonceInvalid => Self::unauthorized("nonce_invalid"),
            E::BadSignature => Self::unauthorized("bad_signature"),
            E::Expired => Self::unauthorized("message_expired"),
            E::WrongDomain => Self::unauthorized("wrong_domain"),
            E::ChainNotAllowed => Self::bad_request("chain_not_allowed", "chain is not enabled for login"),
            E::SessionInvalid => Self::unauthorized("session_invalid"),
            E::RefreshReuse => Self::unauthorized("session_revoked"),
            E::RegistrationsDisabled => Self::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "registrations_disabled",
                "registrations are paused",
            ),
            E::Banned => Self::new(StatusCode::FORBIDDEN, "banned", "this wallet is banned"),
            E::RateLimited => Self::new(StatusCode::TOO_MANY_REQUESTS, "rate_limited", "too many requests"),
            E::GuestsDisabled => Self::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "guests_disabled",
                "guest play is paused",
            ),
            E::WalletTaken => Self::new(
                StatusCode::CONFLICT,
                "wallet_has_account",
                "this wallet already has its own account; accounts are not merged",
            ),
            E::AlreadyLinked => Self::new(
                StatusCode::CONFLICT,
                "already_linked",
                "this account already has a wallet",
            ),
            E::Db(e) => Self::from_db(&e),
            E::Redis(e) => {
                tracing::error!(error = %e, "redis error");
                Self::unavailable()
            }
        }
    }
}

impl From<cf_world::WorldError> for ApiError {
    fn from(err: cf_world::WorldError) -> Self {
        use cf_world::WorldError as E;
        match err {
            E::NotFound => Self::new(StatusCode::NOT_FOUND, "not_found", "not found"),
            E::WrongState => Self::new(
                StatusCode::CONFLICT,
                "wrong_state",
                "plot is not in the required state",
            ),
            E::NotReady => Self::new(StatusCode::CONFLICT, "not_ready", "crop is not ready yet"),
            E::InsufficientItems => Self::new(StatusCode::CONFLICT, "insufficient_items", "not enough items"),
            E::OutOfBounds => Self::bad_request("out_of_bounds", "cell is outside the island"),
            E::CellTaken => Self::new(StatusCode::CONFLICT, "cell_taken", "cell already has a plot"),
            E::PlotLimit => Self::new(
                StatusCode::CONFLICT,
                "plot_limit",
                "plot limit for this level reached",
            ),
            E::BedTooLarge => Self::new(
                StatusCode::CONFLICT,
                "bed_too_large",
                "bed would exceed the size limit",
            ),
            E::Depleted => Self::new(StatusCode::CONFLICT, "depleted", "resource is depleted"),
            E::TooFar => Self::new(
                StatusCode::CONFLICT,
                "too_far",
                "stand next to it (presence) first",
            ),
            E::AlreadyDiscovered => Self::new(
                StatusCode::CONFLICT,
                "already_discovered",
                "already in the journal",
            ),
            E::AlreadyClaimed => Self::new(StatusCode::CONFLICT, "already_claimed", "reward already taken"),
            E::AlreadyFertilized => Self::new(
                StatusCode::CONFLICT,
                "already_fertilized",
                "plot already fertilized",
            ),
            E::ExclusiveLocked => Self::new(
                StatusCode::FORBIDDEN,
                "exclusive_locked",
                "exclusive item requires reputation",
            ),
            E::LevelLocked(level) => Self::new(
                StatusCode::FORBIDDEN,
                "level_locked",
                format!("requires island level {level}"),
            ),
            E::DailyCap => Self::daily_cap(),
            E::InvalidItem => Self::bad_request("invalid_item", "item cannot be used here"),
            E::InvalidQty => Self::bad_request("invalid_qty", "quantity out of range"),
            E::InvalidRotation => {
                Self::bad_request("invalid_rotation", "rotation must be a multiple of 15 degrees")
            }
            E::Overlap => Self::new(StatusCode::CONFLICT, "overlap", "overlaps another object"),
            E::DecorLimit => Self::new(StatusCode::CONFLICT, "decor_limit", "decor limit reached"),
            E::ClearingLimit => Self::new(
                StatusCode::CONFLICT,
                "clearing_limit",
                "this island has no more spots to clear",
            ),
            E::InvalidNickname(why) => Self::bad_request("invalid_nickname", why),
            E::NicknameNotAllowed => Self::bad_request("nickname_not_allowed", "that name is not allowed"),
            E::NicknameTaken => Self::new(StatusCode::CONFLICT, "nickname_taken", "that name is taken"),
            E::CharacterExists => Self::new(
                StatusCode::CONFLICT,
                "character_exists",
                "this account already has a character",
            ),
            E::InvalidLook => Self::bad_request("invalid_look", "look is outside the allowed choices"),
            E::NoCharacter => Self::new(StatusCode::CONFLICT, "no_character", "create the character first"),
            E::SameLook => Self::new(StatusCode::CONFLICT, "same_look", "that is already the look"),
            E::NoRestyleTicket => Self::new(StatusCode::CONFLICT, "no_ticket", "a restyle ticket is needed"),
            E::SameNickname => Self::new(StatusCode::CONFLICT, "same_nickname", "that is already the name"),
            E::NoRenameTicket => Self::new(StatusCode::CONFLICT, "no_ticket", "a rename ticket is needed"),
            E::InvalidSide => Self::bad_request("invalid_side", "side must be n, e, s or w"),
            E::ExpandMax => Self::new(StatusCode::CONFLICT, "expand_max", "this side is fully expanded"),
            E::NoStorm => Self::new(StatusCode::CONFLICT, "no_storm", "no storm right now"),
            E::InsufficientFunds => {
                Self::new(StatusCode::CONFLICT, "insufficient_funds", "not enough credits")
            }
            E::Ledger(e) => {
                tracing::error!(error = %e, "ledger posting rejected");
                Self::internal()
            }
            E::Db(e) => Self::from_db(&e),
        }
    }
}

impl From<cf_payments::PaymentsError> for ApiError {
    fn from(err: cf_payments::PaymentsError) -> Self {
        use cf_payments::PaymentsError as E;
        match err {
            E::Disabled => Self::new(StatusCode::CONFLICT, "payments_disabled", "payments are disabled"),
            E::WalletNotSolana => Self::new(
                StatusCode::CONFLICT,
                "wallet_not_solana",
                "CLASH payments need a Solana wallet",
            ),
            E::InvalidItem => Self::bad_request("invalid_item", "item is not sold for CLASH"),
            E::InvalidQty => Self::bad_request("invalid_qty", "quantity out of range"),
            E::BelowMinimum => Self::bad_request("below_minimum", "order below the minimum amount"),
            E::OracleUnavailable => Self::new(
                StatusCode::SERVICE_UNAVAILABLE,
                "oracle_unavailable",
                "price unavailable, try later",
            ),
            E::NotFound => Self::new(StatusCode::NOT_FOUND, "not_found", "not found"),
            E::WrongStatus => Self::new(StatusCode::CONFLICT, "wrong_status", "order is not open"),
            E::QuoteExpired => Self::new(
                StatusCode::CONFLICT,
                "quote_expired",
                "quote expired; create a new order",
            ),
            E::BadSignature => Self::bad_request("bad_signature", "signature must be base58 of 64 bytes"),
            E::SignatureReused => Self::new(StatusCode::CONFLICT, "sig_reused", "signature already used"),
            E::Pending => Self::new(StatusCode::ACCEPTED, "submitted", "transaction not visible yet"),
            E::Rejected(code) => Self::new(StatusCode::CONFLICT, code, "transaction rejected"),
            E::NeedsReview(_) => Self::new(
                StatusCode::CONFLICT,
                "needs_review",
                "order parked for manual review",
            ),
            E::World(e) => Self::from(e),
            E::Ledger(e) => Self::from(cf_world::WorldError::from(e)),
            E::Chain(e) => {
                tracing::error!(error = %e, "chain error");
                Self::new(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "chain_unavailable",
                    "chain unavailable, try later",
                )
            }
            E::Db(e) => Self::from_db(&e),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let body = ErrorBody {
            error: ErrorInner {
                code: self.code,
                message: &self.message,
                request_id: None,
            },
        };
        let mut response = (self.status, Json(body)).into_response();
        response.extensions_mut().insert(ErrorEnvelope {
            code: self.code,
            message: self.message,
        });
        let retry = match (self.status, self.retry_after) {
            (_, Some(secs)) => Some(secs),
            (StatusCode::TOO_MANY_REQUESTS, None) => Some(60),
            _ => None,
        };
        if let Some(secs) = retry {
            response
                .headers_mut()
                .insert("retry-after", axum::http::HeaderValue::from(secs));
        }
        response
    }
}

/// Seconds left until the next 00:00 UTC (daily caps count by UTC day).
pub fn seconds_until_utc_midnight() -> u64 {
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs())
        .unwrap_or(0);
    86_400 - now % 86_400
}

/// "5 h 12 min", "12 min", "under a minute".
pub fn human_duration(secs: u64) -> String {
    let (h, m) = (secs / 3600, secs % 3600 / 60);
    match (h, m) {
        (0, 0) => "under a minute".to_owned(),
        (0, m) => format!("{m} min"),
        (h, 0) => format!("{h} h"),
        (h, m) => format!("{h} h {m} min"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn durations_read_like_a_person_wrote_them() {
        assert_eq!(human_duration(30), "under a minute");
        assert_eq!(human_duration(12 * 60 + 5), "12 min");
        assert_eq!(human_duration(5 * 3600), "5 h");
        assert_eq!(human_duration(5 * 3600 + 12 * 60), "5 h 12 min");
        let left = seconds_until_utc_midnight();
        assert!((1..=86_400).contains(&left));
    }

    #[test]
    fn a_daily_cap_says_when_it_resets() {
        let e = ApiError::daily_cap();
        assert_eq!((e.status, e.code), (StatusCode::TOO_MANY_REQUESTS, "daily_cap"));
        assert!(
            e.message.starts_with("Daily limit reached, resets in "),
            "{}",
            e.message
        );
        assert!(e.retry_after.is_some_and(|s| s <= 86_400));
    }

    #[test]
    fn outages_are_503_and_unknown_db_errors_stay_internal() {
        assert_eq!(
            ApiError::from_db(&sqlx::Error::PoolTimedOut).status,
            StatusCode::SERVICE_UNAVAILABLE
        );
        assert_eq!(
            ApiError::from_db(&sqlx::Error::RowNotFound).status,
            StatusCode::INTERNAL_SERVER_ERROR
        );
        assert_eq!(ApiError::unavailable().retry_after, Some(2));
    }
}
