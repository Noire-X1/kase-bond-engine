use anchor_lang::prelude::*;

pub mod errors;
pub mod events;
pub mod instructions;
pub mod state;

use instructions::*;
use state::VoteChoice;

declare_id!("D4Zh5HNxQwuzqFJFsw5BFNBfkAGtW7Efk5VRTnFNwEqg");

#[program]
pub mod governance {
    use super::*;

    pub fn create_proposal(
        ctx: Context<CreateProposal>,
        proposal_id: u32,
        title: String,
        description: String,
        start_ts: i64,
        end_ts: i64,
        quorum_bps: u16,
    ) -> Result<()> {
        instructions::create_proposal::handle_create_proposal(
            ctx,
            proposal_id,
            title,
            description,
            start_ts,
            end_ts,
            quorum_bps,
        )
    }

    pub fn cast_vote(ctx: Context<CastVote>, choice: VoteChoice) -> Result<()> {
        instructions::cast_vote::handle_cast_vote(ctx, choice)
    }

    pub fn finalize_proposal(ctx: Context<FinalizeProposal>) -> Result<()> {
        instructions::finalize_proposal::handle_finalize_proposal(ctx)
    }
}
