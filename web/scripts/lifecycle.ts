// @ts-nocheck
/* Devnet lifecycle: issue -> record date -> coupon -> governance -> maturity -> redemption.
   Run from web/:  npx tsx scripts/lifecycle.ts
   SIMULATED: the payment token (a test SPL token standing in for KZT) and the
   jurisdiction flags / withholding rate (demo parameters, not tax advice). */
import fs from "node:fs";
import os from "node:os";
import path from "node:path";
import {
  Connection, Keypair, LAMPORTS_PER_SOL, SystemProgram, SYSVAR_CLOCK_PUBKEY, Transaction,
} from "@solana/web3.js";
import { AnchorProvider, BN } from "@anchor-lang/core";
import {
  TOKEN_PROGRAM_ID, createMint, getAccount, getOrCreateAssociatedTokenAccount, mintTo,
} from "@solana/spl-token";
import {
  ACTIONS_ID, BOND_CORE_ID, GOVERNANCE_ID, explorerAddr, explorerTx, loadPrograms, pda,
} from "../lib/chain/sdk";

const RPC = process.env.RPC_URL ?? "https://api.devnet.solana.com";

// Every RPC call retries on rate limits / server errors.
const retryingFetch = async (input, init) => {
  for (let i = 0; ; i++) {
    try {
      const res = await fetch(input, init);
      if (res.status === 429 || res.status >= 500) throw new Error(`HTTP ${res.status}`);
      return res;
    } catch (e) {
      if (i >= 5) throw e;
      await new Promise((r) => setTimeout(r, 1000 * (i + 1)));
    }
  }
};
const connection = new Connection(RPC, { commitment: "confirmed", fetch: retryingFetch });

// Minimal wallet so we do not depend on a Wallet export from the client package.
class KeypairWallet {
  constructor(payer) { this.payer = payer; }
  get publicKey() { return this.payer.publicKey; }
  async signTransaction(tx) {
    if ("version" in tx) tx.sign([this.payer]);
    else tx.partialSign(this.payer);
    return tx;
  }
  async signAllTransactions(txs) {
    return Promise.all(txs.map((t) => this.signTransaction(t)));
  }
}

// ---- demo parameters -------------------------------------------------------
const DECIMALS = 2;                    // SIMULATED KZT with 2 decimals
const FACE = 100_000;                  // KZT 1,000.00 per bond
const RATE_BPS = 1_000;                // 10% per year
const PERIODS = 2;                     // semi-annual
const TOTAL_COUPONS = 2;               // 1 regular coupon + final coupon paid with redemption
const MATURITY_SECS = 300;
const FOREIGN_WITHHOLDING_BPS = 1_500; // demo parameter (simulated)
const PLAN = [
  { name: "Investor A (domestic)", jurisdiction: { domestic: {} }, bonds: 10, foreign: false, vote: { for: {} }, voteLabel: "For" },
  { name: "Investor B (foreign)", jurisdiction: { foreign: {} }, bonds: 5, foreign: true, vote: { against: {} }, voteLabel: "Against" },
  { name: "Investor C (domestic)", jurisdiction: { domestic: {} }, bonds: 20, foreign: false, vote: { for: {} }, voteLabel: "For" },
];

// ---- helpers ---------------------------------------------------------------
const sleep = (ms) => new Promise((r) => setTimeout(r, ms));
const log = (...a) => console.log(...a);
const steps = [];
const n = (x) => Number(x.toString());

/** Unix time as the chain sees it (Clock sysvar, unix_timestamp at byte offset 32). */
async function chainNow() {
  try {
    const info = await connection.getAccountInfo(SYSVAR_CLOCK_PUBKEY, "confirmed");
    if (info) return Number(info.data.readBigInt64LE(32));
  } catch {}
  return Math.floor(Date.now() / 1000);
}

async function waitUntil(ts, label) {
  for (;;) {
    const now = await chainNow();
    if (now >= ts + 4) return;
    log(`   waiting for ${label}: ~${Math.max(0, ts - now)}s left`);
    await sleep(8000);
  }
}

async function step(label, fn) {
  for (let attempt = 1; ; attempt++) {
    try {
      const sig = await fn();
      steps.push({ step: label, sig, url: explorerTx(sig), ts: Date.now() });
      log(`OK  ${label}\n    ${explorerTx(sig)}`);
      return sig;
    } catch (e) {
      const msg = String(e?.message ?? e);
      const transient = /blockhash|timed out|timeout|429|fetch failed|ECONN|socket/i.test(msg);
      if (transient && attempt < 4) {
        log(`    retrying "${label}" (${attempt}): ${msg.slice(0, 90)}`);
        await sleep(2500 * attempt);
        continue;
      }
      console.error(`FAIL ${label}\n`, e?.logs ?? "", msg);
      throw e;
    }
  }
}

