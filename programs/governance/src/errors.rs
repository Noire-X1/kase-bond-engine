use anchor_lang::prelude::*;

#[error_code]
pub enum GovError {
    #[msg("Only the bond issuer can do this")]
    Unauthorized,
    #[msg("Account belongs to a different bond")]
    WrongBond,
    #[msg("Holder snapshot does not belong to this proposal's epoch")]
    StaleSnapshot,
    #[msg("Record window for this epoch is not closed")]
    SnapshotNotClosed,
    #[msg("Bond is not active")]
    BondNotActive,
    #[msg("Title must be 1 to 64 bytes")]
    InvalidTitle,
    #[msg("Description must be at most 200 bytes")]
    InvalidDescription,
    #[msg("Voting window must end after it starts and after now")]
    InvalidWindow,
    #[msg("Quorum must be between 0 and 10000 bps")]
    InvalidQuorum,
    #[msg("Voting has not started")]
    VotingNotStarted,
    #[msg("Voting has ended")]
    VotingEnded,
    #[msg("Voting is still open")]
    VotingStillOpen,
    #[msg("Proposal is not open")]
    ProposalNotOpen,
    #[msg("Proposal already finalized")]
    AlreadyFinalized,
    #[msg("This holder had no bonds at the snapshot")]
    NoVotingPower,
    #[msg("Arithmetic overflow")]
    MathOverflow,
}
