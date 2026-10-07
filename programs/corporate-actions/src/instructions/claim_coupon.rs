use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, Token, TokenAccount, TransferChecked};
use bond_core::state::BondState;

use crate::errors::ActionError;
use crate::events::EntitlementClaimed;
use crate::state::*;

/// Holder-signed. Pays the holder's NET coupon from the action vault.
/// (SIMULATED rail: the payment token is a test SPL token standing in for KZT/USDC.)
/// Withheld tax stays in the vault until sweep_withholding.
#[derive(Accounts)]
pub struct ClaimCoupon<'info> {
    pub holder: Signer<'info>,

    pub bond: Account<'info, BondState>,

    #[account(
        mut,
        seeds = [ACTION_SEED, bond.key().as_ref(), &action.epoch.to_le_bytes()],
        bump = action.bump,
        has_one = bond @ ActionError::WrongBond
    )]
    pub action: Account<'info, CorporateAction>,

    /// Derived from the signer's own address: a holder can only claim their own.
    #[account(
        mut,
        seeds = [ENTITLEMENT_SEED, action.key().as_ref(), holder.key().as_ref()],
        bump = entitlement.bump
    )]
    pub entitlement: Account<'info, Entitlement>,

    #[account(address = action.payment_mint @ ActionError::WrongMint)]
    pub payment_mint: Account<'info, Mint>,

    #[account(
        mut,
        address = action.vault @ ActionError::WrongTokenAccount
    )]
    pub vault: Account<'info, TokenAccount>,

    #[account(
        mut,
        token::mint = payment_mint,
        token::authority = holder
    )]
    pub holder_payment_account: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

pub fn handle_claim_coupon(ctx: Context<ClaimCoupon>) -> Result<()> {
    let action = &ctx.accounts.action;
    match action.status {
        ActionStatus::Funded => {}
        ActionStatus::Created => return err!(ActionError::NotFunded),
        ActionStatus::Closed => return err!(ActionError::InvalidStatus),
    }
    require!(action.kind == ActionKind::Coupon, ActionError::WrongKind);
    require!(
        !ctx.accounts.entitlement.claimed,
        ActionError::AlreadyClaimed
    );

    let net = ctx.accounts.entitlement.net;
    let bond_key = ctx.accounts.bond.key();
    let epoch_bytes = action.epoch.to_le_bytes();
    let bump = [action.bump];
    let seeds: &[&[u8]] = &[ACTION_SEED, bond_key.as_ref(), &epoch_bytes, &bump];
    let signer_seeds = &[seeds];

    // Effects first: mark claimed and update totals.
    let now = Clock::get()?.unix_timestamp;
    {
        let ent = &mut ctx.accounts.entitlement;
        ent.claimed = true;
        ent.claimed_ts = now;
    }
    {
        let action = &mut ctx.accounts.action;
        action.claimed_count = action
            .claimed_count
            .checked_add(1)
            .ok_or(ActionError::MathOverflow)?;
        action.total_claimed_net = action
            .total_claimed_net
            .checked_add(net)
            .ok_or(ActionError::MathOverflow)?;
    }

    // Interaction: pay the net amount from the vault (zero-net claims skip this).
    if net > 0 {
        token::transfer_checked(
            CpiContext::new_with_signer(
                ctx.accounts.token_program.key(),
                TransferChecked {
                    from: ctx.accounts.vault.to_account_info(),
                    mint: ctx.accounts.payment_mint.to_account_info(),
                    to: ctx.accounts.holder_payment_account.to_account_info(),
                    authority: ctx.accounts.action.to_account_info(),
                },
                signer_seeds,
            ),
            net,
            ctx.accounts.payment_mint.decimals,
        )?;
    }

    emit!(EntitlementClaimed {
        action: ctx.accounts.action.key(),
        wallet: ctx.accounts.holder.key(),
        net,
    });
    Ok(())
}
