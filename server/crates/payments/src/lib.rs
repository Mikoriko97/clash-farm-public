//! CLASH payments (docs/SECURITY.md §4, ADR-004/016/017): USD-priced orders
//! quoted from the Jupiter oracle, paid by the player's own wallet with a
//! burn + treasury transfer, verified on chain, fulfilled atomically with
//! the ledger. The server never signs anything.
#![forbid(unsafe_code)]

pub mod error;
mod fulfil;
pub mod oracle;
pub mod orders;
pub mod quote;
pub mod review;
pub mod submit;

pub use error::PaymentsError;
pub use oracle::PriceOracle;
pub use orders::{OrderStatusView, OrderView};
pub use submit::{submit, SubmitOutcome};

/// Runtime pieces the API needs to take payments.
#[derive(Debug, Clone)]
pub struct PaymentsRuntime {
    pub chain: cf_chain::RpcClient,
    pub oracle: PriceOracle,
    pub mint: String,
    pub decimals: u8,
    pub treasury_token_account: String,
    /// Local test mode: no chain is consulted, any well-formed signature is "paid" for exactly
    /// the quoted amounts. Every other rule (ledger, split, caps, idempotency) stays real.
    pub fake: bool,
}

impl PaymentsRuntime {
    /// USD per CLASH the fake oracle quotes (0.001 = $1 buys 1 000 CLASH).
    pub const FAKE_USD_PER_CLASH_MICRO: i64 = 1_000;

    /// Runtime for `CF_PAYMENTS_FAKE=1`: dead RPC, constant price, placeholder treasury.
    pub fn fake(mint: &str, decimals: u8) -> Result<Self, PaymentsError> {
        let chain = cf_chain::RpcClient::new(cf_chain::RpcConfig {
            primary_url: "http://127.0.0.1:9/unused".to_owned(),
            secondary_url: None,
        })?;
        let price = rust_decimal::Decimal::new(Self::FAKE_USD_PER_CLASH_MICRO, 6);
        Ok(Self {
            chain,
            oracle: PriceOracle::fixed(price, mint)?,
            mint: mint.to_owned(),
            decimals,
            treasury_token_account: "FakeTreasuryTokenAccount1111111111111111111".to_owned(),
            fake: true,
        })
    }
}
