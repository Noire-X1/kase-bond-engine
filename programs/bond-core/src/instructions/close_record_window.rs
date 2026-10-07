use anchor_lang::prelude::*;
use anchor_spl::token::Mint;

use crate::errors::BondError;
use crate::events::RecordWindowClosed;
use crate::state::*;

#[derive(Accounts)]
pub struct CloseRecordWindow<'info> {
    pub issuer: Signer<'info>,

    #[account(mut, has_one = issuer @ BondError::Unauthorized)]
    pub bond: Account<'info, BondState>,

    #[account(address = bond.bond_mint @ BondError::WrongMint)]
    pub bond_mint: Account<'info, Mint>,

    #[account(
        mut,
        seeds = [SNAPSHOT_SEED, bond.key().as_ref(), &bond.record_epoch.to_le_bytes()],
        bump = snapshot_record.bump
    )]
    pub snapshot_record: Account<'info, SnapshotRecord>,
}

pub fn handle_close_record_window(ctx: Context<CloseRecordWindow>) -> Result<()> {
    let clock = Clock::get()?;
    let bond = &mut ctx.accounts.bond;
    require!(bond.window_open, BondError::WindowNotOpen);

    // Completeness: every bond in circulation must be accounted for exactly once.
    require!(
        bond.snapshotted_supply == ctx.accounts.bond_mint.supply,
        BondError::SnapshotIncomplete
    );

    bond.window_open = false;

    let sr = &mut ctx.accounts.snapshot_record;
    sr.closed = true;
    sr.closed_slot = clock.slot;
    sr.closed_ts = clock.unix_timestamp;
    sr.total_supply = bond.snapshotted_supply;

    emit!(RecordWindowClosed {
        bond: bond.key(),
        epoch: bond.record_epoch,
        total_supply: bond.snapshotted_supply,
        holder_count: sr.holder_count,
        slot: clock.slot,
    });
    Ok(())
}