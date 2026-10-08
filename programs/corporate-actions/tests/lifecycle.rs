//! LiteSVM integration tests for the full bond lifecycle.
//! Run with ./scripts/test.sh, which first builds v0 copies of both programs
//! into target/litesvm (Anchor's default SBPF v3 output is not loadable by
//! LiteSVM 0.11).

use anchor_lang::prelude::Pubkey;
use anchor_lang::{AccountDeserialize, InstructionData, ToAccountMetas};
use bond_core::instructions::IssueBondParams;
use bond_core::state::{
    BondState, BondStatus, HolderRecord, Jurisdiction, SnapshotRecord, BOND_SEED, HOLDER_SEED,
    MINT_SEED, SNAPSHOT_SEED,
};
use litesvm::LiteSVM;
use litesvm_token::spl_token::state::Account as TokenAccountState;
use litesvm_token::{get_spl_account, CreateAssociatedTokenAccount, CreateMint};
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

fn account_data(svm: &LiteSVM, key: &Pubkey) -> Vec<u8> {
    svm.get_account(&key.to_bytes().into())
        .expect("account missing")
        .data
}

fn read_bond(svm: &LiteSVM, key: &Pubkey) -> BondState {
    BondState::try_deserialize(&mut account_data(svm, key).as_slice()).unwrap()
}

fn read_holder(svm: &LiteSVM, key: &Pubkey) -> HolderRecord {
    HolderRecord::try_deserialize(&mut account_data(svm, key).as_slice()).unwrap()
}

fn read_snapshot(svm: &LiteSVM, key: &Pubkey) -> SnapshotRecord {
    SnapshotRecord::try_deserialize(&mut account_data(svm, key).as_slice()).unwrap()
}

fn token_state(svm: &LiteSVM, key: &Pubkey) -> TokenAccountState {
    get_spl_account(svm, &key.to_bytes().into()).unwrap()
}

fn holder_pda(bond: &Pubkey, wallet: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[HOLDER_SEED, bond.as_ref(), wallet.as_ref()],
        &bond_core::ID,
    )
    .0
}

fn snapshot_pda(bond: &Pubkey, epoch: u32) -> Pubkey {
    Pubkey::find_program_address(
        &[SNAPSHOT_SEED, bond.as_ref(), &epoch.to_le_bytes()],
        &bond_core::ID,
    )
    .0
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

// ---------------------------------------------------------------- fixture

struct Fixture {
    svm: LiteSVM,
    issuer: Keypair,
    payment_mint: Pubkey,
    bond: Pubkey,
    bond_mint: Pubkey,
}

struct Holder {
    wallet: Keypair,
    token_account: Pubkey,
    record: Pubkey,
}

/// A fresh VM with both programs loaded, a test payment token (SIMULATED
/// KZT/USDC rail) and one issued bond: face 1,000, 10%, semi-annual.
fn fixture() -> Fixture {
    let (mut svm, issuer) = setup();
    let payment_mint = CreateMint::new(&mut svm, &issuer)
        .decimals(6)
        .send()
        .unwrap();
    let payment_mint = Pubkey::new_from_array(payment_mint.to_bytes());
    let (ix, bond, bond_mint) = issue_bond_ix(&issuer, payment_mint, 1, 1_000);
    send_ok(&mut svm, &[ix], &issuer, &[]);
    Fixture {
        svm,
        issuer,
        payment_mint,
        bond,
        bond_mint,
    }
}

/// Creates an investor wallet + bond token account and registers it in the
/// holder registry with the given jurisdiction flag (SIMULATED KYC data).
fn add_holder(f: &mut Fixture, jurisdiction: Jurisdiction) -> Holder {
    let wallet = Keypair::new();
    let token_account = CreateAssociatedTokenAccount::new(
        &mut f.svm,
        &f.issuer,
        &f.bond_mint.to_bytes().into(),
    )
    .owner(&wallet.pubkey())
    .send()
    .unwrap();
    let token_account = Pubkey::new_from_array(token_account.to_bytes());
    let record = holder_pda(&f.bond, &pk(&wallet));

    let ix = Instruction {
        program_id: bond_core::ID,
        accounts: bond_core::accounts::RegisterHolder {
            issuer: pk(&f.issuer),
            bond: f.bond,
            bond_mint: f.bond_mint,
            wallet: pk(&wallet),
            holder_token_account: token_account,
            holder_record: record,
            system_program: anchor_lang::system_program::ID,
        }
        .to_account_metas(None),
        data: bond_core::instruction::RegisterHolder { jurisdiction }.data(),
    };
    send_ok(&mut f.svm, &[ix], &f.issuer, &[]);
    Holder {
        wallet,
        token_account,
        record,
    }
}

fn mint_bonds_ix(f: &Fixture, h: &Holder, amount: u64) -> Instruction {
    Instruction {
        program_id: bond_core::ID,
        accounts: bond_core::accounts::MintBonds {
            issuer: pk(&f.issuer),
            bond: f.bond,
            bond_mint: f.bond_mint,
            holder_record: h.record,
            holder_token_account: h.token_account,
            token_program: anchor_spl::token::ID,
        }
        .to_account_metas(None),
        data: bond_core::instruction::MintBonds { amount }.data(),
    }
}

fn open_window_ix(f: &Fixture, signer: &Keypair, epoch: u32) -> Instruction {
    Instruction {
        program_id: bond_core::ID,
        accounts: bond_core::accounts::OpenRecordWindow {
            issuer: pk(signer),
            bond: f.bond,
            snapshot_record: snapshot_pda(&f.bond, epoch),
            system_program: anchor_lang::system_program::ID,
        }
        .to_account_metas(None),
        data: bond_core::instruction::OpenRecordWindow {}.data(),
    }
}

fn snapshot_ix(f: &Fixture, h: &Holder, epoch: u32) -> Instruction {
    Instruction {
        program_id: bond_core::ID,
        accounts: bond_core::accounts::SnapshotHolder {
            cranker: pk(&f.issuer),
            bond: f.bond,
            bond_mint: f.bond_mint,
            holder_record: h.record,
            holder_token_account: h.token_account,
            snapshot_record: snapshot_pda(&f.bond, epoch),
            token_program: anchor_spl::token::ID,
        }
        .to_account_metas(None),
        data: bond_core::instruction::SnapshotHolder {}.data(),
    }
}

fn close_window_ix(f: &Fixture, epoch: u32) -> Instruction {
    Instruction {
        program_id: bond_core::ID,
        accounts: bond_core::accounts::CloseRecordWindow {
            issuer: pk(&f.issuer),
            bond: f.bond,
            bond_mint: f.bond_mint,
            snapshot_record: snapshot_pda(&f.bond, epoch),
        }
        .to_account_metas(None),
        data: bond_core::instruction::CloseRecordWindow {}.data(),
    }
}

fn thaw_ix(f: &Fixture, h: &Holder) -> Instruction {
    Instruction {
        program_id: bond_core::ID,
        accounts: bond_core::accounts::ThawHolder {
            cranker: pk(&f.issuer),
            bond: f.bond,
            bond_mint: f.bond_mint,
            holder_record: h.record,
            holder_token_account: h.token_account,
            token_program: anchor_spl::token::ID,
        }
        .to_account_metas(None),
        data: bond_core::instruction::ThawHolder {}.data(),
    }
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

#[test]
fn register_mint_snapshot_close_and_thaw() {
    let mut f = fixture();
    let h = add_holder(&mut f, Jurisdiction::Domestic);

    // Holder registry record exists with the jurisdiction flag.
    let rec = read_holder(&f.svm, &h.record);
    assert_eq!(rec.bond, f.bond);
    assert_eq!(rec.wallet, pk(&h.wallet));
    assert_eq!(rec.token_account, h.token_account);
    assert!(rec.jurisdiction == Jurisdiction::Domestic);
    assert_eq!(rec.snapshot_epoch, 0);

    // Mint 10 bonds to the holder.
    let ix = mint_bonds_ix(&f, &h, 10);
    send_ok(&mut f.svm, &[ix], &f.issuer, &[]);
    assert_eq!(token_state(&f.svm, &h.token_account).amount, 10);
    assert_eq!(read_bond(&f.svm, &f.bond).minted_supply, 10);

    // Open the record window (epoch 1).
    let ix = open_window_ix(&f, &f.issuer, 1);
    send_ok(&mut f.svm, &[ix], &f.issuer, &[]);
    let bond = read_bond(&f.svm, &f.bond);
    assert!(bond.window_open);
    assert_eq!(bond.record_epoch, 1);

    // Snapshot: balance recorded AND token account frozen.
    let ix = snapshot_ix(&f, &h, 1);
    send_ok(&mut f.svm, &[ix], &f.issuer, &[]);
    let rec = read_holder(&f.svm, &h.record);
    assert_eq!(rec.snapshot_balance, 10);
    assert_eq!(rec.snapshot_epoch, 1);
    assert!(token_state(&f.svm, &h.token_account).is_frozen());
    assert_eq!(read_bond(&f.svm, &f.bond).snapshotted_supply, 10);

    // Close: supply fully accounted for.
    let ix = close_window_ix(&f, 1);
    send_ok(&mut f.svm, &[ix], &f.issuer, &[]);
    assert!(!read_bond(&f.svm, &f.bond).window_open);
    let snap = read_snapshot(&f.svm, &snapshot_pda(&f.bond, 1));
    assert!(snap.closed);
    assert_eq!(snap.epoch, 1);
    assert_eq!(snap.total_supply, 10);
    assert_eq!(snap.holder_count, 1);

    // Thaw after close: the holder can move bonds again.
    let ix = thaw_ix(&f, &h);
    send_ok(&mut f.svm, &[ix], &f.issuer, &[]);
    assert!(!token_state(&f.svm, &h.token_account).is_frozen());
}

#[test]
fn close_rejected_until_every_holder_snapshotted() {
    let mut f = fixture();
    let h1 = add_holder(&mut f, Jurisdiction::Domestic);
    let h2 = add_holder(&mut f, Jurisdiction::Foreign);
    assert!(read_holder(&f.svm, &h2.record).jurisdiction == Jurisdiction::Foreign);

    let ix = mint_bonds_ix(&f, &h1, 10);
    send_ok(&mut f.svm, &[ix], &f.issuer, &[]);
    let ix = mint_bonds_ix(&f, &h2, 5);
    send_ok(&mut f.svm, &[ix], &f.issuer, &[]);

    let ix = open_window_ix(&f, &f.issuer, 1);
    send_ok(&mut f.svm, &[ix], &f.issuer, &[]);

    // Snapshot only holder 1.
    let ix = snapshot_ix(&f, &h1, 1);
    send_ok(&mut f.svm, &[ix], &f.issuer, &[]);

    // Closing now must fail: 10 of 15 bonds accounted for.
    let ix = close_window_ix(&f, 1);
    let err = send(&mut f.svm, &[ix], &f.issuer, &[]).expect_err("incomplete close must fail");
    assert!(
        err.contains("SnapshotIncomplete"),
        "expected SnapshotIncomplete, got:\n{err}"
    );

    // Snapshotting the same holder twice must fail.
    let ix = snapshot_ix(&f, &h1, 1);
    let err = send(&mut f.svm, &[ix], &f.issuer, &[]).expect_err("double snapshot must fail");
    assert!(
        err.contains("AlreadySnapshotted"),
        "expected AlreadySnapshotted, got:\n{err}"
    );

    // Snapshot holder 2, then close succeeds.
    let ix = snapshot_ix(&f, &h2, 1);
    send_ok(&mut f.svm, &[ix], &f.issuer, &[]);
    let ix = close_window_ix(&f, 1);
    send_ok(&mut f.svm, &[ix], &f.issuer, &[]);

    let snap = read_snapshot(&f.svm, &snapshot_pda(&f.bond, 1));
    assert!(snap.closed);
    assert_eq!(snap.total_supply, 15);
    assert_eq!(snap.holder_count, 2);
}

#[test]
fn minting_is_blocked_while_window_is_open() {
    let mut f = fixture();
    let h = add_holder(&mut f, Jurisdiction::Domestic);
    let ix = mint_bonds_ix(&f, &h, 10);
    send_ok(&mut f.svm, &[ix], &f.issuer, &[]);

    let ix = open_window_ix(&f, &f.issuer, 1);
    send_ok(&mut f.svm, &[ix], &f.issuer, &[]);

    let ix = mint_bonds_ix(&f, &h, 1);
    let err = send(&mut f.svm, &[ix], &f.issuer, &[]).expect_err("mint during window must fail");
    assert!(
        err.contains("WindowOpen"),
        "expected WindowOpen, got:\n{err}"
    );
}

#[test]
fn non_issuer_cannot_open_window() {
    let mut f = fixture();
    let attacker = Keypair::new();
    f.svm.airdrop(&attacker.pubkey(), 10_000_000_000).unwrap();

    let ix = open_window_ix(&f, &attacker, 1);
    let err = send(&mut f.svm, &[ix], &attacker, &[]).expect_err("non-issuer must be rejected");
    assert!(
        err.contains("Unauthorized"),
        "expected Unauthorized, got:\n{err}"
    );
}

// ======================================================= B3: coupon flow

use corporate_actions::state::{
    ActionKind, ActionStatus, BondLedger, CorporateAction, Entitlement, ACTION_SEED,
    ENTITLEMENT_SEED, LEDGER_SEED, VAULT_SEED,
};
use litesvm_token::MintTo as PaymentMintTo;

fn ledger_pda(bond: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[LEDGER_SEED, bond.as_ref()], &corporate_actions::ID).0
}

fn action_pda(bond: &Pubkey, epoch: u32) -> Pubkey {
    Pubkey::find_program_address(
        &[ACTION_SEED, bond.as_ref(), &epoch.to_le_bytes()],
        &corporate_actions::ID,
    )
    .0
}

fn vault_pda(action: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(&[VAULT_SEED, action.as_ref()], &corporate_actions::ID).0
}

fn entitlement_pda(action: &Pubkey, wallet: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[ENTITLEMENT_SEED, action.as_ref(), wallet.as_ref()],
        &corporate_actions::ID,
    )
    .0
}

