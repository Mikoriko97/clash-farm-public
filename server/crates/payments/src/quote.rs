//! Quote arithmetic in fixed-point (`rust_decimal`) and integers: no floats
//! anywhere near money (rules/global.md).

use rust_decimal::prelude::ToPrimitive;
use rust_decimal::Decimal;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Split {
    pub amount_clash: i64,
    pub burn_amount: i64,
    pub transfer_amount: i64,
}

/// Atomic CLASH units for `usd_cents` at `usd_per_clash`, rounded up.
pub fn amount_clash(usd_cents: i64, usd_per_clash: Decimal, decimals: u8) -> Option<i64> {
    if usd_cents <= 0 || usd_per_clash <= Decimal::ZERO {
        return None;
    }
    let scale = Decimal::from(10_i64.checked_pow(u32::from(decimals))?);
    let usd = Decimal::from(usd_cents) / Decimal::from(100);
    let clash = (usd / usd_per_clash).checked_mul(scale)?;
    clash.ceil().to_i64().filter(|v| *v > 0)
}

/// Burn share in basis points, rounded up; the remainder is transferred.
pub fn split(amount_clash: i64, burn_share_bp: i32) -> Option<Split> {
    if amount_clash <= 0 {
        return None;
    }
    let bp = i64::from(burn_share_bp.clamp(0, 10_000));
    let burn = (amount_clash.checked_mul(bp)?.checked_add(9_999)?) / 10_000;
    Some(Split {
        amount_clash,
        burn_amount: burn,
        transfer_amount: amount_clash - burn,
    })
}

/// `1 / usd_per_clash` as stored in `orders.clash_per_usd` (12 dp).
pub fn clash_per_usd(usd_per_clash: Decimal) -> Option<Decimal> {
    if usd_per_clash <= Decimal::ZERO {
        return None;
    }
    Some((Decimal::ONE / usd_per_clash).round_dp(12))
}

#[cfg(test)]
mod tests {
    #![allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
    use super::*;
    use std::str::FromStr;

    #[test]
    fn amounts_round_up_in_atomic_units() {
        let price = Decimal::from_str("0.0031").unwrap(); // $0.0031 per CLASH
                                                          // $0.90 / 0.0031 = 290.322580... CLASH → 290_322_581 atomic units (6 dp), ceil.
        assert_eq!(amount_clash(90, price, 6), Some(290_322_581));
        assert_eq!(
            amount_clash(50, Decimal::from_str("1").unwrap(), 6),
            Some(500_000)
        );
        assert_eq!(amount_clash(0, price, 6), None);
        assert_eq!(amount_clash(90, Decimal::ZERO, 6), None);
    }

    #[test]
    fn split_burns_rounded_up_share() {
        assert_eq!(
            split(1_000_000, 7000),
            Some(Split {
                amount_clash: 1_000_000,
                burn_amount: 700_000,
                transfer_amount: 300_000
            })
        );
        assert_eq!(
            split(3, 7000),
            Some(Split {
                amount_clash: 3,
                burn_amount: 3,
                transfer_amount: 0
            })
        );
        assert_eq!(
            split(10, 10_000),
            Some(Split {
                amount_clash: 10,
                burn_amount: 10,
                transfer_amount: 0
            })
        );
        assert_eq!(
            split(10, 0),
            Some(Split {
                amount_clash: 10,
                burn_amount: 0,
                transfer_amount: 10
            })
        );
        assert_eq!(split(0, 7000), None);
    }

    #[test]
    fn stored_rate_is_inverse_rounded() {
        assert_eq!(
            clash_per_usd(Decimal::from_str("0.0031").unwrap())
                .unwrap()
                .to_string(),
            "322.580645161290"
        );
        assert!(clash_per_usd(Decimal::ZERO).is_none());
    }
}
