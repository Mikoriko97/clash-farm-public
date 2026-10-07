use thiserror::Error;

/// Every failure mode of the auth zone. The API layer maps these to stable
/// error codes (docs/API.md); messages here are for logs, not for parsing.
#[derive(Debug, Error)]
pub enum AuthError {
    #[error("invalid wallet identifier")]
    InvalidWallet,
    #[error("invalid sign-in message: {0}")]
    InvalidMessage(&'static str),
    #[error("nonce missing, expired or already used")]
    NonceInvalid,
    #[error("signature does not verify")]
    BadSignature,
    #[error("message outside its validity window")]
    Expired,
    #[error("message domain does not match this server")]
    WrongDomain,
    #[error("chain not allowed for login")]
    ChainNotAllowed,
    #[error("session missing, expired or revoked")]
    SessionInvalid,
    #[error("refresh token reuse detected; session family revoked")]
    RefreshReuse,
    #[error("registrations are disabled")]
    RegistrationsDisabled,
    /// `users.banned_at` is set (admin panel, 0020): no sign-in, no refresh.
    #[error("wallet is banned")]
    Banned,
    #[error("rate limited")]
    RateLimited,
    /// `guests_enabled` is off (0028): no new guest islands for now.
    #[error("guest play is disabled")]
    GuestsDisabled,
    /// Linking (0028): the wallet already has its own account; accounts are never merged.
    #[error("wallet already has an account")]
    WalletTaken,
    /// Linking (0028): this account already has a wallet.
    #[error("account already has a wallet")]
    AlreadyLinked,
    #[error("database error")]
    Db(#[from] sqlx::Error),
    #[error("redis error")]
    Redis(#[from] redis::RedisError),
}
