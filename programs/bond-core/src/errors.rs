use anchor_lang::prelude::*;

#[error_code]
pub enum BondError {
    #[msg("Face value must be greater than zero")]
    InvalidFaceValue,
    #[msg("Coupon rate must be between 0 and 10000 bps")]
    InvalidCouponRate,
    #[msg("Coupons per year must be 1, 2, 4 or 12")]
    InvalidCouponFrequency,
    #[msg("Maturity must be in the future")]
    InvalidMaturity,
    #[msg("Max supply must be greater than zero")]
    InvalidMaxSupply,
    #[msg("Withholding rate must be between 0 and 10000 bps")]
    InvalidWithholdingRate,
    #[msg("Arithmetic overflow")]
    MathOverflow,
    #[msg("Coupon periods must be greater than zero")]
    ZeroPeriods,
}