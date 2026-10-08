use anchor_lang::prelude::*;

pub mod errors;
pub mod events;
pub mod instructions;
pub mod math;
pub mod state;

use instructions::*;
use state::Jurisdiction;

declare_id!("GsYLz1ieJSo267DW3oga1k4hAmVoqTHgKaEw2K5HxJPT");

#[program]
pub mod bond_core {
    use super::*;

    pub fn issue_bond(ctx: Context<IssueBond>, params: IssueBondParams) -> Result<()> {
        instructions::issue_bond::handle_issue_bond(ctx, params)
    }

    pub fn register_holder(
        ctx: Context<RegisterHolder>,
        jurisdiction: Jurisdiction,
    ) -> Result<()> {
        instructions::register_holder::handle_register_holder(ctx, jurisdiction)
    }

    pub fn mint_bonds(ctx: Context<MintBonds>, amount: u64) -> Result<()> {
        instructions::mint_bonds::handle_mint_bonds(ctx, amount)
    }

    pub fn open_record_window(ctx: Context<OpenRecordWindow>) -> Result<()> {
        instructions::open_record_window::handle_open_record_window(ctx)
    }

    pub fn snapshot_holder(ctx: Context<SnapshotHolder>) -> Result<()> {
        instructions::snapshot_holder::handle_snapshot_holder(ctx)
    }

    pub fn close_record_window(ctx: Context<CloseRecordWindow>) -> Result<()> {
        instructions::close_record_window::handle_close_record_window(ctx)
    }

    pub fn thaw_holder(ctx: Context<ThawHolder>) -> Result<()> {
        instructions::thaw_holder::handle_thaw_holder(ctx)
    }

    pub fn mark_redeemed(ctx: Context<MarkRedeemed>) -> Result<()> {
        instructions::mark_redeemed::handle_mark_redeemed(ctx)
    }
}
