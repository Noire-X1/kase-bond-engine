use anchor_lang::prelude::*;
use bond_core::state::{BondState, HolderRecord};

use crate::errors::GovError;
use crate::events::VoteCast;
use crate::state::*;

/// Holder-signed. The vote's weight is the holder's snapshot balance for the
/// proposal's epoch. The VoteRecord PDA (proposal, wallet) is created here, so a
/// second vote by the same wallet fails.
///
/// TIMING: once the next record window snapshots this holder, the holder record's
/// epoch no longer matches and this refuses with StaleSnapshot.
#[derive(Accounts)]
pub struct CastVote<'info> {
    #[account(mut)]
    pub voter: Signer<'info>,

    pub bond: Account<'info, BondState>,

    #[account(
        mut,
        seeds = [PROPOSAL_SEED, bond.key().as_ref(), &proposal.proposal_id.to_le_bytes()],
        bump = proposal.bump,
        has_one = bond @ GovError::WrongBond
    )]
    pub proposal: Account<'info, Proposal>,

    #[account(
        constraint = holder_record.bond == bond.key() @ GovError::WrongBond,
        constraint = holder_record.wallet == voter.key() @ GovError::Unauthorized,
        constraint = holder_record.snapshot_epoch == proposal.epoch @ GovError::StaleSnapshot
    )]
    pub holder_record: Account<'info, HolderRecord>,

    #[account(
        init,
        payer = voter,
        space = 8 + VoteRecord::INIT_SPACE,
        seeds = [VOTE_SEED, proposal.key().as_ref(), voter.key().as_ref()],
        bump
    )]
    pub vote_record: Account<'info, VoteRecord>,

    pub system_program: Program<'info, System>,
}

pub fn handle_cast_vote(ctx: Context<CastVote>, choice: VoteChoice) -> Result<()> {
    let now = Clock::get()?.unix_timestamp;
    let weight = ctx.accounts.holder_record.snapshot_balance;
    let voter_key = ctx.accounts.voter.key();

    let proposal = &mut ctx.accounts.proposal;
    require!(
        proposal.status == ProposalStatus::Open,
        GovError::ProposalNotOpen
    );
    require!(now >= proposal.start_ts, GovError::VotingNotStarted);
    require!(now < proposal.end_ts, GovError::VotingEnded);
    require!(weight > 0, GovError::NoVotingPower);

    match choice {
        VoteChoice::For => {
            proposal.votes_for = proposal
                .votes_for
                .checked_add(weight)
                .ok_or(GovError::MathOverflow)?;
        }
        VoteChoice::Against => {
            proposal.votes_against = proposal
                .votes_against
                .checked_add(weight)
                .ok_or(GovError::MathOverflow)?;
        }
        VoteChoice::Abstain => {
            proposal.votes_abstain = proposal
                .votes_abstain
                .checked_add(weight)
                .ok_or(GovError::MathOverflow)?;
        }
    }
    proposal.voter_count = proposal
        .voter_count
        .checked_add(1)
        .ok_or(GovError::MathOverflow)?;

    let vote = &mut ctx.accounts.vote_record;
    vote.proposal = proposal.key();
    vote.wallet = voter_key;
    vote.choice = choice;
    vote.weight = weight;
    vote.voted_ts = now;
    vote.bump = ctx.bumps.vote_record;

    emit!(VoteCast {
        proposal: proposal.key(),
        wallet: voter_key,
        choice,
        weight,
    });
    Ok(())
}