fn read_ledger(svm: &LiteSVM, key: &Pubkey) -> BondLedger {
    BondLedger::try_deserialize(&mut account_data(svm, key).as_slice()).unwrap()
}

fn read_action(svm: &LiteSVM, key: &Pubkey) -> CorporateAction {
    CorporateAction::try_deserialize(&mut account_data(svm, key).as_slice()).unwrap()
}

fn read_entitlement(svm: &LiteSVM, key: &Pubkey) -> Entitlement {
    Entitlement::try_deserialize(&mut account_data(svm, key).as_slice()).unwrap()
}

#[allow(dead_code)]
struct CouponEnv {
    issuer_payment: Pubkey,
    tax_collector: Pubkey,
    ledger: Pubkey,
}

struct CouponScenario {
    holder: Holder,
    env: CouponEnv,
    holder_payment: Pubkey,
    action: Pubkey,
}

/// Funds the issuer with test payment tokens (SIMULATED KZT/USDC rail), creates a
/// SIMULATED tax-collector account, and initializes the per-bond coupon ledger.
fn coupon_env(f: &mut Fixture, total_coupons: u16) -> CouponEnv {
    let issuer_ata = CreateAssociatedTokenAccount::new(
        &mut f.svm,
        &f.issuer,
        &f.payment_mint.to_bytes().into(),
    )
    .send()
    .unwrap();
    PaymentMintTo::new(
        &mut f.svm,
        &f.issuer,
        &f.payment_mint.to_bytes().into(),
        &issuer_ata,
        10_000_000,
    )
    .send()
    .unwrap();
    let issuer_payment = Pubkey::new_from_array(issuer_ata.to_bytes());

    let tax_wallet = Keypair::new();
    let tax_ata = CreateAssociatedTokenAccount::new(
        &mut f.svm,
        &f.issuer,
        &f.payment_mint.to_bytes().into(),
    )
    .owner(&tax_wallet.pubkey())
    .send()
    .unwrap();
    let tax_collector = Pubkey::new_from_array(tax_ata.to_bytes());

    let ledger = ledger_pda(&f.bond);
    let ix = Instruction {
        program_id: corporate_actions::ID,
        accounts: corporate_actions::accounts::InitializeLedger {
            issuer: pk(&f.issuer),
            bond: f.bond,
            payment_mint: f.payment_mint,
            tax_collector,
            ledger,
            system_program: anchor_lang::system_program::ID,
        }
        .to_account_metas(None),
        data: corporate_actions::instruction::InitializeLedger { total_coupons }.data(),
    };
    send_ok(&mut f.svm, &[ix], &f.issuer, &[]);

    CouponEnv {
        issuer_payment,
        tax_collector,
        ledger,
    }
}

fn payment_account_for(f: &mut Fixture, wallet: &Keypair) -> Pubkey {
    let acct = CreateAssociatedTokenAccount::new(
        &mut f.svm,
        &f.issuer,
        &f.payment_mint.to_bytes().into(),
    )
    .owner(&wallet.pubkey())
    .send()
    .unwrap();
    Pubkey::new_from_array(acct.to_bytes())
}

