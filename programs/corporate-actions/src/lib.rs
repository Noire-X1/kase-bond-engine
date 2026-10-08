use anchor_lang::prelude::*;

pub mod errors;
pub mod events;
pub mod instructions;
pub mod state;

use instructions::*;

declare_id!("59hoGMNeum386CpHQpbsUStq51vfXS9iYxPkq5apthNg");

#[program]
pub mod corporate_actions {
    use super::*;

    pub fn initialize_ledger(ctx: Context<InitializeLedger>, total_coupons: u16) -> Result<()> {
        instructions::initialize_ledger::handle_initialize_ledger(ctx, total_coupons)
    }

    pub fn create_coupon_action(ctx: Context<CreateCouponAction>, epoch: u32) -> Result<()> {
        instructions::create_coupon_action::handle_create_coupon_action(ctx, epoch)
    }

    pub fn create_redemption_action(
        ctx: Context<CreateRedemptionAction>,
        epoch: u32,
    ) -> Result<()> {
        instructions::create_redemption_action::handle_create_redemption_action(ctx, epoch)
    }

    pub fn calculate_entitlement(ctx: Context<CalculateEntitlement>) -> Result<()> {
        instructions::calculate_entitlement::handle_calculate_entitlement(ctx)
    }

    pub fn fund_action(ctx: Context<FundAction>) -> Result<()> {
        instructions::fund_action::handle_fund_action(ctx)
    }

    pub fn claim_coupon(ctx: Context<ClaimCoupon>) -> Result<()> {
        instructions::claim_coupon::handle_claim_coupon(ctx)
    }

    pub fn claim_redemption(ctx: Context<ClaimRedemption>) -> Result<()> {
        instructions::claim_redemption::handle_claim_redemption(ctx)
    }

    pub fn sweep_withholding(ctx: Context<SweepWithholding>) -> Result<()> {
        instructions::sweep_withholding::handle_sweep_withholding(ctx)
    }
}
