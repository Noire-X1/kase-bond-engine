use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, TransferChecked};
use bond_core::state::BondState;

use crate::errors::ActionError;
use crate::events::ActionFunded;
use crate::state::*;

/// Issuer-only. Deposits EXACTLY action.total_gross into the vault, once, and
/// only after every snapshotted holder has an entitlement, so the amount owed
/// is final. (SIMULATED rail: the payment token is a test SPL token standing
/// in for KZT/USDC.)
#[derive(Accounts)]
pub struct FundAction<'info> {
    pub issuer: Signer<'info>,

    #[account(has_one = issuer @ ActionError::Unauthorized)]
    pub bond: Account<'info, BondState>,

    #[account(
        mut,
        seeds = [LEDGER_SEED, bond.key().as_ref()],
        bump = ledger.bump,
        has_one = bond @ ActionError::WrongBond
    )]
    pub ledger: Account<'info, BondLedger>,

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
        token::mint = payment_mint,
        token::authority = issuer
    )]
    pub issuer_payment_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        address = action.vault @ ActionError::WrongTokenAccount
    )]
    pub vault: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

pub fn handle_fund_action(ctx: Context<FundAction>) -> Result<()> {
    let action = &ctx.accounts.action;
    require!(
        action.status == ActionStatus::Created,
        ActionError::InvalidStatus
    );
    // Every snapshotted holder must be calculated before money moves.
    require!(
        action.entitlement_count == action.snapshot_holder_count,
        ActionError::CalculationIncomplete
    );

    let amount = action.total_gross;
    let kind = action.kind;

    token::transfer_checked(
        CpiContext::new(
            ctx.accounts.token_program.key(),
            TransferChecked {
                from: ctx.accounts.issuer_payment_account.to_account_info(),
                mint: ctx.accounts.payment_mint.to_account_info(),
                to: ctx.accounts.vault.to_account_info(),
                authority: ctx.accounts.issuer.to_account_info(),
            },
        ),
        amount,
        ctx.accounts.payment_mint.decimals,
    )?;

    let now = Clock::get()?.unix_timestamp;
    let action = &mut ctx.accounts.action;
    action.status = ActionStatus::Funded;
    action.funded_amount = amount;
    action.funded_ts = now;

    if kind == ActionKind::Coupon {
        let ledger = &mut ctx.accounts.ledger;
        ledger.coupons_paid = ledger
            .coupons_paid
            .checked_add(1)
            .ok_or(ActionError::MathOverflow)?;
    }

    emit!(ActionFunded {
        action: action.key(),
        amount,
        entitlement_count: action.entitlement_count,
    });
    Ok(())
}