/// Runs a complete record date: open the window, snapshot every holder, close it.
fn record_date(f: &mut Fixture, epoch: u32, holders: &[&Holder]) {
    let ix = open_window_ix(f, &f.issuer, epoch);
    send_ok(&mut f.svm, &[ix], &f.issuer, &[]);
    for h in holders {
        let ix = snapshot_ix(f, h, epoch);
        send_ok(&mut f.svm, &[ix], &f.issuer, &[]);
    }
    let ix = close_window_ix(f, epoch);
    send_ok(&mut f.svm, &[ix], &f.issuer, &[]);
}

fn create_coupon_action_ix(f: &Fixture, env: &CouponEnv, epoch: u32) -> Instruction {
    let action = action_pda(&f.bond, epoch);
    Instruction {
        program_id: corporate_actions::ID,
        accounts: corporate_actions::accounts::CreateCouponAction {
            issuer: pk(&f.issuer),
            bond: f.bond,
            ledger: env.ledger,
            snapshot_record: snapshot_pda(&f.bond, epoch),
            payment_mint: f.payment_mint,
            action,
            vault: vault_pda(&action),
            token_program: anchor_spl::token::ID,
            system_program: anchor_lang::system_program::ID,
        }
        .to_account_metas(None),
        data: corporate_actions::instruction::CreateCouponAction { epoch }.data(),
    }
}

fn calculate_ix(f: &Fixture, h: &Holder, epoch: u32) -> Instruction {
    let action = action_pda(&f.bond, epoch);
    Instruction {
        program_id: corporate_actions::ID,
        accounts: corporate_actions::accounts::CalculateEntitlement {
            cranker: pk(&f.issuer),
            bond: f.bond,
            action,
            holder_record: h.record,
            entitlement: entitlement_pda(&action, &pk(&h.wallet)),
            system_program: anchor_lang::system_program::ID,
        }
        .to_account_metas(None),
        data: corporate_actions::instruction::CalculateEntitlement {}.data(),
    }
}

fn fund_ix(f: &Fixture, env: &CouponEnv, epoch: u32) -> Instruction {
    let action = action_pda(&f.bond, epoch);
    Instruction {
        program_id: corporate_actions::ID,
        accounts: corporate_actions::accounts::FundAction {
            issuer: pk(&f.issuer),
            bond: f.bond,
            ledger: env.ledger,
            action,
            payment_mint: f.payment_mint,
            issuer_payment_account: env.issuer_payment,
            vault: vault_pda(&action),
            token_program: anchor_spl::token::ID,
        }
        .to_account_metas(None),
        data: corporate_actions::instruction::FundAction {}.data(),
    }
}

fn claim_ix(f: &Fixture, h: &Holder, holder_payment: &Pubkey, epoch: u32) -> Instruction {
    let action = action_pda(&f.bond, epoch);
    Instruction {
        program_id: corporate_actions::ID,
        accounts: corporate_actions::accounts::ClaimCoupon {
            holder: pk(&h.wallet),
            bond: f.bond,
            action,
            entitlement: entitlement_pda(&action, &pk(&h.wallet)),
            payment_mint: f.payment_mint,
            vault: vault_pda(&action),
            holder_payment_account: *holder_payment,
            token_program: anchor_spl::token::ID,
        }
        .to_account_metas(None),
        data: corporate_actions::instruction::ClaimCoupon {}.data(),
    }
}

/// One holder with `bonds` bonds, a full record date (epoch 1), a coupon action,
/// and a calculated entitlement. Funding and claiming are left to each test.
fn prepare_coupon(f: &mut Fixture, jurisdiction: Jurisdiction, bonds: u64) -> CouponScenario {
    let holder = add_holder(f, jurisdiction);
    let ix = mint_bonds_ix(f, &holder, bonds);
    send_ok(&mut f.svm, &[ix], &f.issuer, &[]);

    let env = coupon_env(f, 3);
    let holder_payment = payment_account_for(f, &holder.wallet);

    record_date(f, 1, &[&holder]);

    let ix = create_coupon_action_ix(f, &env, 1);
    send_ok(&mut f.svm, &[ix], &f.issuer, &[]);
    let ix = calculate_ix(f, &holder, 1);
    send_ok(&mut f.svm, &[ix], &f.issuer, &[]);

    CouponScenario {
        holder,
        env,
        holder_payment,
        action: action_pda(&f.bond, 1),
    }
}

#[test]
fn coupon_flow_pays_500() {
    let mut f = fixture();
    // The spec example: 10 bonds x face 1,000 x 10% / 2 periods = 500 per coupon.
    let s = prepare_coupon(&mut f, Jurisdiction::Domestic, 10);

    // Action record and the visible, separate calculation step.
    let action = read_action(&f.svm, &s.action);
    assert!(action.kind == ActionKind::Coupon);
    assert!(action.status == ActionStatus::Created);
    assert_eq!(action.epoch, 1);
    assert_eq!(action.action_index, 1);
    assert_eq!(action.snapshot_holder_count, 1);
    assert_eq!(action.entitlement_count, 1);
    assert_eq!(action.total_gross, 500);
    assert_eq!(action.total_withheld, 0);
    assert_eq!(action.total_net, 500);

    let ent = read_entitlement(&f.svm, &entitlement_pda(&s.action, &pk(&s.holder.wallet)));
    assert_eq!(ent.quantity, 10);
    assert_eq!(ent.principal, 0);
    assert_eq!(ent.coupon_gross, 500);
    assert_eq!(ent.gross, 500);
    assert_eq!(ent.withholding_bps, 0);
    assert_eq!(ent.withheld, 0);
    assert_eq!(ent.net, 500);
    assert!(!ent.claimed);

    // Fund: exactly total_gross moves from the issuer into the vault.
    let issuer_before = token_state(&f.svm, &s.env.issuer_payment).amount;
    let ix = fund_ix(&f, &s.env, 1);
    send_ok(&mut f.svm, &[ix], &f.issuer, &[]);
    let action = read_action(&f.svm, &s.action);
    assert!(action.status == ActionStatus::Funded);
    assert_eq!(action.funded_amount, 500);
    assert_eq!(token_state(&f.svm, &vault_pda(&s.action)).amount, 500);
    assert_eq!(
        token_state(&f.svm, &s.env.issuer_payment).amount,
        issuer_before - 500
    );
    let ledger = read_ledger(&f.svm, &s.env.ledger);
    assert_eq!(ledger.coupons_created, 1);
    assert_eq!(ledger.coupons_paid, 1);

    // Claim: the holder signs and receives exactly 500.
    let ix = claim_ix(&f, &s.holder, &s.holder_payment, 1);
    send_ok(&mut f.svm, &[ix], &f.issuer, &[&s.holder.wallet]);
    assert_eq!(token_state(&f.svm, &s.holder_payment).amount, 500);
    assert_eq!(token_state(&f.svm, &vault_pda(&s.action)).amount, 0);
    let ent = read_entitlement(&f.svm, &entitlement_pda(&s.action, &pk(&s.holder.wallet)));
    assert!(ent.claimed);
    let action = read_action(&f.svm, &s.action);
    assert_eq!(action.claimed_count, 1);
    assert_eq!(action.total_claimed_net, 500);
}

#[test]
fn coupon_claim_before_funding_rejected() {
    let mut f = fixture();
    let s = prepare_coupon(&mut f, Jurisdiction::Domestic, 10);

    let ix = claim_ix(&f, &s.holder, &s.holder_payment, 1);
    let err = send(&mut f.svm, &[ix], &f.issuer, &[&s.holder.wallet])
        .expect_err("claim before funding must fail");
    assert!(err.contains("NotFunded"), "expected NotFunded, got:\n{err}");
}

#[test]
fn coupon_double_claim_rejected() {
    let mut f = fixture();
    let s = prepare_coupon(&mut f, Jurisdiction::Domestic, 10);

    let ix = fund_ix(&f, &s.env, 1);
    send_ok(&mut f.svm, &[ix], &f.issuer, &[]);
    let ix = claim_ix(&f, &s.holder, &s.holder_payment, 1);
    send_ok(&mut f.svm, &[ix], &f.issuer, &[&s.holder.wallet]);

    let ix = claim_ix(&f, &s.holder, &s.holder_payment, 1);
    let err = send(&mut f.svm, &[ix], &f.issuer, &[&s.holder.wallet])
        .expect_err("second claim must fail");
    assert!(
        err.contains("AlreadyClaimed"),
        "expected AlreadyClaimed, got:\n{err}"
    );
    // The holder was paid exactly once.
    assert_eq!(token_state(&f.svm, &s.holder_payment).amount, 500);
}

