use anchor_lang::prelude::*;

use crate::state::{ProposalStatus, VoteChoice};

#[event]
pub struct ProposalCreated {
    pub bond: Pubkey,
    pub proposal: Pubkey,
    pub proposal_id: u32,
    pub epoch: u32,
    pub start_ts: i64,
    pub end_ts: i64,
    pub quorum_bps: u16,
    pub snapshot_total_supply: u64,
}

#[event]
pub struct VoteCast {
    pub proposal: Pubkey,
    pub wallet: Pubkey,
    pub choice: VoteChoice,
    pub weight: u64,
}

#[event]
pub struct ProposalFinalized {
    pub proposal: Pubkey,
    pub status: ProposalStatus,
    pub votes_for: u64,
    pub votes_against: u64,
    pub votes_abstain: u64,
    pub voter_count: u32,
}
