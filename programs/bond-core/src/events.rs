use anchor_lang::prelude::*;

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