// ================================================ B4: edge cases and sweep

/// Like issue_bond_ix but with explicit withholding rates (bps) per jurisdiction.
fn issue_bond_with_ix(
    issuer: &Keypair,
    payment_mint: Pubkey,
    bond_id: u64,
    face_value: u64,
    withholding_bps_domestic: u16,
    withholding_bps_foreign: u16,
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
        coupon_rate_bps: 1_000,
        coupons_per_year: 2,
        maturity_ts: 2_000_000_000,
        max_supply: 1_000,
        withholding_bps_domestic,
        withholding_bps_foreign,
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

fn fixture_with_withholding(domestic_bps: u16, foreign_bps: u16) -> Fixture {
    let (mut svm, issuer) = setup();
    let payment_mint = CreateMint::new(&mut svm, &issuer)
        .decimals(6)
        .send()
        .unwrap();
    let payment_mint = Pubkey::new_from_array(payment_mint.to_bytes());
    let (ix, bond, bond_mint) =
        issue_bond_with_ix(&issuer, payment_mint, 1, 1_000, domestic_bps, foreign_bps);
    send_ok(&mut svm, &[ix], &issuer, &[]);
    Fixture {
        svm,
        issuer,
        payment_mint,
        bond,
        bond_mint,
    }
}

/// Replaces the first account (the issuer signer) of an instruction.
fn with_signer(mut ix: Instruction, signer: &Keypair) -> Instruction {
    ix.accounts[0].pubkey = pk(signer);
    ix
}

fn sweep_ix(f: &Fixture, env: &CouponEnv, epoch: u32) -> Instruction {
    let action = action_pda(&f.bond, epoch);
    Instruction {
        program_id: corporate_actions::ID,
        accounts: corporate_actions::accounts::SweepWithholding {
            issuer: pk(&f.issuer),
            bond: f.bond,
            action,
            payment_mint: f.payment_mint,
            vault: vault_pda(&action),
            tax_collector: env.tax_collector,
            token_program: anchor_spl::token::ID,
        }
        .to_account_metas(None),
        data: corporate_actions::instruction::SweepWithholding {}.data(),
    }
}

/// Registers holders (jurisdiction, bonds), mints, runs record date 1 and creates
/// the coupon action for epoch 1. Calculation is left to the test.
/// Returns (holders, their payment accounts, coupon env).
fn prepare_multi(
    f: &mut Fixture,
    specs: &[(Jurisdiction, u64)],
    total_coupons: u16,
) -> (Vec<Holder>, Vec<Pubkey>, CouponEnv) {
    let mut holders = Vec::new();
    for (j, bonds) in specs {
        let h = add_holder(f, *j);
        if *bonds > 0 {
            let ix = mint_bonds_ix(f, &h, *bonds);
            send_ok(&mut f.svm, &[ix], &f.issuer, &[]);
        }
        holders.push(h);
    }
    let env = coupon_env(f, total_coupons);
    let pays: Vec<Pubkey> = holders
        .iter()
        .map(|h| payment_account_for(f, &h.wallet))
        .collect();

    let refs: Vec<&Holder> = holders.iter().collect();
    record_date(f, 1, &refs);

    let ix = create_coupon_action_ix(f, &env, 1);
    send_ok(&mut f.svm, &[ix], &f.issuer, &[]);
    (holders, pays, env)
}

#[test]
fn withholding_split_and_sweep() {
    // Foreign holders: 15% withholding on the coupon. Domestic: 0%.
    let mut f = fixture_with_withholding(0, 1_500);
    let (holders, pays, env) = prepare_multi(
        &mut f,
        &[(Jurisdiction::Domestic, 10), (Jurisdiction::Foreign, 10)],
        3,
    );
    for h in &holders {
        let ix = calculate_ix(&f, h, 1);
        send_ok(&mut f.svm, &[ix], &f.issuer, &[]);
    }
    let action = action_pda(&f.bond, 1);

    let dom = read_entitlement(&f.svm, &entitlement_pda(&action, &pk(&holders[0].wallet)));
    assert_eq!((dom.gross, dom.withheld, dom.net), (500, 0, 500));

    let foreign = read_entitlement(&f.svm, &entitlement_pda(&action, &pk(&holders[1].wallet)));
    assert_eq!(foreign.withholding_bps, 1_500);
    assert_eq!((foreign.gross, foreign.withheld, foreign.net), (500, 75, 425));
    assert_eq!(foreign.gross, foreign.withheld + foreign.net);

    let a = read_action(&f.svm, &action);
    assert_eq!(a.entitlement_count, 2);
    assert_eq!(a.total_gross, 1_000);
    assert_eq!(a.total_withheld, 75);
    assert_eq!(a.total_net, 925);

    // Funding covers gross (net + withheld).
    let ix = fund_ix(&f, &env, 1);
    send_ok(&mut f.svm, &[ix], &f.issuer, &[]);
    assert_eq!(token_state(&f.svm, &vault_pda(&action)).amount, 1_000);

    // Sweep sends exactly the withheld amount to the (simulated) tax collector.
    let ix = sweep_ix(&f, &env, 1);
    send_ok(&mut f.svm, &[ix], &f.issuer, &[]);
    assert_eq!(token_state(&f.svm, &env.tax_collector).amount, 75);
    assert_eq!(token_state(&f.svm, &vault_pda(&action)).amount, 925);
    assert!(read_action(&f.svm, &action).withheld_swept);

    // A second sweep is refused.
    let ix = sweep_ix(&f, &env, 1);
    let err = send(&mut f.svm, &[ix], &f.issuer, &[]).expect_err("second sweep must fail");
    assert!(
        err.contains("AlreadySwept"),
        "expected AlreadySwept, got:\n{err}"
    );

    // Both holders can still claim their net amounts; the vault ends empty.
    let ix = claim_ix(&f, &holders[0], &pays[0], 1);
    send_ok(&mut f.svm, &[ix], &f.issuer, &[&holders[0].wallet]);
    let ix = claim_ix(&f, &holders[1], &pays[1], 1);
    send_ok(&mut f.svm, &[ix], &f.issuer, &[&holders[1].wallet]);
    assert_eq!(token_state(&f.svm, &pays[0]).amount, 500);
    assert_eq!(token_state(&f.svm, &pays[1]).amount, 425);
    assert_eq!(token_state(&f.svm, &vault_pda(&action)).amount, 0);
}

#[test]
fn sweep_rejected_before_funding_and_when_nothing_withheld() {
    let mut f = fixture(); // default withholding 0%
    let (holders, _pays, env) =
        prepare_multi(&mut f, &[(Jurisdiction::Domestic, 10)], 3);
    let ix = calculate_ix(&f, &holders[0], 1);
    send_ok(&mut f.svm, &[ix], &f.issuer, &[]);

    let ix = sweep_ix(&f, &env, 1);
    let err = send(&mut f.svm, &[ix], &f.issuer, &[]).expect_err("sweep before funding must fail");
    assert!(err.contains("NotFunded"), "expected NotFunded, got:\n{err}");

    let ix = fund_ix(&f, &env, 1);
    send_ok(&mut f.svm, &[ix], &f.issuer, &[]);

    let ix = sweep_ix(&f, &env, 1);
    let err = send(&mut f.svm, &[ix], &f.issuer, &[]).expect_err("nothing to sweep must fail");
    assert!(
        err.contains("NothingToSweep"),
        "expected NothingToSweep, got:\n{err}"
    );
}

#[test]
fn zero_balance_holder_gets_zero_entitlement() {
    let mut f = fixture();
    let (holders, pays, env) = prepare_multi(
        &mut f,
        &[(Jurisdiction::Domestic, 10), (Jurisdiction::Domestic, 0)],
        3,
    );
    for h in &holders {
        let ix = calculate_ix(&f, h, 1);
        send_ok(&mut f.svm, &[ix], &f.issuer, &[]);
    }
    let action = action_pda(&f.bond, 1);

    let zero = read_entitlement(&f.svm, &entitlement_pda(&action, &pk(&holders[1].wallet)));
    assert_eq!((zero.quantity, zero.gross, zero.net), (0, 0, 0));

    let a = read_action(&f.svm, &action);
    assert_eq!(a.snapshot_holder_count, 2);
    assert_eq!(a.entitlement_count, 2);
    assert_eq!(a.total_gross, 500);

    let ix = fund_ix(&f, &env, 1);
    send_ok(&mut f.svm, &[ix], &f.issuer, &[]);

    // The zero-balance holder can "claim" (nothing moves); the other gets 500.
    let ix = claim_ix(&f, &holders[1], &pays[1], 1);
    send_ok(&mut f.svm, &[ix], &f.issuer, &[&holders[1].wallet]);
    assert_eq!(token_state(&f.svm, &pays[1]).amount, 0);
    assert!(read_entitlement(&f.svm, &entitlement_pda(&action, &pk(&holders[1].wallet))).claimed);

    let ix = claim_ix(&f, &holders[0], &pays[0], 1);
    send_ok(&mut f.svm, &[ix], &f.issuer, &[&holders[0].wallet]);
    assert_eq!(token_state(&f.svm, &pays[0]).amount, 500);
    assert_eq!(token_state(&f.svm, &vault_pda(&action)).amount, 0);
}

#[test]
fn fund_rejected_until_every_holder_calculated() {
    let mut f = fixture();
    let (holders, _pays, env) = prepare_multi(
        &mut f,
        &[(Jurisdiction::Domestic, 10), (Jurisdiction::Domestic, 5)],
        3,
    );

    // Calculate only the first holder.
    let ix = calculate_ix(&f, &holders[0], 1);
    send_ok(&mut f.svm, &[ix], &f.issuer, &[]);

    let ix = fund_ix(&f, &env, 1);
    let err = send(&mut f.svm, &[ix], &f.issuer, &[]).expect_err("incomplete calculation must fail");
    assert!(
        err.contains("CalculationIncomplete"),
        "expected CalculationIncomplete, got:\n{err}"
    );

    // Calculate the second holder: funding now succeeds with 500 + 250.
    let ix = calculate_ix(&f, &holders[1], 1);
    send_ok(&mut f.svm, &[ix], &f.issuer, &[]);
    let ix = fund_ix(&f, &env, 1);
    send_ok(&mut f.svm, &[ix], &f.issuer, &[]);
    let a = read_action(&f.svm, &action_pda(&f.bond, 1));
    assert!(a.status == ActionStatus::Funded);
    assert_eq!(a.funded_amount, 750);
}

#[test]
fn non_issuer_cannot_create_or_fund() {
    let mut f = fixture();
    let holder = add_holder(&mut f, Jurisdiction::Domestic);
    let ix = mint_bonds_ix(&f, &holder, 10);
    send_ok(&mut f.svm, &[ix], &f.issuer, &[]);
    let env = coupon_env(&mut f, 3);
    record_date(&mut f, 1, &[&holder]);

    let attacker = Keypair::new();
    f.svm.airdrop(&attacker.pubkey(), 10_000_000_000).unwrap();

    // Attacker tries to create the action.
    let ix = with_signer(create_coupon_action_ix(&f, &env, 1), &attacker);
    let err = send(&mut f.svm, &[ix], &attacker, &[]).expect_err("non-issuer create must fail");
    assert!(
        err.contains("Unauthorized"),
        "expected Unauthorized, got:\n{err}"
    );

    // The real issuer creates and calculates; the attacker then tries to fund.
    let ix = create_coupon_action_ix(&f, &env, 1);
    send_ok(&mut f.svm, &[ix], &f.issuer, &[]);
    let ix = calculate_ix(&f, &holder, 1);
    send_ok(&mut f.svm, &[ix], &f.issuer, &[]);

    let ix = with_signer(fund_ix(&f, &env, 1), &attacker);
    let err = send(&mut f.svm, &[ix], &attacker, &[]).expect_err("non-issuer fund must fail");
    assert!(
        err.contains("Unauthorized"),
        "expected Unauthorized, got:\n{err}"
    );
}

#[test]
fn ledger_cap_refuses_extra_coupon() {
    let mut f = fixture();
    let holder = add_holder(&mut f, Jurisdiction::Domestic);
    let ix = mint_bonds_ix(&f, &holder, 10);
    send_ok(&mut f.svm, &[ix], &f.issuer, &[]);

    // total_coupons = 2 includes the final coupon paid with redemption,
    // so only ONE regular coupon action may exist.
    let env = coupon_env(&mut f, 2);
    record_date(&mut f, 1, &[&holder]);
    let ix = create_coupon_action_ix(&f, &env, 1);
    send_ok(&mut f.svm, &[ix], &f.issuer, &[]);
    assert_eq!(read_ledger(&f.svm, &env.ledger).coupons_created, 1);

    // Second record date: the holder's account is still frozen from window 1,
    // which snapshot_holder must handle.
    record_date(&mut f, 2, &[&holder]);
    let ix = create_coupon_action_ix(&f, &env, 2);
    let err = send(&mut f.svm, &[ix], &f.issuer, &[]).expect_err("extra coupon must be refused");
    assert!(
        err.contains("AllCouponsCreated"),
        "expected AllCouponsCreated, got:\n{err}"
    );
}

// ===================================================== B5: redemption

use litesvm_token::spl_token::state::Mint as MintState;
use solana_clock::Clock;

/// Bonds in these tests mature at 2_000_000_000 (see issue_bond_ix).
const AFTER_MATURITY: i64 = 2_000_000_100;

fn warp_to_ts(svm: &mut LiteSVM, unix_ts: i64) {
    let mut clock = svm.get_sysvar::<Clock>();
    clock.unix_timestamp = unix_ts;
    svm.set_sysvar::<Clock>(&clock);
}

fn mint_supply(svm: &LiteSVM, key: &Pubkey) -> u64 {
    let m: MintState = get_spl_account(svm, &key.to_bytes().into()).unwrap();
    m.supply
}

fn create_redemption_ix(f: &Fixture, env: &CouponEnv, epoch: u32) -> Instruction {
    let action = action_pda(&f.bond, epoch);
    Instruction {
        program_id: corporate_actions::ID,
        accounts: corporate_actions::accounts::CreateRedemptionAction {
            issuer: pk(&f.issuer),
            bond: f.bond,
            ledger: env.ledger,
            snapshot_record: snapshot_pda(&f.bond, epoch),
            payment_mint: f.payment_mint,
            action,
            vault: vault_pda(&action),
            token_program: anchor_spl::token::ID,
            system_program: anchor_lang::system_program::ID,
        }
        .to_account_metas(None),
        data: corporate_actions::instruction::CreateRedemptionAction { epoch }.data(),
    }
}

fn claim_redemption_ix(
    f: &Fixture,
    h: &Holder,
    holder_payment: &Pubkey,
    epoch: u32,
) -> Instruction {
    let action = action_pda(&f.bond, epoch);
    Instruction {
        program_id: corporate_actions::ID,
        accounts: corporate_actions::accounts::ClaimRedemption {
            holder: pk(&h.wallet),
            bond: f.bond,
            holder_record: h.record,
            action,
            entitlement: entitlement_pda(&action, &pk(&h.wallet)),
            payment_mint: f.payment_mint,
            vault: vault_pda(&action),
            holder_payment_account: *holder_payment,
            bond_mint: f.bond_mint,
            holder_bond_account: h.token_account,
            token_program: anchor_spl::token::ID,
        }
        .to_account_metas(None),
        data: corporate_actions::instruction::ClaimRedemption {}.data(),
    }
}

fn mark_redeemed_ix(f: &Fixture) -> Instruction {
    Instruction {
        program_id: bond_core::ID,
        accounts: bond_core::accounts::MarkRedeemed {
            cranker: pk(&f.issuer),
            bond: f.bond,
            bond_mint: f.bond_mint,
        }
        .to_account_metas(None),
        data: bond_core::instruction::MarkRedeemed {}.data(),
    }
}

/// Registers holders and mints bonds, sets up the coupon ledger, warps the clock
/// past maturity, runs record date 1 (opened AFTER maturity) and creates the
/// redemption action for epoch 1. total_coupons = 1 means no regular coupon
/// precedes redemption. Calculation is left to the test.
fn prepare_redemption(
    f: &mut Fixture,
    specs: &[(Jurisdiction, u64)],
    total_coupons: u16,
) -> (Vec<Holder>, Vec<Pubkey>, CouponEnv) {
    let mut holders = Vec::new();
    for (j, bonds) in specs {
        let h = add_holder(f, *j);
        if *bonds > 0 {
            let ix = mint_bonds_ix(f, &h, *bonds);
            send_ok(&mut f.svm, &[ix], &f.issuer, &[]);
        }
        holders.push(h);
    }
    let env = coupon_env(f, total_coupons);
    let pays: Vec<Pubkey> = holders
        .iter()
        .map(|h| payment_account_for(f, &h.wallet))
        .collect();

    warp_to_ts(&mut f.svm, AFTER_MATURITY);
    let refs: Vec<&Holder> = holders.iter().collect();
    record_date(f, 1, &refs);

    let ix = create_redemption_ix(f, &env, 1);
    send_ok(&mut f.svm, &[ix], &f.issuer, &[]);
    (holders, pays, env)
}

#[test]
fn redemption_pays_principal_plus_final_coupon_burns_and_marks_redeemed() {
    let mut f = fixture();
    let (holders, pays, env) = prepare_redemption(
        &mut f,
        &[(Jurisdiction::Domestic, 10), (Jurisdiction::Domestic, 5)],
        1,
    );
    let action = action_pda(&f.bond, 1);
    for h in &holders {
        let ix = calculate_ix(&f, h, 1);
        send_ok(&mut f.svm, &[ix], &f.issuer, &[]);
    }

    // Spec example: 10 bonds -> 10,000 principal + 500 final coupon.
    let e1 = read_entitlement(&f.svm, &entitlement_pda(&action, &pk(&holders[0].wallet)));
    assert_eq!(
        (e1.quantity, e1.principal, e1.coupon_gross, e1.gross, e1.withheld, e1.net),
        (10, 10_000, 500, 10_500, 0, 10_500)
    );
    let a = read_action(&f.svm, &action);
    assert!(a.kind == ActionKind::Redemption);
    assert_eq!(a.action_index, 0);
    assert_eq!(a.total_gross, 15_750); // 10,500 + 5,250
    assert!(read_ledger(&f.svm, &env.ledger).redemption_created);

    let ix = fund_ix(&f, &env, 1);
    send_ok(&mut f.svm, &[ix], &f.issuer, &[]);
    assert_eq!(token_state(&f.svm, &vault_pda(&action)).amount, 15_750);

    // Claim WITHOUT thawing first: refused, and nothing moves.
    let ix = claim_redemption_ix(&f, &holders[0], &pays[0], 1);
    let err = send(&mut f.svm, &[ix], &f.issuer, &[&holders[0].wallet])
        .expect_err("claim on a frozen bond account must fail");
    assert!(
        err.contains("BondAccountFrozen"),
        "expected BondAccountFrozen, got:\n{err}"
    );
    assert_eq!(token_state(&f.svm, &holders[0].token_account).amount, 10);
    assert_eq!(token_state(&f.svm, &pays[0]).amount, 0);

    // Bundle thaw + claim in ONE transaction.
    let thaw = thaw_ix(&f, &holders[0]);
    let claim = claim_redemption_ix(&f, &holders[0], &pays[0], 1);
    send_ok(&mut f.svm, &[thaw, claim], &f.issuer, &[&holders[0].wallet]);
    assert_eq!(token_state(&f.svm, &pays[0]).amount, 10_500);
    assert_eq!(token_state(&f.svm, &holders[0].token_account).amount, 0);
    assert_eq!(mint_supply(&f.svm, &f.bond_mint), 5);
    assert_eq!(token_state(&f.svm, &vault_pda(&action)).amount, 5_250);

    // 5 bonds are still outstanding, so the bond cannot be marked redeemed yet.
    let ix = mark_redeemed_ix(&f);
    let err = send(&mut f.svm, &[ix], &f.issuer, &[]).expect_err("must refuse while supply > 0");
    assert!(
        err.contains("SupplyOutstanding"),
        "expected SupplyOutstanding, got:\n{err}"
    );

    // Double claim refused.
    let ix = claim_redemption_ix(&f, &holders[0], &pays[0], 1);
    let err = send(&mut f.svm, &[ix], &f.issuer, &[&holders[0].wallet])
        .expect_err("second claim must fail");
    assert!(
        err.contains("AlreadyClaimed"),
        "expected AlreadyClaimed, got:\n{err}"
    );

    // Second holder redeems: vault empties, supply reaches 0.
    let thaw = thaw_ix(&f, &holders[1]);
    let claim = claim_redemption_ix(&f, &holders[1], &pays[1], 1);
    send_ok(&mut f.svm, &[thaw, claim], &f.issuer, &[&holders[1].wallet]);
    assert_eq!(token_state(&f.svm, &pays[1]).amount, 5_250);
    assert_eq!(mint_supply(&f.svm, &f.bond_mint), 0);
    assert_eq!(token_state(&f.svm, &vault_pda(&action)).amount, 0);
    assert_eq!(read_action(&f.svm, &action).claimed_count, 2);

    // Now the bond can be marked Redeemed.
    let ix = mark_redeemed_ix(&f);
    send_ok(&mut f.svm, &[ix], &f.issuer, &[]);
    assert!(read_bond(&f.svm, &f.bond).status == BondStatus::Redeemed);
}

#[test]
fn redemption_refused_before_maturity() {
    let mut f = fixture();
    let h = add_holder(&mut f, Jurisdiction::Domestic);
    let ix = mint_bonds_ix(&f, &h, 10);
    send_ok(&mut f.svm, &[ix], &f.issuer, &[]);
    let env = coupon_env(&mut f, 1);
    record_date(&mut f, 1, &[&h]); // clock has not reached maturity

    let ix = create_redemption_ix(&f, &env, 1);
    let err = send(&mut f.svm, &[ix], &f.issuer, &[]).expect_err("must refuse before maturity");
    assert!(err.contains("NotMatured"), "expected NotMatured, got:\n{err}");
}

#[test]
fn redemption_refused_while_coupon_outstanding() {
    let mut f = fixture();
    let h = add_holder(&mut f, Jurisdiction::Domestic);
    let ix = mint_bonds_ix(&f, &h, 10);
    send_ok(&mut f.svm, &[ix], &f.issuer, &[]);
    // total_coupons = 2: one regular coupon must be paid before redemption.
    let env = coupon_env(&mut f, 2);
    warp_to_ts(&mut f.svm, AFTER_MATURITY);
    record_date(&mut f, 1, &[&h]);

    let ix = create_redemption_ix(&f, &env, 1);
    let err = send(&mut f.svm, &[ix], &f.issuer, &[]).expect_err("coupon still outstanding");
    assert!(
        err.contains("CouponsOutstanding"),
        "expected CouponsOutstanding, got:\n{err}"
    );
}

#[test]
fn redemption_withholding_applies_to_coupon_part_only() {
    // Foreign holders: 15% withholding. It must hit the 500 coupon, never the principal.
    let mut f = fixture_with_withholding(0, 1_500);
    let (holders, pays, env) =
        prepare_redemption(&mut f, &[(Jurisdiction::Foreign, 10)], 1);
    let action = action_pda(&f.bond, 1);
    let ix = calculate_ix(&f, &holders[0], 1);
    send_ok(&mut f.svm, &[ix], &f.issuer, &[]);

    let e = read_entitlement(&f.svm, &entitlement_pda(&action, &pk(&holders[0].wallet)));
    assert_eq!(e.principal, 10_000);
    assert_eq!(e.coupon_gross, 500);
    assert_eq!(e.gross, 10_500);
    assert_eq!(e.withheld, 75); // 15% of 500 only
    assert_eq!(e.net, 10_425);

    let ix = fund_ix(&f, &env, 1);
    send_ok(&mut f.svm, &[ix], &f.issuer, &[]);
    assert_eq!(token_state(&f.svm, &vault_pda(&action)).amount, 10_500);

    let ix = sweep_ix(&f, &env, 1);
    send_ok(&mut f.svm, &[ix], &f.issuer, &[]);
    assert_eq!(token_state(&f.svm, &env.tax_collector).amount, 75);

    let thaw = thaw_ix(&f, &holders[0]);
    let claim = claim_redemption_ix(&f, &holders[0], &pays[0], 1);
    send_ok(&mut f.svm, &[thaw, claim], &f.issuer, &[&holders[0].wallet]);
    assert_eq!(token_state(&f.svm, &pays[0]).amount, 10_425);
    assert_eq!(token_state(&f.svm, &vault_pda(&action)).amount, 0);
    assert_eq!(mint_supply(&f.svm, &f.bond_mint), 0);
}

#[test]
fn mark_redeemed_refused_before_maturity() {
    let mut f = fixture();
    let ix = mark_redeemed_ix(&f);
    let err = send(&mut f.svm, &[ix], &f.issuer, &[]).expect_err("must refuse before maturity");
    assert!(err.contains("NotMatured"), "expected NotMatured, got:\n{err}");
}

#[test]
fn full_lifecycle_coupon_then_redemption() {
    let mut f = fixture();
    let h = add_holder(&mut f, Jurisdiction::Domestic);
    let ix = mint_bonds_ix(&f, &h, 10);
    send_ok(&mut f.svm, &[ix], &f.issuer, &[]);
    // total_coupons = 2: one regular coupon, then the final coupon rides with redemption.
    let env = coupon_env(&mut f, 2);
    let pay = payment_account_for(&mut f, &h.wallet);

    // --- Coupon 1: record date 1, before maturity ---
    record_date(&mut f, 1, &[&h]);
    let ix = create_coupon_action_ix(&f, &env, 1);
    send_ok(&mut f.svm, &[ix], &f.issuer, &[]);
    let ix = calculate_ix(&f, &h, 1);
    send_ok(&mut f.svm, &[ix], &f.issuer, &[]);
    let ix = fund_ix(&f, &env, 1);
    send_ok(&mut f.svm, &[ix], &f.issuer, &[]);
    let ix = claim_ix(&f, &h, &pay, 1);
    send_ok(&mut f.svm, &[ix], &f.issuer, &[&h.wallet]);
    assert_eq!(token_state(&f.svm, &pay).amount, 500);
    assert_eq!(read_ledger(&f.svm, &env.ledger).coupons_paid, 1);

    // --- A record window opened BEFORE maturity cannot be used for redemption ---
    record_date(&mut f, 2, &[&h]);
    warp_to_ts(&mut f.svm, AFTER_MATURITY);
    let ix = create_redemption_ix(&f, &env, 2);
    let err = send(&mut f.svm, &[ix], &f.issuer, &[])
        .expect_err("pre-maturity snapshot must be refused");
    assert!(
        err.contains("SnapshotBeforeMaturity"),
        "expected SnapshotBeforeMaturity, got:\n{err}"
    );

    // --- Record date 3 after maturity: redemption ---
    record_date(&mut f, 3, &[&h]);
    let ix = create_redemption_ix(&f, &env, 3);
    send_ok(&mut f.svm, &[ix], &f.issuer, &[]);
    let ix = calculate_ix(&f, &h, 3);
    send_ok(&mut f.svm, &[ix], &f.issuer, &[]);
    let ix = fund_ix(&f, &env, 3);
    send_ok(&mut f.svm, &[ix], &f.issuer, &[]);

    let thaw = thaw_ix(&f, &h);
    let claim = claim_redemption_ix(&f, &h, &pay, 3);
    send_ok(&mut f.svm, &[thaw, claim], &f.issuer, &[&h.wallet]);

    // 500 coupon + 10,000 principal + 500 final coupon.
    assert_eq!(token_state(&f.svm, &pay).amount, 11_000);
    assert_eq!(token_state(&f.svm, &h.token_account).amount, 0);
    assert_eq!(mint_supply(&f.svm, &f.bond_mint), 0);

    let ix = mark_redeemed_ix(&f);
    send_ok(&mut f.svm, &[ix], &f.issuer, &[]);
    assert!(read_bond(&f.svm, &f.bond).status == BondStatus::Redeemed);
}

// ===================================================== B6: governance

use governance::state::{
    Proposal, ProposalStatus, VoteChoice, VoteRecord, PROPOSAL_SEED, VOTE_SEED,
};

const GOVERNANCE_SO: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../target/litesvm/governance.so"
);
/// Base clock for governance tests (maturity is 2_000_000_000, so bonds stay active).
const T0: i64 = 1_000_000_000;

