import { Connection, PublicKey } from "@solana/web3.js";

const RPC = "https://api.devnet.solana.com";
const [bondMint, payMint, ...pairs] = process.argv.slice(2);
if (!bondMint || !payMint || pairs.length === 0) {
  console.log('usage: npx tsx scripts/verify-chain.ts <BOND_MINT> <PAYMENT_MINT> "A=<wallet>" "B=<wallet>" ...');
  process.exit(1);
}
const conn = new Connection(RPC, "confirmed");
const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));
async function retry<T>(fn: () => Promise<T>): Promise<T> {
  for (let i = 0; i < 4; i++) {
    try {
      return await fn();
    } catch {
      await sleep(1500 * (i + 1));
    }
  }
  return fn();
}
(async () => {
  console.log("Reading Solana devnet live: " + RPC + "\n");
  const supply = await retry(() => conn.getTokenSupply(new PublicKey(bondMint)));
  console.log("Bond mint " + bondMint);
  console.log("  bonds in circulation now: " + supply.value.uiAmountString + "\n");
  console.log("Payment token (KZT*, simulated) " + payMint);
  for (const p of pairs) {
    const [label, wallet] = p.split("=");
    const r = await retry(() =>
      conn.getParsedTokenAccountsByOwner(new PublicKey(wallet), { mint: new PublicKey(payMint) })
    );
    const total = r.value.reduce(
      (s, a) => s + Number(a.account.data.parsed.info.tokenAmount.uiAmountString),
      0
    );
    console.log("  " + label + ": " + total.toFixed(2) + " KZT*  (" + wallet.slice(0, 4) + "..." + wallet.slice(-4) + ")");
  }
})();
