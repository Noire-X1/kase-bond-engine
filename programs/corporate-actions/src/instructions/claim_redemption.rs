use anchor_lang::prelude::*;
use anchor_spl::token::{self, Burn, Mint, Token, TokenAccount, TransferChecked};
use bond_core::state::{BondState, HolderRecord};

use crate::errors::ActionError;
use crate::events::RedemptionClaimed;
use crate::state::*;

/// Holder-signed. In ONE transaction: pays principal + final coupon (net of
/// withholding on the coupon part only) from the vault, and burns exactly the
/// holder's snapshot quantity of bond tokens.
///
/// The snapshot froze the holder's bond account, and frozen accounts cannot be
/// burned. The client therefore bundles bond-core `thaw_holder` BEFORE this
/// instruction in the same transaction. (SIMULATED rail: the payment token is a
/// test SPL token standing in for KZT/USDC.)
#[derive(Accounts)]
pub struct ClaimRedemption<'info> {
    pub holder: Signer<'info>,

    pub bond: Account<'info, BondState>,

    /// bond-core registry record: binds the burn to the holder's registered bond account.
    #[account(
        constraint = holder_record.bond == bond.key() @ ActionError::WrongBond,
        constraint = holder_record.wallet == holder.key() @ ActionError::Unauthorized
    )]
    pub holder_record: Account<'info, HolderRecord>,

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

    #[account(mut, address = bond.bond_mint @ ActionError::WrongMint)]
    pub bond_mint: Account<'info, Mint>,

    #[account(
        mut,
        address = holder_record.token_account @ ActionError::WrongTokenAccount
    )]
    pub holder_bond_account: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

pub fn handle_claim_redemption(ctx: Context<ClaimRedemption>) -> Result<()> {
    let action = &ctx.accounts.action;
    match action.status {
        ActionStatus::Funded => {}
        ActionStatus::Created => return err!(ActionError::NotFunded),
        ActionStatus::Closed => return err!(ActionError::InvalidStatus),
    }
    require!(action.kind == ActionKind::Redemption, ActionError::WrongKind);
    require!(
        !ctx.accounts.entitlement.claimed,
        ActionError::AlreadyClaimed
    );

    let quantity = ctx.accounts.entitlement.quantity;
    let principal = ctx.accounts.entitlement.principal;
    let coupon_gross = ctx.accounts.entitlement.coupon_gross;
    let withheld = ctx.accounts.entitlement.withheld;
    let net = ctx.accounts.entitlement.net;

    if quantity > 0 {
        require!(
            !ctx.accounts.holder_bond_account.is_frozen(),
            ActionError::BondAccountFrozen
        );
        require!(
            ctx.accounts.holder_bond_account.amount >= quantity,
            ActionError::InsufficientBonds
        );
    }

    let bond_key = ctx.accounts.bond.key();
    let epoch_bytes = action.epoch.to_le_bytes();
    let bump = [action.bump];
    let seeds: &[&[u8]] = &[ACTION_SEED, bond_key.as_ref(), &epoch_bytes, &bump];
    let signer_seeds = &[seeds];

    // Effects first.
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

    // Interaction 1: pay net principal + final coupon from the vault.
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

    // Interaction 2: burn exactly the snapshot quantity. The holder is the
    // owner of the bond account and signs the transaction.
    if quantity > 0 {
        token::burn(
            CpiContext::new(
                ctx.accounts.token_program.key(),
                Burn {
                    mint: ctx.accounts.bond_mint.to_account_info(),
                    from: ctx.accounts.holder_bond_account.to_account_info(),
                    authority: ctx.accounts.holder.to_account_info(),
                },
            ),
            quantity,
        )?;
    }

    emit!(RedemptionClaimed {
        action: ctx.accounts.action.key(),
        wallet: ctx.accounts.holder.key(),
        quantity_burned: quantity,
        principal,
        coupon_gross,
        withheld,
        net,
    });
    Ok(())
}
