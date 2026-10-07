use anchor_lang::prelude::*;

use crate::errors::BondError;
use crate::events::RecordWindowOpened;
use crate::state::*;

#[derive(Accounts)]
pub struct OpenRecordWindow<'info> {
    #[account(mut)]
    pub issuer: Signer<'info>,

    #[account(mut, has_one = issuer @ BondError::Unauthorized)]
    pub bond: Account<'info, BondState>,

    /// One audit record per record-date window (epoch = previous epoch + 1).
    #[account(
        init,
        payer = issuer,
        space = 8 + SnapshotRecord::INIT_SPACE,
        seeds = [SNAPSHOT_SEED, bond.key().as_ref(), &(bond.record_epoch + 1).to_le_bytes()],
        bump
    )]
    pub snapshot_record: Account<'info, SnapshotRecord>,

    pub system_program: Program<'info, System>,
}

pub fn handle_open_record_window(ctx: Context<OpenRecordWindow>) -> Result<()> {
    let clock = Clock::get()?;
    let bond = &mut ctx.accounts.bond;
    require!(bond.status == BondStatus::Active, BondError::BondNotActive);
    require!(!bond.window_open, BondError::WindowOpen);

    bond.record_epoch = bond
        .record_epoch
        .checked_add(1)
        .ok_or(BondError::MathOverflow)?;
    bond.window_open = true;
    bond.record_slot = clock.slot;
    bond.snapshotted_supply = 0;

    let rec = &mut ctx.accounts.snapshot_record;
    rec.bond = bond.key();
    rec.epoch = bond.record_epoch;
    rec.opened_slot = clock.slot;
    rec.opened_ts = clock.unix_timestamp;
    rec.closed = false;
    rec.closed_slot = 0;
    rec.closed_ts = 0;
    rec.total_supply = 0;
    rec.holder_count = 0;
    rec.bump = ctx.bumps.snapshot_record;

    emit!(RecordWindowOpened {
        bond: bond.key(),
        epoch: bond.record_epoch,
        slot: clock.slot,
    });
    Ok(())
}