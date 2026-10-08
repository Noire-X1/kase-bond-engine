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
