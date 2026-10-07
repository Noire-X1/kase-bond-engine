use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, ThawAccount, Token, TokenAccount};

use crate::errors::BondError;
use crate::events::HolderThawed;
use crate::state::*;

/// Permissionless: anyone can thaw a holder once the record window is closed.
#[derive(Accounts)]
pub struct ThawHolder<'info> {
    pub cranker: Signer<'info>,

    pub bond: Account<'info, BondState>,

    #[account(address = bond.bond_mint @ BondError::WrongMint)]
    pub bond_mint: Account<'info, Mint>,

    #[account(has_one = bond @ BondError::WrongBond)]
    pub holder_record: Account<'info, HolderRecord>,

    #[account(
        mut,
        address = holder_record.token_account @ BondError::WrongTokenAccount
    )]
    pub holder_token_account: Account<'info, TokenAccount>,

    pub token_program: Program<'info, Token>,
}

pub fn handle_thaw_holder(ctx: Context<ThawHolder>) -> Result<()> {
    let bond = &ctx.accounts.bond;
    require!(!bond.window_open, BondError::WindowOpen);
    require!(
        ctx.accounts.holder_token_account.is_frozen(),
        BondError::AccountNotFrozen
    );

    let issuer = bond.issuer;
    let bond_id_bytes = bond.bond_id.to_le_bytes();
    let bump = [bond.bump];
    let seeds: &[&[u8]] = &[BOND_SEED, issuer.as_ref(), &bond_id_bytes, &bump];
    let signer_seeds = &[seeds];

    token::thaw_account(CpiContext::new_with_signer(
        ctx.accounts.token_program.key(),
        ThawAccount {
            account: ctx.accounts.holder_token_account.to_account_info(),
            mint: ctx.accounts.bond_mint.to_account_info(),
            authority: ctx.accounts.bond.to_account_info(),
        },
        signer_seeds,
    ))?;

    emit!(HolderThawed {
        bond: ctx.accounts.bond.key(),
        wallet: ctx.accounts.holder_record.wallet,
    });
    Ok(())
}