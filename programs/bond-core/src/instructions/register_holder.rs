use anchor_lang::prelude::*;
use anchor_spl::token::{Mint, TokenAccount};

use crate::errors::BondError;
use crate::events::HolderRegistered;
use crate::state::*;

#[derive(Accounts)]
pub struct RegisterHolder<'info> {
    #[account(mut)]
    pub issuer: Signer<'info>,

    #[account(has_one = issuer @ BondError::Unauthorized)]
    pub bond: Account<'info, BondState>,

    #[account(address = bond.bond_mint @ BondError::WrongMint)]
    pub bond_mint: Account<'info, Mint>,

    /// CHECK: the investor wallet. Only its address is used (PDA seed and
    /// token-account owner check); it does not need to sign.
    pub wallet: UncheckedAccount<'info>,

    /// The investor's existing bond token account (must be owned by `wallet`).
    #[account(
        token::mint = bond_mint,
        token::authority = wallet
    )]
    pub holder_token_account: Account<'info, TokenAccount>,

    #[account(
        init,
        payer = issuer,
        space = 8 + HolderRecord::INIT_SPACE,
        seeds = [HOLDER_SEED, bond.key().as_ref(), wallet.key().as_ref()],
        bump
    )]
    pub holder_record: Account<'info, HolderRecord>,

    pub system_program: Program<'info, System>,
}

pub fn handle_register_holder(
    ctx: Context<RegisterHolder>,
    jurisdiction: Jurisdiction,
) -> Result<()> {
    require!(
        ctx.accounts.bond.status == BondStatus::Active,
        BondError::BondNotActive
    );

    let now = Clock::get()?.unix_timestamp;
    let rec = &mut ctx.accounts.holder_record;
    rec.bond = ctx.accounts.bond.key();
    rec.wallet = ctx.accounts.wallet.key();
    rec.token_account = ctx.accounts.holder_token_account.key();
    rec.jurisdiction = jurisdiction;
    rec.snapshot_balance = 0;
    rec.snapshot_epoch = 0;
    rec.snapshot_slot = 0;
    rec.registered_ts = now;
    rec.bump = ctx.bumps.holder_record;

    emit!(HolderRegistered {
        bond: rec.bond,
        wallet: rec.wallet,
        token_account: rec.token_account,
        jurisdiction,
    });
    Ok(())
}