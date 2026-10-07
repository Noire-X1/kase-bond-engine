use anchor_lang::prelude::*;

pub mod errors;
pub mod events;
pub mod instructions;
pub mod state;

use instructions::*;

declare_id!("59hoGMNeum386CpHQpbsUStq51vfXS9iYxPkq5apthNg");

#[program]
pub mod corporate_actions {
    use super::*;

    pub fn initialize_ledger(ctx: Context<InitializeLedger>, total_coupons: u16) -> Result<()> {
        instructions::initialize_ledger::handle_initialize_ledger(ctx, total_coupons)
    }
}
