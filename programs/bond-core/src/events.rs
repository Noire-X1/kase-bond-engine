use anchor_lang::prelude::*;

use crate::state::Jurisdiction;

#[event]
pub struct BondIssued {
    pub bond: Pubkey,
    pub issuer: Pubkey,
    pub bond_mint: Pubkey,
    pub payment_mint: Pubkey,
    pub face_value: u64,
    pub coupon_rate_bps: u16,
    pub coupons_per_year: u8,
    pub maturity_ts: i64,
    pub max_supply: u64,
}

#[event]
pub struct HolderRegistered {
    pub bond: Pubkey,
    pub wallet: Pubkey,
    pub token_account: Pubkey,
    pub jurisdiction: Jurisdiction,
}

#[event]
pub struct BondsMinted {
    pub bond: Pubkey,
    pub wallet: Pubkey,
    pub amount: u64,
    pub minted_supply: u64,
}

#[event]
pub struct RecordWindowOpened {
    pub bond: Pubkey,
    pub epoch: u32,
    pub slot: u64,
}

#[event]
pub struct HolderSnapshotted {
    pub bond: Pubkey,
    pub wallet: Pubkey,
    pub epoch: u32,
    pub balance: u64,
    pub slot: u64,
}

#[event]
pub struct RecordWindowClosed {
    pub bond: Pubkey,
    pub epoch: u32,
    pub total_supply: u64,
    pub holder_count: u32,
    pub slot: u64,
}

#[event]
pub struct HolderThawed {
    pub bond: Pubkey,
    pub wallet: Pubkey,
}