fn gov_fixture() -> Fixture {
    let mut f = fixture();
    f.svm
        .add_program_from_file(governance::ID, GOVERNANCE_SO)
        .unwrap();
    f
}

/// Voters pay the rent of their own vote record, so they need a little SOL.
fn fund_wallet(f: &mut Fixture, h: &Holder) {
    f.svm.airdrop(&h.wallet.pubkey(), 1_000_000_000).unwrap();
}

fn proposal_pda(bond: &Pubkey, id: u32) -> Pubkey {
    Pubkey::find_program_address(
        &[PROPOSAL_SEED, bond.as_ref(), &id.to_le_bytes()],
        &governance::ID,
    )
    .0
}

fn vote_pda(proposal: &Pubkey, wallet: &Pubkey) -> Pubkey {
    Pubkey::find_program_address(
        &[VOTE_SEED, proposal.as_ref(), wallet.as_ref()],
        &governance::ID,
    )
    .0
}

fn read_proposal(svm: &LiteSVM, key: &Pubkey) -> Proposal {
    Proposal::try_deserialize(&mut account_data(svm, key).as_slice()).unwrap()
}

fn read_vote(svm: &LiteSVM, key: &Pubkey) -> VoteRecord {
    VoteRecord::try_deserialize(&mut account_data(svm, key).as_slice()).unwrap()
}

