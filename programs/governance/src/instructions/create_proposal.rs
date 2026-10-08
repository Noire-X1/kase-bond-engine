use anchor_lang::prelude::*;
use bond_core::state::{BondState, BondStatus, SnapshotRecord};

use crate::errors::GovError;
use crate::events::ProposalCreated;
use crate::state::*;

/// Issuer-only. Opens a proposal whose voting weights are the balances recorded in a
/// closed record window (the latest one). Creating a proposal is its own on-chain
/// record (the Proposal account) and emits ProposalCreated.
#[derive(Accounts)]
#[instruction(proposal_id: u32)]
pub struct CreateProposal<'info> {
    #[account(mut)]
    pub issuer: Signer<'info>,

    #[account(has_one = issuer @ GovError::Unauthorized)]
    pub bond: Account<'info, BondState>,

    #[account(
        constraint = snapshot_record.bond == bond.key() @ GovError::WrongBond,
        constraint = snapshot_record.epoch == bond.record_epoch @ GovError::StaleSnapshot
    )]
    pub snapshot_record: Account<'info, SnapshotRecord>,

    #[account(
        init,
        payer = issuer,
        space = 8 + Proposal::INIT_SPACE,
        seeds = [PROPOSAL_SEED, bond.key().as_ref(), &proposal_id.to_le_bytes()],
        bump
    )]
    pub proposal: Account<'info, Proposal>,

    pub system_program: Program<'info, System>,
}

pub fn handle_create_proposal(
    ctx: Context<CreateProposal>,
    proposal_id: u32,
    title: String,
    description: String,
    start_ts: i64,
    end_ts: i64,
    quorum_bps: u16,
) -> Result<()> {
    let clock = Clock::get()?;
    require!(
        ctx.accounts.bond.status == BondStatus::Active,
        GovError::BondNotActive
    );
    require!(
        ctx.accounts.snapshot_record.closed,
        GovError::SnapshotNotClosed
    );
    require!(
        !title.is_empty() && title.len() <= MAX_TITLE_LEN,
        GovError::InvalidTitle
    );
    require!(
        description.len() <= MAX_DESCRIPTION_LEN,
        GovError::InvalidDescription
    );
    require!(
        end_ts > start_ts && end_ts > clock.unix_timestamp,
        GovError::InvalidWindow
    );
    require!(quorum_bps <= 10_000, GovError::InvalidQuorum);

    let bond_key = ctx.accounts.bond.key();
    let issuer_key = ctx.accounts.issuer.key();
    let epoch = ctx.accounts.snapshot_record.epoch;
    let supply = ctx.accounts.snapshot_record.total_supply;

    let p = &mut ctx.accounts.proposal;
    p.bond = bond_key;
    p.issuer = issuer_key;
    p.proposal_id = proposal_id;
    p.title = title;
    p.description = description;
    p.epoch = epoch;
    p.snapshot_total_supply = supply;
    p.start_ts = start_ts;
    p.end_ts = end_ts;
    p.quorum_bps = quorum_bps;
    p.votes_for = 0;
    p.votes_against = 0;
    p.votes_abstain = 0;
    p.voter_count = 0;
    p.status = ProposalStatus::Open;
    p.created_slot = clock.slot;
    p.created_ts = clock.unix_timestamp;
    p.finalized_ts = 0;
    p.bump = ctx.bumps.proposal;

    emit!(ProposalCreated {
        bond: bond_key,
        proposal: p.key(),
        proposal_id,
        epoch,
        start_ts,
        end_ts,
        quorum_bps,
        snapshot_total_supply: supply,
    });
    Ok(())
}
