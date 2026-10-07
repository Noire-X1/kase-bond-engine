use anchor_lang::prelude::*;

use crate::state::ActionKind;

#[event]
pub struct LedgerInitialized {
    pub bond: Pubkey,
    pub issuer: Pubkey,
    pub total_coupons: u16,
    pub tax_collector: Pubkey,
}

#[event]
pub struct ActionCreated {
    pub bond: Pubkey,
    pub action: Pubkey,
    pub kind: ActionKind,
    pub epoch: u32,
    pub action_index: u16,
    pub snapshot_holder_count: u32,
    pub slot: u64,
}

#[event]
pub struct EntitlementCalculated {
    pub action: Pubkey,
    pub wallet: Pubkey,
    pub quantity: u64,
    pub gross: u64,
    pub withheld: u64,
    pub net: u64,
}

#[event]
pub struct ActionFunded {
    pub action: Pubkey,
    pub amount: u64,
    pub entitlement_count: u32,
}

#[event]
pub struct EntitlementClaimed {
    pub action: Pubkey,
    pub wallet: Pubkey,
    pub net: u64,
}

#[event]
pub struct WithholdingSwept {
    pub action: Pubkey,
    pub amount: u64,
    pub tax_collector: Pubkey,
}