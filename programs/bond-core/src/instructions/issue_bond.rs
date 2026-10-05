use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token};

use crate::errors::BondError;
use crate::events::BondIssued;
use crate::state::*;

#[derive(AnchorSerialize, AnchorDeserialize, Clone)]
pub struct IssueBondParams {
    pub bond_id: u64,
    pub face_value: u64,
    pub coupon_rate_bps: u16,
    pub coupons_per_year: u8,
    pub maturity_ts: i64,
    pub max_supply: u64,
    pub withholding_bps_domestic: u16, // pass 0 for the default
    pub withholding_bps_foreign: u16,  // pass 0 for the default
}

#[derive(Accounts)]
#[instruction(params: IssueBondParams)]
pub struct IssueBond<'info> {
    #[account(mut)]
    pub issuer: Signer<'info>,

    #[account(
        init,
        payer = issuer,
        space = 8 + BondState::INIT_SPACE,
        seeds = [BOND_SEED, issuer.key().as_ref(), &params.bond_id.to_le_bytes()],
        bump
    )]
    pub bond: Account<'info, BondState>,

    /// 1 token = 1 bond, so 0 decimals. The bond PDA is both mint and freeze
    /// authority: only program instructions can mint, snapshot-freeze or thaw.
    #[account(
        init,
        payer = issuer,
        seeds = [MINT_SEED, bond.key().as_ref()],
        bump,
        mint::decimals = 0,
        mint::authority = bond,
        mint::freeze_authority = bond
    )]
    pub bond_mint: Account<'info, Mint>,

    /// Existing test SPL token that stands in for KZT/USDC (simulated rail).
    pub payment_mint: Account<'info, Mint>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

pub fn handle_issue_bond(ctx: Context<IssueBond>, params: IssueBondParams) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;

    require!(params.face_value > 0, BondError::InvalidFaceValue);
    require!(params.coupon_rate_bps <= 10_000, BondError::InvalidCouponRate);
    require!(
        matches!(params.coupons_per_year, 1 | 2 | 4 | 12),
        BondError::InvalidCouponFrequency
    );
    require!(params.maturity_ts > now, BondError::InvalidMaturity);
    require!(params.max_supply > 0, BondError::InvalidMaxSupply);
    require!(
        params.withholding_bps_domestic <= 10_000 && params.withholding_bps_foreign <= 10_000,
        BondError::InvalidWithholdingRate
    );

    let bond = &mut ctx.accounts.bond;
    bond.issuer = ctx.accounts.issuer.key();
    bond.bond_id = params.bond_id;
    bond.bond_mint = ctx.accounts.bond_mint.key();
    bond.payment_mint = ctx.accounts.payment_mint.key();
    bond.face_value = params.face_value;
    bond.coupon_rate_bps = params.coupon_rate_bps;
    bond.coupons_per_year = params.coupons_per_year;
    bond.issue_ts = now;
    bond.maturity_ts = params.maturity_ts;
    bond.max_supply = params.max_supply;
    bond.minted_supply = 0;
    bond.withholding_bps_domestic = params.withholding_bps_domestic;
    bond.withholding_bps_foreign = params.withholding_bps_foreign;
    bond.status = BondStatus::Active;
    bond.record_epoch = 0;
    bond.window_open = false;
    bond.record_slot = 0;
    bond.snapshotted_supply = 0;
    bond.bump = ctx.bumps.bond;
    bond.mint_bump = ctx.bumps.bond_mint;

    emit!(BondIssued {
        bond: bond.key(),
        issuer: bond.issuer,
        bond_mint: bond.bond_mint,
        payment_mint: bond.payment_mint,
        face_value: bond.face_value,
        coupon_rate_bps: bond.coupon_rate_bps,
        coupons_per_year: bond.coupons_per_year,
        maturity_ts: bond.maturity_ts,
        max_supply: bond.max_supply,
    });
    Ok(())
}