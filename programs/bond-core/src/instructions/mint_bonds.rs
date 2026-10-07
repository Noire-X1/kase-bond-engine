use anchor_lang::prelude::*;
use anchor_spl::token::{self, Mint, MintTo, Token, TokenAccount};

use crate::errors::BondError;
use crate::events::BondsMinted;
use crate::state::*;

#[derive(Accounts)]
pub struct MintBonds<'info> {
    pub issuer: Signer<'info>,

    #[account(mut, has_one = issuer @ BondError::Unauthorized)]
    pub bond: Account<'info, BondState>,

    #[account(mut, address = bond.bond_mint @ BondError::WrongMint)]
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

pub fn handle_mint_bonds(ctx: Context<MintBonds>, amount: u64) -> Result<()> {
    let bond = &ctx.accounts.bond;
    require!(bond.status == BondStatus::Active, BondError::BondNotActive);
    require!(!bond.window_open, BondError::WindowOpen);
    require!(amount > 0, BondError::InvalidAmount);
    let new_minted = bond
        .minted_supply
        .checked_add(amount)
        .ok_or(BondError::MathOverflow)?;
    require!(new_minted <= bond.max_supply, BondError::MaxSupplyExceeded);

    let issuer = bond.issuer;
    let bond_id_bytes = bond.bond_id.to_le_bytes();
    let bump = [bond.bump];
    let seeds: &[&[u8]] = &[BOND_SEED, issuer.as_ref(), &bond_id_bytes, &bump];
    let signer_seeds = &[seeds];

    token::mint_to(
        CpiContext::new_with_signer(
            ctx.accounts.token_program.key(),
            MintTo {
                mint: ctx.accounts.bond_mint.to_account_info(),
                to: ctx.accounts.holder_token_account.to_account_info(),
                authority: ctx.accounts.bond.to_account_info(),
            },
            signer_seeds,
        ),
        amount,
    )?;

    ctx.accounts.bond.minted_supply = new_minted;

    emit!(BondsMinted {
        bond: ctx.accounts.bond.key(),
        wallet: ctx.accounts.holder_record.wallet,
        amount,
        minted_supply: new_minted,
    });
    Ok(())
}