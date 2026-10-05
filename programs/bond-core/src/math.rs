use anchor_lang::prelude::*;
use crate::errors::BondError;

pub const BPS_DENOM: u128 = 10_000;

/// Gross coupon = bonds_held * face_value * rate_bps / (10_000 * periods)
/// Single floor division at the end; dust stays in the vault.
pub fn coupon_gross(bonds_held: u64, face_value: u64, rate_bps: u16, periods: u8) -> Result<u64> {
    if periods == 0 {
        return err!(BondError::ZeroPeriods);
    }
    let num = (bonds_held as u128)
        .checked_mul(face_value as u128)
        .and_then(|v| v.checked_mul(rate_bps as u128))
        .ok_or(BondError::MathOverflow)?;
    let den = BPS_DENOM
        .checked_mul(periods as u128)
        .ok_or(BondError::MathOverflow)?;
    u64::try_from(num / den).map_err(|_| error!(BondError::MathOverflow))
}

/// Principal at maturity = bonds_held * face_value
pub fn redemption_principal(bonds_held: u64, face_value: u64) -> Result<u64> {
    (bonds_held as u128)
        .checked_mul(face_value as u128)
        .and_then(|v| u64::try_from(v).ok())
        .ok_or_else(|| error!(BondError::MathOverflow))
}

/// Returns (tax_withheld, net). tax is floored, net = gross - tax,
/// so tax + net == gross always holds exactly.
pub fn withholding(gross: u64, tax_bps: u16) -> Result<(u64, u64)> {
    if tax_bps as u128 > BPS_DENOM {
        return err!(BondError::InvalidWithholdingRate);
    }
    let tax = (gross as u128)
        .checked_mul(tax_bps as u128)
        .ok_or(BondError::MathOverflow)?
        / BPS_DENOM;
    let tax = tax as u64; // tax <= gross, so this fits
    Ok((tax, gross - tax))
}

#[cfg(test)]
mod tests {
    use super::*;

    // ---- Phase 1 spec example ----
    // 10 bonds x 1,000 x 10% / 2 = 500 per coupon; 10,000 principal at maturity.
    #[test]
    fn spec_coupon_is_500() {
        assert_eq!(coupon_gross(10, 1_000, 1_000, 2).unwrap(), 500);
    }

    #[test]
    fn spec_principal_is_10_000() {
        assert_eq!(redemption_principal(10, 1_000).unwrap(), 10_000);
    }

    #[test]
    fn spec_default_withholding_is_zero() {
        assert_eq!(withholding(500, 0).unwrap(), (0, 500));
    }

    // ---- Edge cases ----
    #[test]
    fn coupon_floors_never_rounds_up() {
        // 1 * 1000 * 1000 / (10000 * 12) = 8.33... -> 8
        assert_eq!(coupon_gross(1, 1_000, 1_000, 12).unwrap(), 8);
    }

    #[test]
    fn withholding_splits_exactly() {
        // 15% of 500 = 75 tax, 425 net
        assert_eq!(withholding(500, 1_500).unwrap(), (75, 425));
        for gross in [0u64, 1, 7, 499, 500, 10_000, u64::MAX] {
            for bps in [0u16, 1, 1_500, 9_999, 10_000] {
                let (t, n) = withholding(gross, bps).unwrap();
                assert_eq!(t + n, gross);
            }
        }
    }

    #[test]
    fn zero_periods_rejected() {
        assert!(coupon_gross(10, 1_000, 1_000, 0).is_err());
    }

    #[test]
    fn overflow_is_an_error_not_a_wrap() {
        assert!(coupon_gross(u64::MAX, u64::MAX, 10_000, 1).is_err());
        assert!(redemption_principal(u64::MAX, 2).is_err());
    }

    #[test]
    fn tax_rate_over_100_percent_rejected() {
        assert!(withholding(500, 10_001).is_err());
    }
}