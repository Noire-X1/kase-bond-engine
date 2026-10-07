use anchor_lang::prelude::*;

// Compile probe: proves the bond-core dependency (cpi feature) resolves.
#[allow(unused_imports)]
use bond_core::state::{BondState, HolderRecord, SnapshotRecord};

declare_id!("59hoGMNeum386CpHQpbsUStq51vfXS9iYxPkq5apthNg");

#[program]
pub mod corporate_actions {
    use super::*;
}