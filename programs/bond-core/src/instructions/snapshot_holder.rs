use anchor_lang::prelude::*;
use anchor_spl::token::{self, FreezeAccount, Mint, Token, TokenAccount};

use crate::errors::BondError;
use crate::events::HolderSnapshotted;
use crate::state::*;

/// Permissionless: anyone can crank a holder's snapshot while the window is open.
#[derive(Accounts)]
pub struct SnapshotHolder<'info> {
    pub cranker: Signer<'info>,

    #[account(mut)]
    pub bond: Account<'info, BondState>,

    #[account(address = bond.bond_mint @ BondError::WrongMint)]
    pub bond_mint: Account<'info, Mint>,

    #[account(mut, has_one = bond @ BondError::WrongBond)]
    pub holder_record: Account<'info, HolderRecord>,

    #[account(
        mut,
        address = holder_record.token_account @ BondError::WrongTokenAccount
    )]
    pub holder_token_account: Account<'info, TokenAccount>,

    #[account(
        mut,
        seeds = [SNAPSHOT_SEED, bond.key().as_ref(), &bond.record_epoch.to_le_bytes()],
        bump = snapshot_record.bump
    )]
    pub snapshot_record: Account<'info, SnapshotRecord>,

    pub token_program: Program<'info, Token>,
}

pub fn handle_snapshot_holder(ctx: Context<SnapshotHolder>) -> Result<()> {
    let bond = &ctx.accounts.bond;
    require!(bond.window_open, BondError::WindowNotOpen);
    let epoch = bond.record_epoch;
    require!(
        ctx.accounts.holder_record.snapshot_epoch != epoch,
        BondError::AlreadySnapshotted
    );

    let issuer = bond.issuer;
    let bond_id_bytes = bond.bond_id.to_le_bytes();
    let bump = [bond.bump];
    let seeds: &[&[u8]] = &[BOND_SEED, issuer.as_ref(), &bond_id_bytes, &bump];
    let signer_seeds = &[seeds];

    let amount = ctx.accounts.holder_token_account.amount;

    // Freeze so the recorded balance is binding. If the account is still frozen
    // from an earlier window it cannot have changed, so it is already safe.
    if !ctx.accounts.holder_token_account.is_frozen() {
        token::freeze_account(CpiContext::new_with_signer(
            ctx.accounts.token_program.key(),
            FreezeAccount {
                account: ctx.accounts.holder_token_account.to_account_info(),
                mint: ctx.accounts.bond_mint.to_account_info(),
                authority: ctx.accounts.bond.to_account_info(),
            },
            signer_seeds,
        ))?;
    }

    let slot = Clock::get()?.slot;

    let hr = &mut ctx.accounts.holder_record;
    hr.snapshot_balance = amount;
    hr.snapshot_epoch = epoch;
    hr.snapshot_slot = slot;

    let bond = &mut ctx.accounts.bond;
    bond.snapshotted_supply = bond
        .snapshotted_supply
        .checked_add(amount)
        .ok_or(BondError::MathOverflow)?;

    let sr = &mut ctx.accounts.snapshot_record;
    sr.holder_count = sr
        .holder_count
        .checked_add(1)
        .ok_or(BondError::MathOverflow)?;

    emit!(HolderSnapshotted {
        bond: bond.key(),
        wallet: hr.wallet,
        epoch,
        balance: amount,
        slot,
    });
    Ok(())
}