use thiserror::Error;

#[derive(Debug, Error)]
pub enum LedgerError {
    #[error("posting must contain at least two entries")]
    EmptyPosting,
    #[error("posting does not balance for currency {0}")]
    Unbalanced(&'static str),
    #[error("entry delta must be non-zero")]
    ZeroDelta,
    #[error("system accounts must start with system:, user accounts must not")]
    BadAccount,
    #[error("insufficient funds")]
    InsufficientFunds,
    #[error("database error")]
    Db(#[from] sqlx::Error),
}

/// Postgres `check_violation` — the only way `balances.amount >= 0` can fail.
pub const CHECK_VIOLATION: &str = "23514";
