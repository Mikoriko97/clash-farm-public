//! Pure verification of a fetched transaction against an order
//! (docs/SECURITY.md §4, ADR-017). No I/O: fully unit-testable on fixtures.

use crate::parsed::{Instruction, TransactionResult};
use crate::{ALLOWED_PROGRAMS, MEMO_PROGRAM, TOKEN_PROGRAM};

/// What the order expects to see on chain.
#[derive(Debug, Clone)]
pub struct Expectation {
    pub mint: String,
    pub decimals: u8,
    /// Player wallet (base58) that must be the burn/transfer authority.
    pub owner: String,
    pub burn_amount: u64,
    pub transfer_amount: u64,
    /// Treasury token account (not the wallet): the only allowed destination.
    pub treasury_token_account: String,
    pub memo: String,
    /// Reject blocks older than this (order.created_at - 60 s), unix seconds.
    pub min_block_time: i64,
    /// Reject blocks newer than this (quote_expires_at + grace), unix seconds:
    /// a quote can never be paid at a later, cheaper rate (SECURITY §2).
    pub max_block_time: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Verified {
    pub burn: u64,
    pub transfer: u64,
    pub slot: u64,
    pub block_time: i64,
}

/// Stable reason codes exposed by the API (docs/API.md).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RejectReason {
    TxFailed,
    ForbiddenInstruction,
    WrongMint,
    WrongOwner,
    AmountLow,
    MemoMismatch,
    TooOld,
    TooLate,
    Malformed,
}

impl RejectReason {
    pub const fn code(self) -> &'static str {
        match self {
            Self::TxFailed => "tx_failed",
            Self::ForbiddenInstruction => "forbidden_instruction",
            Self::WrongMint => "wrong_mint",
            Self::WrongOwner => "wrong_owner",
            Self::AmountLow => "amount_low",
            Self::MemoMismatch => "memo_mismatch",
            Self::TooOld => "too_old",
            Self::TooLate => "too_late",
            Self::Malformed => "malformed",
        }
    }
}

pub fn verify(tx: &TransactionResult, exp: &Expectation) -> Result<Verified, RejectReason> {
    let meta = tx.meta.as_ref().ok_or(RejectReason::Malformed)?;
    if meta.err.is_some() {
        return Err(RejectReason::TxFailed);
    }
    // A missing block time is not "unknown, so fine": it is an answer we cannot bound.
    let block_time = tx.block_time.ok_or(RejectReason::Malformed)?;
    if block_time < exp.min_block_time {
        return Err(RejectReason::TooOld);
    }
    if block_time > exp.max_block_time {
        return Err(RejectReason::TooLate);
    }
    let all_instructions = tx
        .transaction
        .message
        .instructions
        .iter()
        .chain(meta.inner_instructions.iter().flat_map(|i| i.instructions.iter()));
    for ix in all_instructions {
        if !ALLOWED_PROGRAMS.contains(&ix.program_id.as_str()) {
            return Err(RejectReason::ForbiddenInstruction);
        }
    }

    let mut burn: u64 = 0;
    let mut transfer: u64 = 0;
    let mut memo_ok = false;
    for ix in &tx.transaction.message.instructions {
        match ix.program_id.as_str() {
            p if p == TOKEN_PROGRAM => match check_token(ix, exp)? {
                TokenMove::Burn(amount) => burn = burn.checked_add(amount).ok_or(RejectReason::Malformed)?,
                TokenMove::Transfer(amount) => {
                    transfer = transfer.checked_add(amount).ok_or(RejectReason::Malformed)?
                }
            },
            p if p == MEMO_PROGRAM => {
                memo_ok |= ix.memo_text().as_deref() == Some(exp.memo.as_str());
            }
            _ => {} // ComputeBudget: allowed, carries nothing we verify.
        }
    }
    if !memo_ok {
        return Err(RejectReason::MemoMismatch);
    }
    if burn < exp.burn_amount || transfer < exp.transfer_amount {
        return Err(RejectReason::AmountLow);
    }
    Ok(Verified {
        burn,
        transfer,
        slot: tx.slot,
        block_time,
    })
}

enum TokenMove {
    Burn(u64),
    Transfer(u64),
}