fn create_proposal_ix(
    f: &Fixture,
    id: u32,
    epoch: u32,
    start_ts: i64,
    end_ts: i64,
    quorum_bps: u16,
) -> Instruction {
    Instruction {
        program_id: governance::ID,
        accounts: governance::accounts::CreateProposal {
            issuer: pk(&f.issuer),
            bond: f.bond,
            snapshot_record: snapshot_pda(&f.bond, epoch),
            proposal: proposal_pda(&f.bond, id),
            system_program: anchor_lang::system_program::ID,
        }
        .to_account_metas(None),
        data: governance::instruction::CreateProposal {
            proposal_id: id,
            title: "Approve covenant waiver".to_string(),
            description: "One-time waiver of the leverage covenant.".to_string(),
            start_ts,
            end_ts,
            quorum_bps,
        }
        .data(),
    }
}

fn cast_vote_ix(f: &Fixture, h: &Holder, id: u32, choice: VoteChoice) -> Instruction {
    let proposal = proposal_pda(&f.bond, id);
    Instruction {
        program_id: governance::ID,
        accounts: governance::accounts::CastVote {
            voter: pk(&h.wallet),
            bond: f.bond,
            proposal,
            holder_record: h.record,
            vote_record: vote_pda(&proposal, &pk(&h.wallet)),
            system_program: anchor_lang::system_program::ID,
        }
        .to_account_metas(None),
        data: governance::instruction::CastVote { choice }.data(),
    }
}

