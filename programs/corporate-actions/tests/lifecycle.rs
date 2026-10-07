//! LiteSVM integration tests for the full bond lifecycle.
//! Run with ./scripts/test.sh, which first builds v0 copies of both programs
//! into target/litesvm (Anchor's default SBPF v3 output is not loadable by
//! LiteSVM 0.11).

use anchor_lang::prelude::Pubkey;
use anchor_lang::{AccountDeserialize, InstructionData, ToAccountMetas};
use bond_core::instructions::IssueBondParams;
use bond_core::state::{BondState, BondStatus, BOND_SEED, MINT_SEED};
use litesvm::LiteSVM;
use litesvm_token::CreateMint;
use solana_instruction::Instruction;
use solana_keypair::Keypair;
use solana_signer::Signer;
use solana_transaction::Transaction;

const BOND_CORE_SO: &str =
    concat!(env!("CARGO_MANIFEST_DIR"), "/../../target/litesvm/bond_core.so");
const ACTIONS_SO: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../target/litesvm/corporate_actions.so"
);

// ---------------------------------------------------------------- helpers

/// Anchor Pubkey of a keypair (converted via bytes so type differences between
/// crates cannot bite).
fn pk(kp: &Keypair) -> Pubkey {
    Pubkey::new_from_array(kp.pubkey().to_bytes())
}

fn setup() -> (LiteSVM, Keypair) {
    let mut svm = LiteSVM::new();
    svm.add_program_from_file(bond_core::ID, BOND_CORE_SO)
        .unwrap();
    svm.add_program_from_file(corporate_actions::ID, ACTIONS_SO)
        .unwrap();
    let issuer = Keypair::new();
    svm.airdrop(&issuer.pubkey(), 100_000_000_000).unwrap();
    (svm, issuer)
}

/// Sends a transaction; on failure returns the debug text (includes program logs).
fn send(
    svm: &mut LiteSVM,
    ixs: &[Instruction],
    payer: &Keypair,
    extra: &[&Keypair],
) -> Result<(), String> {
    let mut signers: Vec<&Keypair> = vec![payer];
    signers.extend_from_slice(extra);
    let tx = Transaction::new_signed_with_payer(
        ixs,
        Some(&payer.pubkey()),
        &signers,
        svm.latest_blockhash(),
    );
    let result = svm.send_transaction(tx);
    // New blockhash so identical-looking transactions are never rejected as duplicates.
    svm.expire_blockhash();
    result.map(|_| ()).map_err(|e| format!("{e:?}"))
}

fn send_ok(svm: &mut LiteSVM, ixs: &[Instruction], payer: &Keypair, extra: &[&Keypair]) {
    send(svm, ixs, payer, extra).unwrap_or_else(|e| panic!("transaction failed:\n{e}"));
}

fn read_bond(svm: &LiteSVM, bond: &Pubkey) -> BondState {
    let data = svm
        .get_account(&bond.to_bytes().into())
        .expect("bond account missing")
        .data;
    BondState::try_deserialize(&mut data.as_slice()).unwrap()
}

/// Builds the issue_bond instruction. Returns (instruction, bond PDA, bond mint PDA).
fn issue_bond_ix(
    issuer: &Keypair,
    payment_mint: Pubkey,
    bond_id: u64,
    face_value: u64,
) -> (Instruction, Pubkey, Pubkey) {
    let (bond, _) = Pubkey::find_program_address(
        &[BOND_SEED, pk(issuer).as_ref(), &bond_id.to_le_bytes()],
        &bond_core::ID,
    );
    let (bond_mint, _) =
        Pubkey::find_program_address(&[MINT_SEED, bond.as_ref()], &bond_core::ID);

    let params = IssueBondParams {
        bond_id,
        face_value,
        coupon_rate_bps: 1_000, // 10%
        coupons_per_year: 2,    // semi-annual
        maturity_ts: 2_000_000_000,
        max_supply: 1_000,
        withholding_bps_domestic: 0,
        withholding_bps_foreign: 0,
    };

    let ix = Instruction {
        program_id: bond_core::ID,
        accounts: bond_core::accounts::IssueBond {
            issuer: pk(issuer),
            bond,
            bond_mint,
            payment_mint,
            token_program: anchor_spl::token::ID,
            system_program: anchor_lang::system_program::ID,
        }
        .to_account_metas(None),
        data: bond_core::instruction::IssueBond { params }.data(),
    };
    (ix, bond, bond_mint)
}

// ------------------------------------------------------------------ tests

#[test]
fn issue_bond_creates_bond_state() {
    let (mut svm, issuer) = setup();

    // SIMULATED fiat rail: a test SPL token standing in for KZT/USDC.
    let payment_mint = CreateMint::new(&mut svm, &issuer)
        .decimals(6)
        .send()
        .unwrap();
    let payment_mint = Pubkey::new_from_array(payment_mint.to_bytes());

    let (ix, bond, bond_mint) = issue_bond_ix(&issuer, payment_mint, 1, 1_000);
    send_ok(&mut svm, &[ix], &issuer, &[]);

    let state = read_bond(&svm, &bond);
    assert_eq!(state.issuer, pk(&issuer));
    assert_eq!(state.bond_id, 1);
    assert_eq!(state.bond_mint, bond_mint);
    assert_eq!(state.payment_mint, payment_mint);
    assert_eq!(state.face_value, 1_000);
    assert_eq!(state.coupon_rate_bps, 1_000);
    assert_eq!(state.coupons_per_year, 2);
    assert_eq!(state.minted_supply, 0);
    assert_eq!(state.withholding_bps_domestic, 0);
    assert_eq!(state.withholding_bps_foreign, 0);
    assert_eq!(state.record_epoch, 0);
    assert!(!state.window_open);
    assert!(state.status == BondStatus::Active);
}

#[test]
fn issue_bond_rejects_zero_face_value() {
    let (mut svm, issuer) = setup();
    let payment_mint = CreateMint::new(&mut svm, &issuer)
        .decimals(6)
        .send()
        .unwrap();
    let payment_mint = Pubkey::new_from_array(payment_mint.to_bytes());

    let (ix, _bond, _mint) = issue_bond_ix(&issuer, payment_mint, 2, 0);
    let err = send(&mut svm, &[ix], &issuer, &[]).expect_err("zero face value must be rejected");
    assert!(
        err.contains("InvalidFaceValue"),
        "expected InvalidFaceValue, got:\n{err}"
    );
}
