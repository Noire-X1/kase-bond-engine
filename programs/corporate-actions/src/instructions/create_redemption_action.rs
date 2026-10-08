use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, Token, TokenAccount};
use bond_core::state::{BondState, BondStatus, SnapshotRecord};

use crate::errors::ActionError;
use crate::events::ActionCreated;
use crate::state::*;

/// Issuer-only. Opens THE redemption action for a record window that was
/// opened at or after maturity. Requires every regular coupon to be paid first;
/// the final coupon is then paid together with principal (see calculate_entitlement).
#[derive(Accounts)]
#[instruction(epoch: u32)]
pub struct CreateRedemptionAction<'info> {
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

pub fn handle_create_redemption_action(
    ctx: Context<CreateRedemptionAction>,
    epoch: u32,
) -> Result<()> {
    let clock = Clock::get()?;
    let bond = &ctx.accounts.bond;
    require!(bond.status == BondStatus::Active, ActionError::BondNotActive);
    require!(clock.unix_timestamp >= bond.maturity_ts, ActionError::NotMatured);
    require!(epoch == bond.record_epoch, ActionError::StaleSnapshot);
    require!(
        ctx.accounts.snapshot_record.closed,
        ActionError::SnapshotNotClosed
    );
    // The snapshot must describe holders at or after maturity.
    require!(
        ctx.accounts.snapshot_record.opened_ts >= bond.maturity_ts,
        ActionError::SnapshotBeforeMaturity
    );

    let ledger = &mut ctx.accounts.ledger;
    require!(
        !ledger.redemption_created,
        ActionError::RedemptionAlreadyCreated
    );
    // total_coupons includes the final coupon paid with redemption, so every
    // regular coupon (total_coupons - 1) must already be funded.
    let required = ledger
        .total_coupons
        .checked_sub(1)
        .ok_or(ActionError::MathOverflow)?;
    require!(ledger.coupons_paid == required, ActionError::CouponsOutstanding);
    ledger.redemption_created = true;

    let action = &mut ctx.accounts.action;
    action.bond = bond.key();
    action.issuer = ctx.accounts.issuer.key();
    action.kind = ActionKind::Redemption;
    action.epoch = epoch;
    action.action_index = 0;
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
