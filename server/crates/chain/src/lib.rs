//! Read-only Solana access for payment verification (ADR-002, ADR-016, ADR-017).
//! No keys, no signing, no `solana-sdk`: a JSON-RPC client over `reqwest` with
//! strict `serde` shapes for `jsonParsed` transactions, and a pure verifier
//! that checks a transaction against what an order expects.
#![forbid(unsafe_code)]

pub mod error;
pub mod parsed;
pub mod rpc;
pub mod signature;
pub mod verify;

pub use error::ChainError;
pub use parsed::{MintInfo, TokenAccountInfo, TransactionResult};
pub use rpc::{Commitment, RpcClient, RpcConfig};
pub use verify::{verify, Expectation, RejectReason, Verified};

/// SPL Token program (classic). Token-2022 is deliberately not accepted.
pub const TOKEN_PROGRAM: &str = "TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA";
pub const MEMO_PROGRAM: &str = "MemoSq4gqABAXKb96qnH8TysNcWxMyWCqXgDLGmfcHr";
pub const COMPUTE_BUDGET_PROGRAM: &str = "ComputeBudget111111111111111111111111111111";
/// Every program a payment transaction may touch (rules/server-payments.md).
pub const ALLOWED_PROGRAMS: [&str; 3] = [COMPUTE_BUDGET_PROGRAM, TOKEN_PROGRAM, MEMO_PROGRAM];

#[cfg(test)]
mod program_id_tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;

    /// A one-character typo in a base58 id would silently reject every real payment
    /// after the burn: the constants must match shared/programs.json byte for byte.
    #[test]
    fn allowlist_matches_shared_program_ids() {
        let shared: serde_json::Value =
            serde_json::from_str(include_str!("../../../../shared/programs.json")).unwrap();
        assert_eq!(shared["compute_budget"], COMPUTE_BUDGET_PROGRAM);
        assert_eq!(shared["token"], TOKEN_PROGRAM);
        assert_eq!(shared["memo"], MEMO_PROGRAM);
        for id in ALLOWED_PROGRAMS {
            assert_eq!(bs58::decode(id).into_vec().unwrap().len(), 32, "{id}");
        }
    }
}
