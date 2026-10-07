use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, TokenAccount};
use bond_core::state::{BondState, BondStatus};

use crate::errors::ActionError;
use crate::events::LedgerInitialized;
use crate::state::*;

/// Issuer-only, once per bond (the ledger PDA can only be created once).
/// Sets how many coupons the bond pays in total (including the final coupon
/// that is paid with redemption) and the SIMULATED tax-collector account.
#[derive(Accounts)]
pub struct InitializeLedger<'info> {
    #[account(mut)]
    pub issuer: Signer<'info>,

    #[account(has_one = issuer @ ActionError::Unauthorized)]
    pub bond: Account<'info, BondState>,

    #[account(address = bond.payment_mint @ ActionError::WrongMint)]
    pub payment_mint: Account<'info, Mint>,

    /// SIMULATED tax authority: any payment-token account chosen by the issuer.
    /// It receives swept withholding. No real tax rail is integrated.
    #[account(token::mint = payment_mint)]
    pub tax_collector: Account<'info, TokenAccount>,

    #[account(
        init,
        payer = issuer,
        space = 8 + BondLedger::INIT_SPACE,
        seeds = [LEDGER_SEED, bond.key().as_ref()],
        bump
    )]
    pub ledger: Account<'info, BondLedger>,

    pub system_program: Program<'info, System>,
}

pub fn handle_initialize_ledger(
    ctx: Context<InitializeLedger>,
    total_coupons: u16,
) -> Result<()> {
    require!(total_coupons >= 1, ActionError::InvalidTotalCoupons);
    require!(
        ctx.accounts.bond.status == BondStatus::Active,
        ActionError::BondNotActive
    );

    let ledger = &mut ctx.accounts.ledger;
    ledger.bond = ctx.accounts.bond.key();
    ledger.issuer = ctx.accounts.issuer.key();
    ledger.tax_collector = ctx.accounts.tax_collector.key();
    ledger.total_coupons = total_coupons;
    ledger.coupons_created = 0;
    ledger.coupons_paid = 0;
    ledger.redemption_created = false;
    ledger.created_ts = Clock::get()?.unix_timestamp;
    ledger.bump = ctx.bumps.ledger;

    emit!(LedgerInitialized {
        bond: ledger.bond,
        issuer: ledger.issuer,
        total_coupons,
        tax_collector: ledger.tax_collector,
    });
    Ok(())
}
