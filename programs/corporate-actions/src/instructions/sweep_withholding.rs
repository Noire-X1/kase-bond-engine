use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, TransferChecked};
use bond_core::state::BondState;

use crate::errors::ActionError;
use crate::events::WithholdingSwept;
use crate::state::*;

/// Issuer-only. Moves EXACTLY action.total_withheld from the vault to the
/// tax-collector account, once, after funding. Holders' net amounts stay in the
/// vault untouched. The tax collector is a SIMULATED tax authority: a payment-token
/// account chosen by the issuer; no real tax rail is integrated.
#[derive(Accounts)]
pub struct SweepWithholding<'info> {
    pub issuer: Signer<'info>,

    #[account(has_one = issuer @ ActionError::Unauthorized)]
    pub bond: Account<'info, BondState>,

    #[account(
        mut,
        seeds = [ACTION_SEED, bond.key().as_ref(), &action.epoch.to_le_bytes()],
        bump = action.bump,
        has_one = bond @ ActionError::WrongBond
    )]
    pub action: Account<'info, CorporateAction>,

    #[account(address = action.payment_mint @ ActionError::WrongMint)]
    pub payment_mint: Account<'info, Mint>,

    #[account(
        mut,
        address = action.vault @ ActionError::WrongTokenAccount
    )]
    pub vault: Account<'info, TokenAccount>,

    #[account(
        mut,
        address = action.tax_collector @ ActionError::WrongTokenAccount
    )]
    pub tax_collector: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

pub fn handle_sweep_withholding(ctx: Context<SweepWithholding>) -> Result<()> {
    let action = &ctx.accounts.action;
    require!(action.status == ActionStatus::Funded, ActionError::NotFunded);
    require!(!action.withheld_swept, ActionError::AlreadySwept);
    let amount = action.total_withheld;
    require!(amount > 0, ActionError::NothingToSweep);

    let bond_key = ctx.accounts.bond.key();
    let epoch_bytes = action.epoch.to_le_bytes();
    let bump = [action.bump];
    let seeds: &[&[u8]] = &[ACTION_SEED, bond_key.as_ref(), &epoch_bytes, &bump];
    let signer_seeds = &[seeds];

    // Effects first, then the transfer.
    ctx.accounts.action.withheld_swept = true;

    token::transfer_checked(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.key(),
            TransferChecked {
                from: ctx.accounts.vault.to_account_info(),
                mint: ctx.accounts.payment_mint.to_account_info(),
                to: ctx.accounts.tax_collector.to_account_info(),
                authority: ctx.accounts.action.to_account_info(),
            },
            signer_seeds,
        ),
        amount,
        ctx.accounts.payment_mint.decimals,
    )?;

    emit!(WithholdingSwept {
        action: ctx.accounts.action.key(),
        amount,
        tax_collector: ctx.accounts.tax_collector.key(),
    });
    Ok(())
}
