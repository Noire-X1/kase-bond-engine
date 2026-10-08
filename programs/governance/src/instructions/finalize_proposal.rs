use anchor_lang::prelude::*;
use bond_core::state::BondState;

use crate::errors::GovError;
use crate::events::ProposalFinalized;
use crate::state::*;

/// Permissionless, after the voting window ends. Stores the immutable result:
/// NoQuorum if turnout is below the quorum, otherwise Passed if For > Against,
/// else Rejected. A finalized proposal can never be finalized again.
#[derive(Accounts)]
pub struct FinalizeProposal<'info> {
    pub cranker: Signer<'info>,

    pub bond: Account<'info, BondState>,

    #[account(
        mut,
        seeds = [PROPOSAL_SEED, bond.key().as_ref(), &proposal.proposal_id.to_le_bytes()],
        bump = proposal.bump,
        has_one = bond @ GovError::WrongBond
    )]
    pub proposal: Account<'info, Proposal>,
}

pub fn handle_finalize_proposal(ctx: Context<FinalizeProposal>) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    let p = &mut ctx.accounts.proposal;
    require!(
        p.status == ProposalStatus::Open,
        GovError::AlreadyFinalized
    );
    require!(now >= p.end_ts, GovError::VotingStillOpen);

    let voted = p
        .votes_for
        .checked_add(p.votes_against)
        .and_then(|v| v.checked_add(p.votes_abstain))
        .ok_or(GovError::MathOverflow)?;

    // Quorum: voted / snapshot_supply >= quorum_bps / 10_000 (integer-safe).
    let lhs = (voted as u128)
        .checked_mul(10_000)
        .ok_or(GovError::MathOverflow)?;
    let rhs = (p.snapshot_total_supply as u128)
        .checked_mul(p.quorum_bps as u128)
        .ok_or(GovError::MathOverflow)?;

    p.status = if lhs < rhs {
        ProposalStatus::NoQuorum
    } else if p.votes_for > p.votes_against {
        ProposalStatus::Passed
    } else {
        ProposalStatus::Rejected
    };
    p.finalized_ts = now;

    emit!(ProposalFinalized {
        proposal: p.key(),
        status: p.status,
        votes_for: p.votes_for,
        votes_against: p.votes_against,
        votes_abstain: p.votes_abstain,
        voter_count: p.voter_count,
    });
    Ok(())
}
