use anchor_lang::prelude::*;
use bond_core::math;
use bond_core::state::{BondState, HolderRecord, Jurisdiction};

use crate::errors::ActionError;
use crate::events::EntitlementCalculated;
use crate::state::*;

/// Permissionless crank, one call per holder. This is the visible on-chain
/// calculation step: snapshot quantity -> gross -> withholding -> net.
///
/// TIMING: must run before the next record window snapshots this holder
/// (see TIMING RULES in state.rs), otherwise the epoch check below rejects it.
#[derive(Accounts)]
pub struct CalculateEntitlement<'info> {
    #[account(mut)]
    pub cranker: Signer<'info>,

    pub bond: Account<'info, BondState>,

    #[account(
        mut,
        seeds = [ACTION_SEED, bond.key().as_ref(), &action.epoch.to_le_bytes()],
        bump = action.bump,
        has_one = bond @ ActionError::WrongBond
    )]
    pub action: Account<'info, CorporateAction>,

    /// bond-core holder registry record, carrying the snapshot balance.
    #[account(
        constraint = holder_record.bond == bond.key() @ ActionError::WrongBond,
        constraint = holder_record.snapshot_epoch == action.epoch @ ActionError::StaleSnapshot
    )]
    pub holder_record: Account<'info, HolderRecord>,

    /// One entitlement per (action, holder): a holder cannot be calculated twice.
    #[account(
        init,
        payer = cranker,
        space = 8 + Entitlement::INIT_SPACE,
        seeds = [ENTITLEMENT_SEED, action.key().as_ref(), holder_record.wallet.as_ref()],
        bump
    )]
    pub entitlement: Account<'info, Entitlement>,

    pub system_program: Program<'info, System>,
}

pub fn handle_calculate_entitlement(ctx: Context<CalculateEntitlement>) -> Result<()> {
    require!(
        ctx.accounts.action.status == ActionStatus::Created,
        ActionError::InvalidStatus
    );

    let bond = &ctx.accounts.bond;
    let hr = &ctx.accounts.holder_record;
    let qty = hr.snapshot_balance;
    let overflow = || error!(ActionError::MathOverflow);

    // Coupon part: qty * face * rate_bps / (10_000 * periods), floored.
    let coupon_gross = math::coupon_gross(
        qty,
        bond.face_value,
        bond.coupon_rate_bps,
        bond.coupons_per_year,
    )
    .map_err(|_| overflow())?;

    // Principal part: only for redemption actions (final coupon is paid with it).
    let principal = match ctx.accounts.action.kind {
        ActionKind::Coupon => 0u64,
        ActionKind::Redemption => {
            math::redemption_principal(qty, bond.face_value).map_err(|_| overflow())?
        }
    };

    // Withholding applies to the COUPON part only, never to principal.
    let withholding_bps = match hr.jurisdiction {
        Jurisdiction::Domestic => bond.withholding_bps_domestic,
        Jurisdiction::Foreign => bond.withholding_bps_foreign,
    };
    let (withheld, _) =
        math::withholding(coupon_gross, withholding_bps).map_err(|_| overflow())?;

    let gross = principal
        .checked_add(coupon_gross)
        .ok_or(ActionError::MathOverflow)?;
    let net = gross
        .checked_sub(withheld)
        .ok_or(ActionError::MathOverflow)?;

    let now = Clock::get()?.unix_timestamp;
    let action = &mut ctx.accounts.action;

    let ent = &mut ctx.accounts.entitlement;
    ent.action = action.key();
    ent.wallet = hr.wallet;
    ent.jurisdiction = hr.jurisdiction;
    ent.quantity = qty;
    ent.principal = principal;
    ent.coupon_gross = coupon_gross;
    ent.gross = gross;
    ent.withholding_bps = withholding_bps;
    ent.withheld = withheld;
    ent.net = net;
    ent.claimed = false;
    ent.calculated_ts = now;
    ent.claimed_ts = 0;
    ent.bump = ctx.bumps.entitlement;

    action.entitlement_count = action
        .entitlement_count
        .checked_add(1)
        .ok_or(ActionError::MathOverflow)?;
    action.total_gross = action
        .total_gross
        .checked_add(gross)
        .ok_or(ActionError::MathOverflow)?;
    action.total_withheld = action
        .total_withheld
        .checked_add(withheld)
        .ok_or(ActionError::MathOverflow)?;
    action.total_net = action
        .total_net
        .checked_add(net)
        .ok_or(ActionError::MathOverflow)?;

    emit!(EntitlementCalculated {
        action: action.key(),
        wallet: ent.wallet,
        quantity: qty,
        gross,
        withheld,
        net,
    });
    Ok(())
}
