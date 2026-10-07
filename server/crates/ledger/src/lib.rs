//! Double-entry, append-only ledger (docs/DATA_MODEL.md §3, ADR-006).
//! Every economic effect is one `post()` inside the caller's transaction:
//! ≥ 2 rows whose deltas sum to zero per currency, plus the materialised
//! `balances` rows updated in the same transaction. `balances.amount >= 0`
//! is enforced by the database; a violation surfaces as `InsufficientFunds`.
#![forbid(unsafe_code)]

pub mod error;
pub mod post;
pub mod query;

pub use error::LedgerError;
pub use post::{post, Currency, Entry};
pub use query::{balance, balances, Balance};

pub const ACCOUNT_CREDITS: &str = "credits";
pub const SYSTEM_MINT: &str = "system:mint";
pub const SYSTEM_SINK: &str = "system:sink";
pub const SYSTEM_BURN_CLASH: &str = "system:burn_clash";
