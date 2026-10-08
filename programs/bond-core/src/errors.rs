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
    #[msg("Only the bond issuer can do this")]
    Unauthorized,
    #[msg("Account belongs to a different bond")]
    WrongBond,
    #[msg("Wrong bond mint")]
    WrongMint,
    #[msg("Wrong token account for this holder")]
    WrongTokenAccount,
    #[msg("Bond is not active")]
    BondNotActive,
    #[msg("A record window is already open")]
    WindowOpen,
    #[msg("No record window is open")]
    WindowNotOpen,
    #[msg("Amount must be greater than zero")]
    InvalidAmount,
    #[msg("Minting would exceed max supply")]
    MaxSupplyExceeded,
    #[msg("Holder already snapshotted in this window")]
    AlreadySnapshotted,
    #[msg("Snapshot incomplete: snapshotted balances do not equal total supply")]
    SnapshotIncomplete,
    #[msg("Token account is not frozen")]
    AccountNotFrozen,
    #[msg("Bond has not reached maturity")]
    NotMatured,
    #[msg("Bond tokens are still outstanding: not every holder has redeemed")]
    SupplyOutstanding,
    #[msg("No bonds were ever issued")]
    NothingIssued,
}
