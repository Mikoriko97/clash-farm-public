//! Authentication: Sign-In-With-Solana / Sign-In-With-Ethereum messages, signature
//! verification, one-shot nonces (Redis), cookie sessions with refresh rotation
//! (Postgres + Redis) and a sliding-window rate limiter.
//! Rules: docs/SECURITY.md §3, docs/SECURITY.md.
#![forbid(unsafe_code)]

pub mod error;
pub mod message;
pub mod nonce;
pub mod rate;
pub mod session;
pub mod user;
pub mod verify;
pub mod wallet;

pub use error::AuthError;
pub use message::{SignInMessage, STATEMENT};
pub use nonce::NonceStore;
pub use session::{IssuedSession, Principal, SessionService};
pub use wallet::{WalletKind, WalletTag};
