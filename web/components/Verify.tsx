"use client";
import { useEffect, useState, type CSSProperties, type FormEvent } from "react";
import { Connection, PublicKey } from "@solana/web3.js";

const RPC = "https://api.devnet.solana.com";
const PROGRAMS = [
  { name: "bond-core", id: "GsYLz1ieJSo267DW3oga1k4hAmVoqTHgKaEw2K5HxJPT" },
  { name: "corporate-actions", id: "59hoGMNeum386CpHQpbsUStq51vfXS9iYxPkq5apthNg" },
  { name: "governance", id: "D4Zh5HNxQwuzqFJFsw5BFNBfkAGtW7Efk5VRTnFNwEqg" },
];
const SAMPLE = "8oC6TyaooxbUJsLM6JfUkUe4bngi7uwwUCmdy9fbWdK9";
const OWNERS: Record<string, string> = {
  GsYLz1ieJSo267DW3oga1k4hAmVoqTHgKaEw2K5HxJPT: "Tamga bond-core program",
  "59hoGMNeum386CpHQpbsUStq51vfXS9iYxPkq5apthNg": "Tamga corporate-actions program",
  D4Zh5HNxQwuzqFJFsw5BFNBfkAGtW7Efk5VRTnFNwEqg: "Tamga governance program",
  TokenkegQfeZyiNwAJbNbGKPFXCWuBvf9Ss623VQ5DA: "SPL Token program",
  "11111111111111111111111111111111": "System program (plain wallet)",
};
const addr = (a: string) => `https://explorer.solana.com/address/${a}?cluster=devnet`;
const tx = (s: string) => `https://explorer.solana.com/tx/${s}?cluster=devnet`;
const pick = (o: Record<string, unknown>, ks: string[]) => {
  for (const k of ks) {
    const v = o[k];
    if (typeof v === "string" && v) return v;
  }
  return undefined;
};

type Found = { owner: string; bytes: number; sol: number; exec: boolean } | null;

function Search() {
  const [q, setQ] = useState("");
  const [busy, setBusy] = useState(false);
  const [err, setErr] = useState("");
  const [res, setRes] = useState<{ key: string; info: Found } | null>(null);

  const run = async (value: string) => {
    const v = value.trim();
    setErr("");
    setRes(null);
    let pk: PublicKey;
    try { pk = new PublicKey(v); } catch { setErr("That is not a valid Solana address."); return; }
    setBusy(true);
    try {
      const info = await new Connection(RPC, "confirmed").getAccountInfo(pk);
      setRes({
        key: pk.toBase58(),
        info: info ? { owner: info.owner.toBase58(), bytes: info.data.length, sol: info.lamports / 1e9, exec: info.executable } : null,
      });
    } catch {
      setErr("Devnet RPC did not answer. The recorded run in the dashboard below still shows the same data.");
    } finally { setBusy(false); }
  };
  const submit = (e: FormEvent) => { e.preventDefault(); void run(q); };

  return (
    <div className="tg-search">
      <form onSubmit={submit}>
        <div className="tg-beam">
          <label>
            <svg width="18" height="18" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round"><circle cx="11" cy="11" r="7" /><path d="m20 20-3.5-3.5" /></svg>
            <input value={q} onChange={(e) => setQ(e.target.value)} placeholder="Paste any devnet address: bond, mint, holder or program" spellCheck={false} aria-label="Devnet address" />
            <button type="submit" className="tg-go" disabled={busy}>{busy ? "Reading…" : "Verify"}</button>
          </label>
        </div>
      </form>
      <button type="button" className="tg-link" onClick={() => { setQ(SAMPLE); void run(SAMPLE); }}>Try the sample bond from our devnet run</button>
      {err && <p className="tg-err">{err}</p>}
      {res && (
        <div className="tg-card tg-result">
          {res.info ? (
            <>
              <p><b>Account found on devnet.</b> Owned by: <em>{OWNERS[res.info.owner] ?? res.info.owner}</em></p>
              <p>{res.info.bytes} bytes of data, {res.info.sol.toFixed(4)} SOL{res.info.exec ? ", executable program" : ""}.</p>
              <a href={addr(res.key)} target="_blank" rel="noreferrer">Open in Solana Explorer →</a>
            </>
          ) : (
            <p>No account exists at that address on devnet.</p>
          )}
        </div>
      )}
    </div>
  );
}

function ProgramCheck() {
  const [st, setSt] = useState<(boolean | null)[]>([null, null, null]);
  const [bad, setBad] = useState(false);
  useEffect(() => {
    let live = true;
    new Connection(RPC, "confirmed")
      .getMultipleAccountsInfo(PROGRAMS.map((p) => new PublicKey(p.id)))
      .then((r) => { if (live) setSt(r.map((a) => (a ? a.executable : false))); })
      .catch(() => { if (live) setBad(true); });
    return () => { live = false; };
  }, []);
  return (
    <ul className="tg-progs">
      {PROGRAMS.map((p, i) => (
        <li key={p.id} className="tg-card">
          <span className={`tg-dot ${st[i] === true ? "ok" : st[i] === false ? "no" : ""}`} />
          <div>
            <b>{p.name}</b>
            <small>{bad ? "RPC unreachable" : st[i] === null ? "checking devnet…" : st[i] ? "executable: true" : "executable: false"}</small>
          </div>
          <a href={addr(p.id)} target="_blank" rel="noreferrer">Explorer →</a>
        </li>
      ))}
    </ul>
  );
}

function TxGrid() {
  const [steps, setSteps] = useState<Record<string, unknown>[]>([]);
  useEffect(() => {
    let live = true;
    fetch("/demo-state.json")
      .then((r) => r.json())
      .then((d: { steps?: unknown }) => {
        if (live && Array.isArray(d.steps)) setSteps(d.steps as Record<string, unknown>[]);
      })
      .catch(() => {});
    return () => { live = false; };
  }, []);
  const n = steps.length || 43;
  return (
    <div className="tg-txgrid" data-reveal>
      {Array.from({ length: n }).map((_, i) => {
        const s = steps[i];
        const label = (s && pick(s, ["label", "name", "title", "step", "action"])) || `Transaction ${i + 1}`;
        const sig = s && pick(s, ["signature", "sig", "tx", "txid"]);
        const href = (s && pick(s, ["explorer", "explorerUrl", "url", "link"])) || (sig ? tx(sig) : undefined);
        const style = { "--i": i, "--h": String(i / Math.max(1, n - 1)) } as CSSProperties;
        return href ? (
          <a key={i} className="tg-cell" style={style} href={href} target="_blank" rel="noreferrer" title={`${i + 1}. ${label}`} />
        ) : (
          <span key={i} className="tg-cell" style={style} title={`${i + 1}. ${label}`} />
        );
      })}
    </div>
  );
}

export default function Verify() {
  return (
    <section className="tg-sec" id="verify">
      <div className="tg-wrap">
        <p className="tg-kicker" data-reveal>Don&apos;t trust us, check the chain</p>
        <h2 className="tg-h2" data-reveal>Read any account straight from devnet.</h2>
        <div data-reveal><Search /></div>
        <div data-reveal><ProgramCheck /></div>
        <h3 className="tg-h3" data-reveal>One lifecycle, {43} transactions</h3>
        <p className="tg-note" data-reveal>Each square is one devnet transaction from the recorded run, in order. Hover for the step, click to open it in Explorer.</p>
        <TxGrid />
      </div>
    </section>
  );
}
