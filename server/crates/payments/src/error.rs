use thiserror::Error;

#[derive(Debug, Error)]
pub enum PaymentsError {
    #[error("payments are disabled")]
    Disabled,
    #[error("only Solana wallets can pay in CLASH")]
    WalletNotSolana,
    #[error("item is not sold for CLASH")]
    InvalidItem,
    #[error("quantity out of range")]
    InvalidQty,
    #[error("order below the minimum")]
    BelowMinimum,
    #[error("price oracle unavailable")]
    OracleUnavailable,
    #[error("order not found")]
    NotFound,
    #[error("order is not in a state that accepts this action")]
    WrongStatus,
    #[error("quote expired")]
    QuoteExpired,
    #[error("signature malformed")]
    BadSignature,
    #[error("signature already used")]
    SignatureReused,
    #[error("transaction not visible yet")]
    Pending,
    #[error("transaction rejected: {0}")]
    Rejected(&'static str),
    #[error("order needs manual review: {0}")]
    NeedsReview(String),
    #[error("chain error")]
    Chain(#[from] cf_chain::ChainError),
    #[error("world error")]
    World(#[from] cf_world::WorldError),
    #[error("ledger error")]
    Ledger(#[from] cf_ledger::LedgerError),
    #[error("database error")]
    Db(#[from] sqlx::Error),
}
