use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token, TokenAccount};
use bond_core::state::{BondState, BondStatus, SnapshotRecord};

use crate::errors::ActionError;
use crate::events::ActionCreated;
use crate::state::*;

/// Issuer-only. Opens ONE coupon action for ONE closed record window (epoch).
/// The action PDA is keyed by (bond, epoch), so a snapshot can never pay twice.
#[derive(Accounts)]
#[instruction(epoch: u32)]
pub struct CreateCouponAction<'info> {
    #[account(mut)]
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

    /// The bond-core audit record of the record-date window for this epoch.
    #[account(
        constraint = snapshot_record.bond == bond.key() @ ActionError::WrongBond,
        constraint = snapshot_record.epoch == epoch @ ActionError::StaleSnapshot
    )]
    pub snapshot_record: Account<'info, SnapshotRecord>,

    #[account(address = bond.payment_mint @ ActionError::WrongMint)]
    pub payment_mint: Account<'info, Mint>,

    #[account(
        init,
        payer = issuer,
        space = 8 + CorporateAction::INIT_SPACE,
        seeds = [ACTION_SEED, bond.key().as_ref(), &epoch.to_le_bytes()],
        bump
    )]
    pub action: Account<'info, CorporateAction>,

    /// Holds the payment tokens for this action. Authority is the action PDA.
    #[account(
        init,
        payer = issuer,
        seeds = [VAULT_SEED, action.key().as_ref()],
        bump,
        token::mint = payment_mint,
        token::authority = action
    )]
    pub vault: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
    pub system_program: Program<'info, System>,
}

pub fn handle_create_coupon_action(ctx: Context<CreateCouponAction>, epoch: u32) -> Result<()> {
    let bond = &ctx.accounts.bond;
    require!(bond.status == BondStatus::Active, ActionError::BondNotActive);

    // Only the latest window: older windows' holder snapshots were overwritten
    // (see TIMING RULES in state.rs), so an action on them could never be calculated.
    require!(epoch == bond.record_epoch, ActionError::StaleSnapshot);
    require!(
        ctx.accounts.snapshot_record.closed,
        ActionError::SnapshotNotClosed
    );

    let ledger = &mut ctx.accounts.ledger;
    require!(!ledger.redemption_created, ActionError::InvalidStatus);

    // Regular coupons are capped at total_coupons - 1; the final coupon is
    // paid together with redemption.
    let max_regular = ledger
        .total_coupons
        .checked_sub(1)
        .ok_or(ActionError::MathOverflow)?;
    require!(
        ledger.coupons_created < max_regular,
        ActionError::AllCouponsCreated
    );
    ledger.coupons_created = ledger
        .coupons_created
        .checked_add(1)
        .ok_or(ActionError::MathOverflow)?;

    let clock = Clock::get()?;
    let action = &mut ctx.accounts.action;
    action.bond = bond.key();
    action.issuer = ctx.accounts.issuer.key();
    action.kind = ActionKind::Coupon;
    action.epoch = epoch;
    action.action_index = ledger.coupons_created;
    action.payment_mint = ctx.accounts.payment_mint.key();
    action.vault = ctx.accounts.vault.key();
    action.tax_collector = ledger.tax_collector;
    action.status = ActionStatus::Created;
    action.snapshot_holder_count = ctx.accounts.snapshot_record.holder_count;
    action.entitlement_count = 0;
    action.claimed_count = 0;
    action.total_gross = 0;
    action.total_withheld = 0;
    action.total_net = 0;
    action.total_claimed_net = 0;
    action.funded_amount = 0;
    action.withheld_swept = false;
    action.created_slot = clock.slot;
    action.created_ts = clock.unix_timestamp;
    action.funded_ts = 0;
    action.closed_ts = 0;
    action.bump = ctx.bumps.action;
    action.vault_bump = ctx.bumps.vault;

    emit!(ActionCreated {
        bond: action.bond,
        action: action.key(),
        kind: action.kind,
        epoch,
        action_index: action.action_index,
        snapshot_holder_count: action.snapshot_holder_count,
        slot: clock.slot,
    });
    Ok(())
}