fn finalize_ix(f: &Fixture, id: u32) -> Instruction {
    Instruction {
        program_id: governance::ID,
        accounts: governance::accounts::FinalizeProposal {
            cranker: pk(&f.issuer),
            bond: f.bond,
            proposal: proposal_pda(&f.bond, id),
        }
        .to_account_metas(None),
        data: governance::instruction::FinalizeProposal {}.data(),
    }
}

/// Four holders with 10, 5, 2 and 0 bonds (17 in circulation), record date 1,
/// clock set to T0. The zero-balance holder is registered and snapshotted.
fn prepare_governance() -> (Fixture, Vec<Holder>) {
    let mut f = gov_fixture();
    let mut holders = Vec::new();
    for bonds in [10u64, 5, 2, 0] {
        let h = add_holder(&mut f, Jurisdiction::Domestic);
        if bonds > 0 {
            let ix = mint_bonds_ix(&f, &h, bonds);
            send_ok(&mut f.svm, &[ix], &f.issuer, &[]);
        }
        fund_wallet(&mut f, &h);
        holders.push(h);
    }
    let refs: Vec<&Holder> = holders.iter().collect();
    record_date(&mut f, 1, &refs);
    warp_to_ts(&mut f.svm, T0);
    (f, holders)
}

#[test]
fn governance_weighted_vote_passes_and_result_is_final() {
    let (mut f, hs) = prepare_governance();

    let ix = create_proposal_ix(&f, 1, 1, T0, T0 + 3_600, 5_000); // 50% quorum
    send_ok(&mut f.svm, &[ix], &f.issuer, &[]);
    let proposal = proposal_pda(&f.bond, 1);
    let p = read_proposal(&f.svm, &proposal);
    assert_eq!(p.epoch, 1);
    assert_eq!(p.snapshot_total_supply, 17);
    assert!(p.status == ProposalStatus::Open);

    // Weights come from snapshot balances: 10 For, 5 Against, 2 Abstain.
    let ix = cast_vote_ix(&f, &hs[0], 1, VoteChoice::For);
    send_ok(&mut f.svm, &[ix], &hs[0].wallet, &[]);
    let ix = cast_vote_ix(&f, &hs[1], 1, VoteChoice::Against);
    send_ok(&mut f.svm, &[ix], &hs[1].wallet, &[]);
    let ix = cast_vote_ix(&f, &hs[2], 1, VoteChoice::Abstain);
    send_ok(&mut f.svm, &[ix], &hs[2].wallet, &[]);

    let p = read_proposal(&f.svm, &proposal);
    assert_eq!(
        (p.votes_for, p.votes_against, p.votes_abstain, p.voter_count),
        (10, 5, 2, 3)
    );
    let v = read_vote(&f.svm, &vote_pda(&proposal, &pk(&hs[0].wallet)));
    assert_eq!(v.weight, 10);
    assert!(v.choice == VoteChoice::For);

    // Too early to finalize.
    let ix = finalize_ix(&f, 1);
    let err = send(&mut f.svm, &[ix], &f.issuer, &[]).expect_err("must refuse while voting is open");
    assert!(
        err.contains("VotingStillOpen"),
        "expected VotingStillOpen, got:\n{err}"
    );

    // After the window: 17 of 17 turnout (>= 50%) and 10 > 5, so Passed.
    warp_to_ts(&mut f.svm, T0 + 3_601);
    let ix = finalize_ix(&f, 1);
    send_ok(&mut f.svm, &[ix], &f.issuer, &[]);
    let p = read_proposal(&f.svm, &proposal);
    assert!(p.status == ProposalStatus::Passed);
    assert!(p.finalized_ts >= T0 + 3_601);

    // The result is final.
    let ix = finalize_ix(&f, 1);
    let err = send(&mut f.svm, &[ix], &f.issuer, &[]).expect_err("second finalize must fail");
    assert!(
        err.contains("AlreadyFinalized"),
        "expected AlreadyFinalized, got:\n{err}"
    );
}