/// Only `burnChecked` (by the player, on our mint) and `transferChecked`
/// (by the player, on our mint, to the treasury account) are acceptable.
fn check_token(ix: &Instruction, exp: &Expectation) -> Result<TokenMove, RejectReason> {
    let token = ix.token().ok_or(RejectReason::ForbiddenInstruction)?;
    if token.kind != "burnChecked" && token.kind != "transferChecked" {
        return Err(RejectReason::ForbiddenInstruction);
    }
    let info = &token.info;
    if info.mint.as_deref() != Some(exp.mint.as_str()) {
        return Err(RejectReason::WrongMint);
    }
    if info.authority.as_deref() != Some(exp.owner.as_str()) {
        return Err(RejectReason::WrongOwner);
    }
    let amount = info.token_amount.as_ref().ok_or(RejectReason::Malformed)?;
    if amount.decimals != exp.decimals {
        return Err(RejectReason::WrongMint);
    }
    let units: u64 = amount.amount.parse().map_err(|_| RejectReason::Malformed)?;
    match token.kind.as_str() {
        "burnChecked" => Ok(TokenMove::Burn(units)),
        "transferChecked" => {
            if info.destination.as_deref() != Some(exp.treasury_token_account.as_str()) {
                return Err(RejectReason::ForbiddenInstruction);
            }
            Ok(TokenMove::Transfer(units))
        }
        _ => Err(RejectReason::ForbiddenInstruction),
    }
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;
    use crate::COMPUTE_BUDGET_PROGRAM;
    use serde_json::json;

    const MINT: &str = "9mM1Mc4Ta9UJJ32v5qsHef91PiXi7EWyiSsqF5WXpump";
    const OWNER: &str = "7Np41oeYqPefeNQEHSv1UDhYrehxin3NStELsSKCT4K2";
    const TREASURY_ATA: &str = "TreasuryATA111111111111111111111111111111111";

    fn expectation() -> Expectation {
        Expectation {
            mint: MINT.into(),
            decimals: 6,
            owner: OWNER.into(),
            burn_amount: 700_000,
            transfer_amount: 300_000,
            treasury_token_account: TREASURY_ATA.into(),
            memo: "cf:order-1".into(),
            min_block_time: 1_800_000_000,
            max_block_time: 1_800_000_200,
        }
    }

    fn token_ix(kind: &str, amount: u64, extra: serde_json::Value) -> serde_json::Value {
        let mut info = json!({ "mint": MINT, "authority": OWNER, "tokenAmount": { "amount": amount.to_string(), "decimals": 6, "uiAmount": 0.0, "uiAmountString": "0" } });
        for (k, v) in extra.as_object().unwrap() {
            info[k] = v.clone();
        }
        json!({ "program": "spl-token", "programId": TOKEN_PROGRAM, "parsed": { "type": kind, "info": info } })
    }

    fn tx(instructions: Vec<serde_json::Value>, err: serde_json::Value) -> TransactionResult {
        tx_at(instructions, err, json!(1_800_000_100))
    }

    fn tx_at(
        instructions: Vec<serde_json::Value>,
        err: serde_json::Value,
        block_time: serde_json::Value,
    ) -> TransactionResult {
        serde_json::from_value(json!({
            "slot": 123,
            "blockTime": block_time,
            "meta": { "err": err, "innerInstructions": [] },
            "transaction": { "message": { "instructions": instructions }, "signatures": ["sig"] }
        }))
        .unwrap()
    }

    fn good() -> Vec<serde_json::Value> {
        vec![
            json!({ "programId": COMPUTE_BUDGET_PROGRAM, "accounts": [], "data": "3gJqkocMWaMm" }),
            token_ix("burnChecked", 700_000, json!({ "account": "PlayerATA" })),
            token_ix(
                "transferChecked",
                300_000,
                json!({ "source": "PlayerATA", "destination": TREASURY_ATA }),
            ),
            json!({ "program": "spl-memo", "programId": MEMO_PROGRAM, "parsed": "cf:order-1" }),
        ]
    }

    #[test]
    fn accepts_canonical_payment() {
        let v = verify(&tx(good(), serde_json::Value::Null), &expectation()).unwrap();
        assert_eq!(
            v,
            Verified {
                burn: 700_000,
                transfer: 300_000,
                slot: 123,
                block_time: 1_800_000_100
            }
        );
    }

    #[test]
    fn rejects_failed_tx() {
        assert_eq!(
            verify(
                &tx(good(), json!({ "InstructionError": [1, "Custom"] })),
                &expectation()
            ),
            Err(RejectReason::TxFailed)
        );
    }

    #[test]
    fn rejects_every_forbidden_instruction() {
        for kind in [
            "approve",
            "approveChecked",
            "setAuthority",
            "closeAccount",
            "transfer",
            "revoke",
            "initializeAccount",
        ] {
            let mut ixs = good();
            ixs.push(token_ix(kind, 1, json!({ "destination": TREASURY_ATA })));
            assert_eq!(
                verify(&tx(ixs, serde_json::Value::Null), &expectation()),
                Err(RejectReason::ForbiddenInstruction),
                "{kind}"
            );
        }
        let mut ixs = good();
        ixs.push(json!({ "programId": "SomeOtherProgram1111111111111111111111111111", "accounts": [], "data": "" }));
        assert_eq!(
            verify(&tx(ixs, serde_json::Value::Null), &expectation()),
            Err(RejectReason::ForbiddenInstruction)
        );
    }

    #[test]
    fn rejects_transfer_to_any_other_address() {
        let ixs = vec![
            token_ix("burnChecked", 700_000, json!({})),
            token_ix(
                "transferChecked",
                300_000,
                json!({ "destination": "Attacker1111111111111111111111111111111111111" }),
            ),
            json!({ "programId": MEMO_PROGRAM, "parsed": "cf:order-1" }),
        ];
        assert_eq!(
            verify(&tx(ixs, serde_json::Value::Null), &expectation()),
            Err(RejectReason::ForbiddenInstruction)
        );
    }

    #[test]
    fn rejects_wrong_mint_owner_amount_memo_and_age() {
        let mut ixs = good();
        ixs[1]["parsed"]["info"]["mint"] = json!("OtherMint1111111111111111111111111111111111");
        assert_eq!(
            verify(&tx(ixs, serde_json::Value::Null), &expectation()),
            Err(RejectReason::WrongMint)
        );

        let mut ixs = good();
        ixs[1]["parsed"]["info"]["authority"] = json!("SomeoneElse111111111111111111111111111111111");
        assert_eq!(
            verify(&tx(ixs, serde_json::Value::Null), &expectation()),
            Err(RejectReason::WrongOwner)
        );

        let mut ixs = good();
        ixs[2]["parsed"]["info"]["tokenAmount"]["amount"] = json!("299999");
        assert_eq!(
            verify(&tx(ixs, serde_json::Value::Null), &expectation()),
            Err(RejectReason::AmountLow)
        );

        let mut ixs = good();
        ixs[1]["parsed"]["info"]["tokenAmount"]["decimals"] = json!(9);
        assert_eq!(
            verify(&tx(ixs, serde_json::Value::Null), &expectation()),
            Err(RejectReason::WrongMint)
        );

        let mut ixs = good();
        ixs[3]["parsed"] = json!("cf:other-order");
        assert_eq!(
            verify(&tx(ixs, serde_json::Value::Null), &expectation()),
            Err(RejectReason::MemoMismatch)
        );

        let mut old = tx(good(), serde_json::Value::Null);
        old.block_time = Some(1_700_000_000);
        assert_eq!(verify(&old, &expectation()), Err(RejectReason::TooOld));
    }

    #[test]
    fn inner_instructions_are_checked_too() {
        let mut t = tx(good(), serde_json::Value::Null);
        t.meta.as_mut().unwrap().inner_instructions = vec![serde_json::from_value(json!({
            "index": 1,
            "instructions": [{ "programId": "Evil11111111111111111111111111111111111111", "accounts": [], "data": "" }]
        }))
        .unwrap()];
        assert_eq!(
            verify(&t, &expectation()),
            Err(RejectReason::ForbiddenInstruction)
        );
    }

    #[test]
    fn rejects_blocks_outside_the_quote_window_and_unbounded_ones() {
        assert_eq!(
            verify(&tx_at(good(), json!(null), json!(1_799_999_000)), &expectation()),
            Err(RejectReason::TooOld)
        );
        assert_eq!(
            verify(&tx_at(good(), json!(null), json!(1_800_000_201)), &expectation()),
            Err(RejectReason::TooLate)
        );
        assert_eq!(
            verify(&tx_at(good(), json!(null), json!(null)), &expectation()),
            Err(RejectReason::Malformed)
        );
    }

    #[test]
    fn meta_without_err_key_does_not_parse() {
        let value = json!({
            "slot": 1, "blockTime": 1_800_000_100,
            "meta": { "innerInstructions": [] },
            "transaction": { "message": { "instructions": good() }, "signatures": ["sig"] }
        });
        assert!(serde_json::from_value::<TransactionResult>(value).is_err());
    }

    #[test]
    fn approve_is_forbidden_even_without_mint_or_authority() {
        let mut ixs = good();
        ixs.push(json!({ "program": "spl-token", "programId": TOKEN_PROGRAM, "parsed": { "type": "approve", "info": { "source": "a", "delegate": "b", "owner": OWNER, "amount": "1" } } }));
        assert_eq!(
            verify(&tx(ixs, json!(null)), &expectation()),
            Err(RejectReason::ForbiddenInstruction)
        );
    }
}
