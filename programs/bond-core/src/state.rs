use anchor_lang::prelude::*;

pub const BOND_SEED: &[u8] = b"bond";
pub const MINT_SEED: &[u8] = b"bond_mint";
pub const HOLDER_SEED: &[u8] = b"holder";
pub const SNAPSHOT_SEED: &[u8] = b"snapshot";

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, InitSpace)]
pub enum BondStatus {
    Active,
    Redeemed,
}

#[derive(AnchorSerialize, AnchorDeserialize, Clone, Copy, PartialEq, Eq, InitSpace)]
pub enum Jurisdiction {
    Domestic,
    Foreign,
}

#[account]
#[derive(InitSpace)]
pub struct BondState {
    pub issuer: Pubkey,
    pub bond_id: u64,
    pub bond_mint: Pubkey,
    /// Test SPL token standing in for KZT/USDC (simulated fiat rail).
    pub payment_mint: Pubkey,

    // Terms. All money amounts are in payment-mint base units.
    pub face_value: u64,
    pub coupon_rate_bps: u16,   // 1000 = 10.00% per year
    pub coupons_per_year: u8,   // periods
    pub issue_ts: i64,
    pub maturity_ts: i64,

    pub max_supply: u64,
    pub minted_supply: u64,

    // Withholding applied at claim time. Default 0 so the spec example is exact.
    pub withholding_bps_domestic: u16,
    pub withholding_bps_foreign: u16,

    pub status: BondStatus,

    // Record-date window state.
    pub record_epoch: u32,
    pub window_open: bool,
    pub record_slot: u64,
    pub snapshotted_supply: u64,

    pub bump: u8,
    pub mint_bump: u8,
}

/// Holder registry: one record per (bond, wallet).
#[account]
#[derive(InitSpace)]
pub struct HolderRecord {
    pub bond: Pubkey,
    pub wallet: Pubkey,
    /// The one token account that holds this wallet's bonds for this bond.
    pub token_account: Pubkey,
    pub jurisdiction: Jurisdiction,
    pub snapshot_balance: u64,
    pub snapshot_epoch: u32,
    pub snapshot_slot: u64,
    pub registered_ts: i64,
    pub bump: u8,
}

/// Persistent audit record of one record-date window (one per epoch).
#[account]
#[derive(InitSpace)]
pub struct SnapshotRecord {
    pub bond: Pubkey,
    pub epoch: u32,
    pub opened_slot: u64,
    pub opened_ts: i64,
    pub closed: bool,
    pub closed_slot: u64,
    pub closed_ts: i64,
    pub total_supply: u64,
    pub holder_count: u32,
    pub bump: u8,
}