#[test]
fn governance_vote_rules() {
    let (mut f, hs) = prepare_governance();
    let ix = create_proposal_ix(&f, 1, 1, T0, T0 + 3_600, 0);
    send_ok(&mut f.svm, &[ix], &f.issuer, &[]);
    let ix = create_proposal_ix(&f, 2, 1, T0 + 1_000, T0 + 2_000, 0);
    send_ok(&mut f.svm, &[ix], &f.issuer, &[]);
    let proposal1 = proposal_pda(&f.bond, 1);

    // One vote per holder: the second attempt fails (vote record already exists).
    let ix = cast_vote_ix(&f, &hs[0], 1, VoteChoice::For);
    send_ok(&mut f.svm, &[ix], &hs[0].wallet, &[]);
    let ix = cast_vote_ix(&f, &hs[0], 1, VoteChoice::Against);
    let err = send(&mut f.svm, &[ix], &hs[0].wallet, &[]).expect_err("double vote must fail");
    assert!(
        err.contains("already in use"),
        "expected an already-in-use failure, got:\n{err}"
    );
    let p = read_proposal(&f.svm, &proposal1);
    assert_eq!((p.votes_for, p.votes_against), (10, 0));

    // Voting on proposal 2 has not started.
    let ix = cast_vote_ix(&f, &hs[1], 2, VoteChoice::For);
    let err = send(&mut f.svm, &[ix], &hs[1].wallet, &[]).expect_err("vote before start must fail");
    assert!(
        err.contains("VotingNotStarted"),
        "expected VotingNotStarted, got:\n{err}"
    );

    // A registered holder with zero bonds has no voting power.
    let ix = cast_vote_ix(&f, &hs[3], 1, VoteChoice::For);
    let err = send(&mut f.svm, &[ix], &hs[3].wallet, &[]).expect_err("zero balance must fail");
    assert!(
        err.contains("NoVotingPower"),
        "expected NoVotingPower, got:\n{err}"
    );

    // Nobody can borrow someone else's weight: the attacker signs with hs[1]'s record.
    let attacker = Keypair::new();
    f.svm.airdrop(&attacker.pubkey(), 10_000_000_000).unwrap();
    let mut ix = with_signer(cast_vote_ix(&f, &hs[1], 1, VoteChoice::For), &attacker);
    // accounts[4] is the vote record; derive it from the attacker so seeds are valid
    // and the holder-record ownership check is what refuses.
    ix.accounts[4].pubkey = vote_pda(&proposal1, &pk(&attacker));
    let err = send(&mut f.svm, &[ix], &attacker, &[]).expect_err("borrowed weight must fail");
    assert!(
        err.contains("Unauthorized"),
        "expected Unauthorized, got:\n{err}"
    );

    // After the end of proposal 1, voting is refused.
    warp_to_ts(&mut f.svm, T0 + 3_601);
    let ix = cast_vote_ix(&f, &hs[1], 1, VoteChoice::For);
    let err = send(&mut f.svm, &[ix], &hs[1].wallet, &[]).expect_err("vote after end must fail");
    assert!(
        err.contains("VotingEnded"),
        "expected VotingEnded, got:\n{err}"
    );
}

#[test]
fn governance_quorum_rejection_and_create_rules() {
    let (mut f, hs) = prepare_governance();

    // Create-time rules.
    let attacker = Keypair::new();
    f.svm.airdrop(&attacker.pubkey(), 10_000_000_000).unwrap();
    let ix = with_signer(create_proposal_ix(&f, 9, 1, T0, T0 + 100, 0), &attacker);
    let err = send(&mut f.svm, &[ix], &attacker, &[]).expect_err("non-issuer must be refused");
    assert!(
        err.contains("Unauthorized"),
        "expected Unauthorized, got:\n{err}"
    );
    let ix = create_proposal_ix(&f, 9, 1, T0 + 100, T0 + 100, 0);
    let err = send(&mut f.svm, &[ix], &f.issuer, &[]).expect_err("empty window must be refused");
    assert!(
        err.contains("InvalidWindow"),
        "expected InvalidWindow, got:\n{err}"
    );
    let ix = create_proposal_ix(&f, 9, 1, T0, T0 + 100, 10_001);
    let err = send(&mut f.svm, &[ix], &f.issuer, &[]).expect_err("quorum over 100% must be refused");
    assert!(
        err.contains("InvalidQuorum"),
        "expected InvalidQuorum, got:\n{err}"
    );

    // Proposal 1 needs 60% turnout; only the 10-bond holder votes: 10/17 = 58.8%.
    let ix = create_proposal_ix(&f, 1, 1, T0, T0 + 3_600, 6_000);
    send_ok(&mut f.svm, &[ix], &f.issuer, &[]);
    // Proposal 2 has no quorum; 10 Against beats 5 For.
    let ix = create_proposal_ix(&f, 2, 1, T0, T0 + 3_600, 0);
    send_ok(&mut f.svm, &[ix], &f.issuer, &[]);

    let ix = cast_vote_ix(&f, &hs[0], 1, VoteChoice::For);
    send_ok(&mut f.svm, &[ix], &hs[0].wallet, &[]);
    let ix = cast_vote_ix(&f, &hs[0], 2, VoteChoice::Against);
    send_ok(&mut f.svm, &[ix], &hs[0].wallet, &[]);
    let ix = cast_vote_ix(&f, &hs[1], 2, VoteChoice::For);
    send_ok(&mut f.svm, &[ix], &hs[1].wallet, &[]);

    warp_to_ts(&mut f.svm, T0 + 3_601);
    let ix = finalize_ix(&f, 1);
    send_ok(&mut f.svm, &[ix], &f.issuer, &[]);
    let ix = finalize_ix(&f, 2);
    send_ok(&mut f.svm, &[ix], &f.issuer, &[]);

    assert!(read_proposal(&f.svm, &proposal_pda(&f.bond, 1)).status == ProposalStatus::NoQuorum);
    assert!(read_proposal(&f.svm, &proposal_pda(&f.bond, 2)).status == ProposalStatus::Rejected);
}

#[test]
fn governance_vote_after_next_record_window_is_refused() {
    let (mut f, hs) = prepare_governance();
    let ix = create_proposal_ix(&f, 1, 1, T0, T0 + 3_600, 0);
    send_ok(&mut f.svm, &[ix], &f.issuer, &[]);
    let ix = cast_vote_ix(&f, &hs[0], 1, VoteChoice::For);
    send_ok(&mut f.svm, &[ix], &hs[0].wallet, &[]);

    // The next record date overwrites every holder record with epoch 2.
    let refs: Vec<&Holder> = hs.iter().collect();
    record_date(&mut f, 2, &refs);

    let ix = cast_vote_ix(&f, &hs[1], 1, VoteChoice::For);
    let err = send(&mut f.svm, &[ix], &hs[1].wallet, &[]).expect_err("late vote must be refused");
    assert!(
        err.contains("StaleSnapshot"),
        "expected StaleSnapshot, got:\n{err}"
    );
    // The vote cast before the new window is still counted.
    let p = read_proposal(&f.svm, &proposal_pda(&f.bond, 1));
    assert_eq!(p.votes_for, 10);
}
