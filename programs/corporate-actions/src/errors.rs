use anchor_lang::prelude::*;

#[error_code]
pub enum ActionError {
    #[msg("Only the bond issuer can do this")]
    Unauthorized,
    #[msg("Account belongs to a different bond")]
    WrongBond,
    #[msg("Wrong payment mint")]
    WrongMint,
    #[msg("Wrong token account")]
    WrongTokenAccount,
    #[msg("Bond is not active")]
    BondNotActive,
    #[msg("total_coupons must be at least 1")]
    InvalidTotalCoupons,
    #[msg("All coupon actions for this bond have already been created")]
    AllCouponsCreated,
    #[msg("Coupons must be paid before redemption")]
    CouponsOutstanding,
    #[msg("Bond has not reached maturity")]
    NotMatured,
    #[msg("Redemption action already created for this bond")]
    RedemptionAlreadyCreated,
    #[msg("Record window for this epoch is not closed")]
    SnapshotNotClosed,
    #[msg("Holder snapshot does not belong to this action's epoch")]
    StaleSnapshot,
    #[msg("Action is not in the right status for this step")]
    InvalidStatus,
    #[msg("Wrong action kind for this instruction")]
    WrongKind,
    #[msg("Entitlements not yet calculated for every snapshotted holder")]
    CalculationIncomplete,
    #[msg("Funding amount must equal the action's total gross")]
    WrongFundingAmount,
    #[msg("Action has not been funded")]
    NotFunded,
    #[msg("Entitlement already claimed")]
    AlreadyClaimed,
    #[msg("Withholding already swept")]
    AlreadySwept,
    #[msg("No withholding to sweep")]
    NothingToSweep,
    #[msg("Arithmetic overflow")]
    MathOverflow,
    #[msg("Redemption record window must open at or after maturity")]
    SnapshotBeforeMaturity,
    #[msg("Thaw the holder's bond account first (bond-core thaw_holder)")]
    BondAccountFrozen,
    #[msg("Holder no longer holds the snapshot quantity of bonds")]
    InsufficientBonds,
}
