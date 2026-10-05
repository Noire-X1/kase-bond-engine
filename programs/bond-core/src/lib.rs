use anchor_lang::prelude::*;

pub mod errors;
pub mod events;
pub mod instructions;
pub mod math;
pub mod state;

use instructions::*;

declare_id!("GsYLz1ieJSo267DW3oga1k4hAmVoqTHgKaEw2K5HxJPT");

#[program]
pub mod bond_core {
    use super::*;

    pub fn issue_bond(ctx: Context<IssueBond>, params: IssueBondParams) -> Result<()> {
        instructions::issue_bond::handle_issue_bond(ctx, params)
    }
}