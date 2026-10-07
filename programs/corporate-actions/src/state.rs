//! corporate-actions state.
//!
//! TIMING RULES (repeated in the technical overview):
//! 1. The record date is a WINDOW (bond-core: open_record_window ..
//!    close_record_window), not a single instant. Bonds are frozen per holder
//!    as each holder is snapshotted, and the window only closes once every
//!    bond in circulation has been counted exactly once.
//! 2. bond-core's HolderRecord keeps only the LATEST snapshot balance and is
//!    overwritten when the next window snapshots that holder. Therefore every
//!    entitlement calculation (and every governance vote that reads the
//!    snapshot) for an epoch must finish BEFORE the next record window opens.
//!    Claims may happen later: an Entitlement copies the quantity at
//!    calculation time, so it is unaffected by later windows.

use anchor_lang::prelude::*;
use bond_core::state::Jurisdiction;

pub const LEDGER_SEED: &[u8] = b"ledger";
pub const ACTION_SEED: &[u8] = b"action";
pub const ENTITLEMENT_SEED: &[u8] = b"entitlement";
pub const VAULT_SEED: &[u8] = b"vault";

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, InitSpace)]
pub enum ActionKind {
    Coupon,
    Redemption,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, InitSpace)]
pub enum ActionStatus {
    Created,
    Funded,
    Closed,
}

/// One per bond. Set once by the issuer; caps how many coupon actions exist.
/// PDA: ["ledger", bond]
#[account]
#[derive(InitSpace)]
pub struct BondLedger {
    pub bond: Pubkey,
    pub issuer: Pubkey,
    /// SIMULATED tax authority: a payment-token account that receives swept
    /// withholding. No real tax rail is integrated.
    pub tax_collector: Pubkey,
    /// Total coupons over the bond's life, INCLUDING the final coupon that is
    /// paid together with redemption. Regular coupon actions are therefore
    /// capped at total_coupons - 1.
    pub total_coupons: u16,
    pub coupons_created: u16,
    pub coupons_paid: u16,
    pub redemption_created: bool,
    pub created_ts: i64,
    pub bump: u8,
}

/// The persistent on-chain record of one corporate action.
/// One action per record window. PDA: ["action", bond, epoch]
#[account]
#[derive(InitSpace)]
pub struct CorporateAction {
    pub bond: Pubkey,
    pub issuer: Pubkey,
    pub kind: ActionKind,
    /// Snapshot reference: the bond-core record-window epoch this action uses.
    pub epoch: u32,
    /// Coupon number (1-based) for coupons; 0 for redemption.
    pub action_index: u16,
    pub payment_mint: Pubkey,
    /// Vault token account PDA: ["vault", action]
    pub vault: Pubkey,
    /// Copied from the ledger at creation (SIMULATED tax authority).
    pub tax_collector: Pubkey,
    pub status: ActionStatus,
    /// Copied from the SnapshotRecord at creation; calculation must reach it.
    pub snapshot_holder_count: u32,
    pub entitlement_count: u32,
    pub claimed_count: u32,
    pub total_gross: u64,
    pub total_withheld: u64,
    pub total_net: u64,
    pub total_claimed_net: u64,
    pub funded_amount: u64,
    pub withheld_swept: bool,
    pub created_slot: u64,
    pub created_ts: i64,
    pub funded_ts: i64,
    pub closed_ts: i64,
    pub bump: u8,
    pub vault_bump: u8,
}

/// What one holder is owed for one action. PDA: ["entitlement", action, wallet]
/// gross = principal + coupon_gross
/// withheld = withholding on coupon_gross ONLY (never on principal)
/// net = gross - withheld
#[account]
#[derive(InitSpace)]
pub struct Entitlement {
    pub action: Pubkey,
    pub wallet: Pubkey,
    pub jurisdiction: Jurisdiction,
    /// Snapshot quantity copied at calculation time.
    pub quantity: u64,
    pub principal: u64,
    pub coupon_gross: u64,
    pub gross: u64,
    pub withholding_bps: u16,
    pub withheld: u64,
    pub net: u64,
    pub claimed: bool,
    pub calculated_ts: i64,
    pub claimed_ts: i64,
    pub bump: u8,
}