function check(label, actual, expected) {
  const ok = Number(actual) === expected;
  log(`${ok ? "PASS" : "FAIL"} ${label}: ${actual} (expected ${expected})`);
  if (!ok) process.exitCode = 1;
}

const readIdl = (name) =>
  JSON.parse(fs.readFileSync(path.join(process.cwd(), "lib/idl", `${name}.json`), "utf8"));

async function main() {
  const issuer = Keypair.fromSecretKey(
    Uint8Array.from(JSON.parse(fs.readFileSync(path.join(os.homedir(), ".config/solana/id.json"), "utf8"))),
  );
  const provider = new AnchorProvider(connection, new KeypairWallet(issuer), { commitment: "confirmed" });
  const progs = loadPrograms(provider, {
    bondCore: readIdl("bond_core"), actions: readIdl("corporate_actions"), governance: readIdl("governance"),
  });
  const bc = progs.bondCore.methods;
  const ac = progs.actions.methods;
  const gv = progs.governance.methods;
  const tokenProgram = TOKEN_PROGRAM_ID;
  const systemProgram = SystemProgram.programId;

  const startBalance = (await connection.getBalance(issuer.publicKey)) / LAMPORTS_PER_SOL;
  log(`Issuer ${issuer.publicKey.toBase58()} on ${RPC}\nBalance ${startBalance} SOL\n`);
  if (startBalance < 1) throw new Error("Need at least 1 SOL on the issuer wallet.");

  // ---- wallets (throwaway devnet keys, gitignored) ---------------------------
  const wallets = PLAN.map(() => Keypair.generate());
  const taxAuthority = Keypair.generate(); // SIMULATED tax authority
  fs.writeFileSync(
    path.join(process.cwd(), "scripts/.demo-wallets.json"),
    JSON.stringify({ investors: wallets.map((w) => Array.from(w.secretKey)) }),
  );
  const fund = new Transaction();
  for (const w of wallets) {
    fund.add(SystemProgram.transfer({ fromPubkey: issuer.publicKey, toPubkey: w.publicKey, lamports: 0.02 * LAMPORTS_PER_SOL }));
  }
  await step("Fund investor wallets (rent for vote records)", () => provider.sendAndConfirm(fund));

  // ---- SIMULATED payment token -------------------------------------------------
  const paymentMint = await createMint(connection, issuer, issuer.publicKey, null, DECIMALS);
  const issuerPay = (await getOrCreateAssociatedTokenAccount(connection, issuer, paymentMint, issuer.publicKey)).address;
  await mintTo(connection, issuer, paymentMint, issuerPay, issuer, 10_000_000);
  const taxAta = (await getOrCreateAssociatedTokenAccount(connection, issuer, paymentMint, taxAuthority.publicKey)).address;
  log(`Payment mint (SIMULATED KZT) ${paymentMint.toBase58()}\n`);

  // ---- issue bond + ledger -------------------------------------------------------
  const idNum = Date.now();
  const bond = pda.bond(issuer.publicKey, idNum);
  const bondMint = pda.bondMint(bond);
  const ledger = pda.ledger(bond);
  const maturity = (await chainNow()) + MATURITY_SECS;

  await step("Issue bond", () =>
    bc.issueBond({
      bondId: new BN(idNum), faceValue: new BN(FACE), couponRateBps: RATE_BPS, couponsPerYear: PERIODS,
      maturityTs: new BN(maturity), maxSupply: new BN(1000),
      withholdingBpsDomestic: 0, withholdingBpsForeign: FOREIGN_WITHHOLDING_BPS,
    }).accountsPartial({ issuer: issuer.publicKey, bond, bondMint, paymentMint, tokenProgram, systemProgram }).rpc());

  await step("Initialize coupon ledger", () =>
    ac.initializeLedger(TOTAL_COUPONS).accountsPartial({
      issuer: issuer.publicKey, bond, paymentMint, taxCollector: taxAta, ledger, systemProgram,
    }).rpc());

  // ---- holders -----------------------------------------------------------------
  const holders = [];
  for (let i = 0; i < PLAN.length; i++) {
    const p = PLAN[i];
    const kp = wallets[i];
    const bondAta = (await getOrCreateAssociatedTokenAccount(connection, issuer, bondMint, kp.publicKey)).address;
    const payAta = (await getOrCreateAssociatedTokenAccount(connection, issuer, paymentMint, kp.publicKey)).address;
    const record = pda.holder(bond, kp.publicKey);
    await step(`Register ${p.name}`, () =>
      bc.registerHolder(p.jurisdiction).accountsPartial({
        issuer: issuer.publicKey, bond, bondMint, wallet: kp.publicKey,
        holderTokenAccount: bondAta, holderRecord: record, systemProgram,
      }).rpc());
    await step(`Mint ${p.bonds} bonds to ${p.name}`, () =>
      bc.mintBonds(new BN(p.bonds)).accountsPartial({
        issuer: issuer.publicKey, bond, bondMint, holderRecord: record, holderTokenAccount: bondAta, tokenProgram,
      }).rpc());
    holders.push({ ...p, kp, bondAta, payAta, record });
  }

  // ---- reusable flows --------------------------------------------------------------
  async function recordDate(epoch) {
    const snapshotRecord = pda.snapshot(bond, epoch);
    await step(`Open record window ${epoch}`, () =>
      bc.openRecordWindow().accountsPartial({ issuer: issuer.publicKey, bond, snapshotRecord, systemProgram }).rpc());
    for (const h of holders) {
      await step(`Snapshot ${h.name} (window ${epoch})`, () =>
        bc.snapshotHolder().accountsPartial({
          cranker: issuer.publicKey, bond, bondMint, holderRecord: h.record,
          holderTokenAccount: h.bondAta, snapshotRecord, tokenProgram,
        }).rpc());
    }
    await step(`Close record window ${epoch}`, () =>
      bc.closeRecordWindow().accountsPartial({ issuer: issuer.publicKey, bond, bondMint, snapshotRecord }).rpc());
  }

  async function payAction(kind, epoch) {
    const snapshotRecord = pda.snapshot(bond, epoch);
    const action = pda.action(bond, epoch);
    const vault = pda.vault(action);
    await step(`Create ${kind} action`, () =>
      (kind === "coupon" ? ac.createCouponAction(epoch) : ac.createRedemptionAction(epoch)).accountsPartial({
        issuer: issuer.publicKey, bond, ledger, snapshotRecord, paymentMint, action, vault, tokenProgram, systemProgram,
      }).rpc());
    for (const h of holders) {
      await step(`Calculate entitlement: ${h.name} (${kind})`, () =>
        ac.calculateEntitlement().accountsPartial({
          cranker: issuer.publicKey, bond, action, holderRecord: h.record,
          entitlement: pda.entitlement(action, h.kp.publicKey), systemProgram,
        }).rpc());
    }
    await step(`Fund ${kind} action`, () =>
      ac.fundAction().accountsPartial({
        issuer: issuer.publicKey, bond, ledger, action, paymentMint,
        issuerPaymentAccount: issuerPay, vault, tokenProgram,
      }).rpc());
    await step(`Sweep withholding (${kind}, SIMULATED tax authority)`, () =>
      ac.sweepWithholding().accountsPartial({
        issuer: issuer.publicKey, bond, action, paymentMint, vault, taxCollector: taxAta, tokenProgram,
      }).rpc());
    for (const h of holders) {
      const entitlement = pda.entitlement(action, h.kp.publicKey);
      if (kind === "coupon") {
        await step(`Claim coupon: ${h.name}`, () =>
          ac.claimCoupon().accountsPartial({
            holder: h.kp.publicKey, bond, action, entitlement, paymentMint, vault,
            holderPaymentAccount: h.payAta, tokenProgram,
          }).signers([h.kp]).rpc());
      } else {
        // Snapshot froze the bond account; frozen accounts cannot be burned, so thaw first
        // (only if frozen) in the SAME transaction as the claim.
        const frozen = (await getAccount(connection, h.bondAta, "confirmed")).isFrozen;
        const pre = frozen
          ? [await bc.thawHolder().accountsPartial({
              cranker: issuer.publicKey, bond, bondMint, holderRecord: h.record,
              holderTokenAccount: h.bondAta, tokenProgram,
            }).instruction()]
          : [];
        await step(`Redeem: ${h.name}${frozen ? " (thaw + claim in one tx)" : ""}`, () =>
          ac.claimRedemption().accountsPartial({
            holder: h.kp.publicKey, bond, holderRecord: h.record, action, entitlement, paymentMint, vault,
            holderPaymentAccount: h.payAta, bondMint, holderBondAccount: h.bondAta, tokenProgram,
          }).preInstructions(pre).signers([h.kp]).rpc());
      }
    }
  }

  // ---- 1. record date 1 + coupon -----------------------------------------------------
  await recordDate(1);
  await payAction("coupon", 1);

  // ---- 2. governance (voting must finish before record window 2) ----------------------
  const proposalId = 1;
  const proposal = pda.proposal(bond, proposalId);
  const t = await chainNow();
  const endTs = t + 90;
  const PROPOSAL_TITLE = "Approve covenant waiver";
  await step("Create proposal", () =>
    gv.createProposal(proposalId, PROPOSAL_TITLE, "One-time waiver of the leverage covenant (demo).",
      new BN(t - 30), new BN(endTs), 5000).accountsPartial({
      issuer: issuer.publicKey, bond, snapshotRecord: pda.snapshot(bond, 1), proposal, systemProgram,
    }).rpc());
  for (const h of holders) {
    await step(`Vote ${h.voteLabel}: ${h.name}`, () =>
      gv.castVote(h.vote).accountsPartial({
        voter: h.kp.publicKey, bond, proposal, holderRecord: h.record,
        voteRecord: pda.vote(proposal, h.kp.publicKey), systemProgram,
      }).signers([h.kp]).rpc());
  }
  await waitUntil(endTs, "voting window to end");
  await step("Finalize proposal", () =>
    gv.finalizeProposal().accountsPartial({ cranker: issuer.publicKey, bond, proposal }).rpc());

  // ---- 3. maturity -> record date 2 + redemption ----------------------------------------
  await waitUntil(maturity, "bond maturity");
  await recordDate(2);
  await payAction("redemption", 2);
  await step("Mark bond redeemed", () =>
    bc.markRedeemed().accountsPartial({ cranker: issuer.publicKey, bond, bondMint }).rpc());

  // ---- 4. verify against expectations computed independently ---------------------------
  log("\n=== VERIFICATION ===");
  let expectedTax = 0;
  for (const h of holders) {
    const gross = Math.floor((h.bonds * FACE * RATE_BPS) / (10_000 * PERIODS));
    const withheld = h.foreign ? Math.floor((gross * FOREIGN_WITHHOLDING_BPS) / 10_000) : 0;
    expectedTax += 2 * withheld;
    const expected = 2 * (gross - withheld) + h.bonds * FACE;
    check(`${h.name} payment balance`, (await getAccount(connection, h.payAta, "confirmed")).amount, expected);
    check(`${h.name} bonds left (burned)`, (await getAccount(connection, h.bondAta, "confirmed")).amount, 0);
  }
  check("Tax collector (SIMULATED)", (await getAccount(connection, taxAta, "confirmed")).amount, expectedTax);
  const bondState = await progs.bondCore.account.bondState.fetch(bond);
  const prop = await progs.governance.account.proposal.fetch(proposal);
  const bondStatus = Object.keys(bondState.status)[0];
  const propStatus = Object.keys(prop.status)[0];
  log("Bond status:", bondStatus, "| Proposal status:", propStatus);

  // ---- 5. export on-chain data for the UI ----------------------------------------------------
  const act1 = pda.action(bond, 1);
  const act2 = pda.action(bond, 2);
  const holderViews = [];
  for (const h of holders) {
    const e1 = await progs.actions.account.entitlement.fetch(pda.entitlement(act1, h.kp.publicKey));
    const e2 = await progs.actions.account.entitlement.fetch(pda.entitlement(act2, h.kp.publicKey));
    holderViews.push({
      name: h.name, wallet: h.kp.publicKey.toBase58(),
      jurisdiction: h.foreign ? "Foreign" : "Domestic", bonds: h.bonds,
      coupon: { gross: n(e1.gross), withheld: n(e1.withheld), net: n(e1.net) },
      redemption: {
        principal: n(e2.principal), couponGross: n(e2.couponGross),
        gross: n(e2.gross), withheld: n(e2.withheld), net: n(e2.net),
      },
      vote: { choice: h.voteLabel, weight: h.bonds },
      finalBalance: n((await getAccount(connection, h.payAta, "confirmed")).amount),
    });
  }

  fs.writeFileSync(
    path.join(process.cwd(), "public/demo-state.json"),
    JSON.stringify({
      cluster: "devnet",
      generatedAt: new Date().toISOString(),
      programs: { bondCore: BOND_CORE_ID.toBase58(), actions: ACTIONS_ID.toBase58(), governance: GOVERNANCE_ID.toBase58() },
      bond: {
        address: bond.toBase58(), mint: bondMint.toBase58(), paymentMint: paymentMint.toBase58(),
        issuer: issuer.publicKey.toBase58(), taxCollector: taxAta.toBase58(),
        faceValue: FACE, decimals: DECIMALS, couponRateBps: RATE_BPS, periods: PERIODS,
        maturityTs: maturity, status: bondStatus, totalCoupons: TOTAL_COUPONS,
      },
      holders: holderViews,
      proposal: {
        address: proposal.toBase58(), title: PROPOSAL_TITLE, status: propStatus,
        votesFor: n(prop.votesFor), votesAgainst: n(prop.votesAgainst), votesAbstain: n(prop.votesAbstain),
        voters: n(prop.voterCount), quorumBps: 5000,
      },
      steps,
    }, null, 2),
  );
  log(`\nBond: ${explorerAddr(bond)}\nWrote public/demo-state.json (${steps.length} transactions)`);
  log(process.exitCode === 1 ? "\nSOME CHECKS FAILED" : "\nALL CHECKS PASSED");
}

main().catch((e) => { console.error(e); process.exit(1); });
