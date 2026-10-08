//! governance state.
//!
//! TIMING RULE (repeated in the technical overview): voting weight is the holder's
//! snapshot balance read from bond-core's HolderRecord, which keeps only the LATEST
//! snapshot and is overwritten when the next record window snapshots that holder.
//! A proposal's votes must therefore finish BEFORE the next record window opens;
//! after that, cast_vote refuses with StaleSnapshot. The result stored by
//! finalize_proposal does not depend on holder records, so it stays valid afterwards.

use anchor_lang::prelude::*;

pub const PROPOSAL_SEED: &[u8] = b"proposal";
pub const VOTE_SEED: &[u8] = b"vote";
pub const MAX_TITLE_LEN: usize = 64;
pub const MAX_DESCRIPTION_LEN: usize = 200;

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, InitSpace)]
pub enum VoteChoice {
    For,
    Against,
    Abstain,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, InitSpace)]
pub enum ProposalStatus {
    Open,
    Passed,
    Rejected,
    NoQuorum,
}

/// One bondholder proposal. PDA: ["proposal", bond, proposal_id (u32 LE)]
/// After finalize_proposal the status and tallies never change.
#[account]
#[derive(InitSpace)]
pub struct Proposal {
    pub bond: Pubkey,
    pub issuer: Pubkey,
    pub proposal_id: u32,
    #[max_len(64)]
    pub title: String,
    #[max_len(200)]
    pub description: String,
    /// Snapshot reference: the bond-core record-window epoch whose balances are the weights.
    pub epoch: u32,
    /// Total bond supply at that snapshot (the quorum denominator).
    pub snapshot_total_supply: u64,
    pub start_ts: i64,
    pub end_ts: i64,
    /// Minimum turnout, in basis points of the snapshot supply (0 = no quorum).
    pub quorum_bps: u16,
    pub votes_for: u64,
    pub votes_against: u64,
    pub votes_abstain: u64,
    pub voter_count: u32,
    pub status: ProposalStatus,
    pub created_slot: u64,
    pub created_ts: i64,
    pub finalized_ts: i64,
    pub bump: u8,
}

/// One vote per (proposal, holder); its existence blocks double voting.
/// PDA: ["vote", proposal, wallet]
#[account]
#[derive(InitSpace)]
pub struct VoteRecord {
    pub proposal: Pubkey,
    pub wallet: Pubkey,
    pub choice: VoteChoice,
    /// The holder's snapshot balance at the proposal's epoch.
    pub weight: u64,
    pub voted_ts: i64,
    pub bump: u8,
}
