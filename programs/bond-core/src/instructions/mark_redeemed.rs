use anchor_lang::prelude::*;
use anchor_spl::token::Mint;

use crate::errors::BondError;
use crate::events::BondRedeemed;
use crate::state::*;

/// Permissionless. Marks a bond Redeemed once maturity has passed and every
/// bond token has been burned (mint supply == 0). The burn itself is the proof,
/// so no cross-program trust is needed.
#[derive(Accounts)]
pub struct MarkRedeemed<'info> {
    pub cranker: Signer<'info>,

    #[account(mut)]
    pub bond: Account<'info, BondState>,

    #[account(address = bond.bond_mint @ BondError::WrongMint)]
    pub bond_mint: Account<'info, Mint>,
}

pub fn handle_mark_redeemed(ctx: Context<MarkRedeemed>) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    let bond = &mut ctx.accounts.bond;
    require!(bond.status == BondStatus::Active, BondError::BondNotActive);
    require!(now >= bond.maturity_ts, BondError::NotMatured);
    require!(bond.minted_supply > 0, BondError::NothingIssued);
    require!(
        ctx.accounts.bond_mint.supply == 0,
        BondError::SupplyOutstanding
    );

    bond.status = BondStatus::Redeemed;

    emit!(BondRedeemed {
        bond: bond.key(),
        burned_supply: bond.minted_supply,
        ts: now,
    });
    Ok(())
}
