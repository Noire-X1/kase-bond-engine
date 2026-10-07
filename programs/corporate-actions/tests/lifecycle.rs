//! LiteSVM integration tests for the full bond lifecycle.
//! Requires `anchor build` first: the tests load the compiled .so files.

use litesvm::LiteSVM;
use litesvm_token::spl_token::state::Mint;
use litesvm_token::{get_spl_account, CreateMint};
use solana_keypair::Keypair;
use solana_signer::Signer;

const BOND_CORE_SO: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/litesvm/bond_core.so");
const ACTIONS_SO: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../target/litesvm/corporate_actions.so"
);

#[test]
fn programs_load_and_payment_mint_can_be_created() {
    let svm = &mut LiteSVM::new();
    svm.add_program_from_file(bond_core::ID, BOND_CORE_SO)
        .unwrap();
    svm.add_program_from_file(corporate_actions::ID, ACTIONS_SO)
        .unwrap();

    let payer = Keypair::new();
    svm.airdrop(&payer.pubkey(), 10_000_000_000).unwrap();

    // SIMULATED fiat rail: a test SPL token standing in for KZT/USDC.
    let mint = CreateMint::new(svm, &payer).decimals(6).send().unwrap();
    let mint_state: Mint = get_spl_account(svm, &mint).unwrap();
    assert_eq!(mint_state.decimals, 6);
}
