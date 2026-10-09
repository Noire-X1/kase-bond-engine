"use client";
import { useEffect, useState } from "react";
import Certificate from "@/components/Certificate";
import { type DemoState, type HolderView, explorerAddr, loadDemo, money, short } from "@/lib/demo";

type Tab = "issuer" | "investor" | "audit";

function Row({ k, v, cls = "" }: { k: string; v: string; cls?: string }) {
  return (
    <div className={`row ${cls}`}>
      <span>{k}</span>
      <b>{v}</b>
    </div>
  );
}

export default function Home() {
  const [demo, setDemo] = useState<DemoState | null>(null);
  const [tab, setTab] = useState<Tab>("issuer");
  const [who, setWho] = useState(0);

  useEffect(() => {
    loadDemo().then(setDemo);
  }, []);

  if (!demo) {
    return (
      <main className="shell">
        <p className="muted">Loading ledger…</p>
      </main>
    );
  }

  const d = demo.bond.decimals;
  const has = (prefix: string) => demo.steps.some((s) => s.step.startsWith(prefix));
  const regularPaid = demo.steps.filter((s) => s.step === "Fund coupon action").length;
  const redeemed = has("Mark bond redeemed");
  const sum = (f: (h: HolderView) => number) => demo.holders.reduce((a, h) => a + f(h), 0);

  const stages = [
    { label: "Bond issued", note: "Terms fixed on-chain", done: has("Issue bond") },
    { label: "Record date 1", note: "Holders snapshotted and frozen", done: has("Close record window 1") },
    { label: "Coupon paid", note: "Entitlements calculated, funded, claimed", done: has("Claim coupon") },
    { label: "Bondholder vote", note: "Weighted by snapshot holdings", done: has("Finalize proposal") },
    { label: "Maturity reached", note: "Redemption record date opens", done: has("Open record window 2") },
    { label: "Record date 2", note: "Final holders confirmed", done: has("Close record window 2") },
    { label: "Redemption and burn", note: "Principal plus final coupon paid, tokens burned", done: has("Redeem:") },
    { label: "Bond retired", note: "Supply is zero, status Redeemed", done: redeemed },
  ];

  const h = demo.holders[who] ?? demo.holders[0];
  const p = demo.proposal;
  const totalVotes = Math.max(1, p.votesFor + p.votesAgainst + p.votesAbstain);

  return (
    <main className="shell">
      <header className="topbar rise">
        <div className="brand">
          <b>Corporate Action Engine</b>
          <span>KASE · Solana</span>
        </div>
        <div className="badges">
          <span className="badge">Solana {demo.cluster}</span>
          <span className={`badge ${demo.live ? "live" : "sample"}`}>
            {demo.live ? "Live on-chain data" : "Sample data: run the lifecycle script"}
          </span>
          <span className="badge">Payment rail: simulated</span>
        </div>
      </header>

      <nav className="tabs rise" style={{ ["--d" as string]: "80ms" }}>
        {(["issuer", "investor", "audit"] as Tab[]).map((t) => (
          <button key={t} className="tab" aria-pressed={tab === t} onClick={() => setTab(t)}>
            {t === "issuer" ? "Issuer" : t === "investor" ? "Investor" : "Public audit"}
          </button>
        ))}
      </nav>

      {tab === "issuer" && (
        <section className="grid2">
          <div className="rise" style={{ ["--d" as string]: "140ms" }}>
            <Certificate demo={demo} regularPaid={regularPaid} redeemed={redeemed} />
          </div>
          <div className="rise" style={{ ["--d" as string]: "220ms" }}>
            <div className="stats">
              <div className="stat"><small>Coupon paid (gross)</small><strong>{money(sum((x) => x.coupon.gross), d)}</strong></div>
              <div className="stat"><small>Tax withheld (coupons)</small><strong>{money(sum((x) => x.coupon.withheld), d)}</strong></div>
              <div className="stat"><small>Redemption paid (net)</small><strong>{money(sum((x) => x.redemption.net), d)}</strong></div>
              <div className="stat"><small>Bonds burned</small><strong>{redeemed ? sum((x) => x.bonds) : 0}</strong></div>
            </div>
            <div className="card">
              <h3>Lifecycle</h3>
              <ol className="timeline">
                {stages.map((s) => (
                  <li key={s.label} className={`tl ${s.done ? "done" : ""}`}>
                    <b>{s.label}</b>
                    <small>{s.note}</small>
                  </li>
                ))}
              </ol>
            </div>
          </div>
        </section>
      )}

      {tab === "investor" && (
        <section className="grid2">
          <div className="rise">
            <div className="chips">
              {demo.holders.map((x, i) => (
                <button key={x.wallet} className="chip" aria-pressed={who === i} onClick={() => setWho(i)}>
                  {x.name}
                </button>
              ))}
            </div>
            <div className="card">
              <h3>{h.name}</h3>
              <div className="rows">
                <Row k="Wallet" v={short(h.wallet)} />
                <Row k="Bonds held at record date" v={String(h.bonds)} />
                <Row k="Jurisdiction flag (simulated)" v={h.jurisdiction} />
                <Row k="Vote cast" v={`${h.vote.choice} · weight ${h.vote.weight}`} />
              </div>
            </div>
          </div>
          <div className="rise" style={{ ["--d" as string]: "100ms" }}>
            <div className="card" style={{ marginBottom: 18 }}>
              <h3>Coupon entitlement</h3>
              <div className="rows">
                <Row k="Gross" v={`${money(h.coupon.gross, d)} KZT*`} />
                <Row k="Tax withheld" v={`${money(h.coupon.withheld, d)} KZT*`} cls="tax" />
                <Row k="Net paid" v={`${money(h.coupon.net, d)} KZT*`} cls="total" />
              </div>
            </div>
            <div className="card">
              <h3>Redemption at maturity</h3>
              <div className="rows">
                <Row k="Principal" v={`${money(h.redemption.principal, d)} KZT*`} />
                <Row k="Final coupon" v={`${money(h.redemption.couponGross, d)} KZT*`} />
                <Row k="Tax withheld (coupon only)" v={`${money(h.redemption.withheld, d)} KZT*`} cls="tax" />
                <Row k="Net paid" v={`${money(h.redemption.net, d)} KZT*`} cls="total" />
                <Row k="Final balance (coupon + redemption)" v={`${money(h.finalBalance, d)} KZT*`} />
              </div>
              <p className="note">Bonds burned in the same transaction as the payout: {redeemed ? h.bonds : 0} of {h.bonds}.</p>
            </div>
          </div>
        </section>
      )}

      {tab === "audit" && (
        <section className="grid2">
          <div className="rise">
            <div className="card" style={{ marginBottom: 18 }}>
              <h3>Bondholder vote: {p.title}</h3>
              <div className="bar">
                <i className="f" style={{ width: `${(p.votesFor / totalVotes) * 100}%` }} />
                <i className="a" style={{ width: `${(p.votesAgainst / totalVotes) * 100}%` }} />
                <i className="x" style={{ width: `${(p.votesAbstain / totalVotes) * 100}%` }} />
              </div>
              <div className="rows">
                <Row k="For / Against / Abstain (weighted)" v={`${p.votesFor} / ${p.votesAgainst} / ${p.votesAbstain}`} />
                <Row k="Voters" v={String(p.voters)} />
                <Row k="Quorum required" v={`${(p.quorumBps / 100).toFixed(0)}%`} />
                <Row k="Result (final, on-chain)" v={p.status.toUpperCase()} cls="total" />
              </div>
            </div>
            <div className="card">
              <h3>On-chain accounts</h3>
              <div className="rows">
                {[
                  ["Bond", demo.bond.address], ["Bond mint", demo.bond.mint], ["Proposal", p.address],
                  ["Corporate actions program", demo.programs.actions], ["Bond core program", demo.programs.bondCore],
                  ["Governance program", demo.programs.governance],
                ].map(([k, a]) => (
                  <div className="row" key={k}>
                    <span>{k}</span>
                    {demo.live ? <a className="mono" href={explorerAddr(a)} target="_blank" rel="noreferrer" style={{ color: "var(--brass-2)" }}>{short(a)} ↗</a> : <b>{short(a)}</b>}
                  </div>
                ))}
              </div>
            </div>
          </div>
          <div className="card rise" style={{ ["--d" as string]: "100ms" }}>
            <h3>Every action, verifiable ({demo.steps.length})</h3>
            <div style={{ maxHeight: 560, overflow: "auto" }}>
              <table className="audit">
                <thead><tr><th>#</th><th>Action</th><th>Proof</th></tr></thead>
                <tbody>
                  {demo.steps.map((s, i) => (
                    <tr key={`${s.step}-${i}`}>
                      <td className="mono">{i + 1}</td>
                      <td>{s.step}</td>
                      <td>{s.url ? <a href={s.url} target="_blank" rel="noreferrer">{short(s.sig)} ↗</a> : <span className="muted">sample</span>}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </div>
        </section>
      )}

      <p className="legend">
        KZT* is a test SPL token standing in for tenge: the payment rail is simulated. Jurisdiction flags and the 15% withholding
        rate are demo parameters, not tax advice. Everything else (holder registry, record-date snapshots, entitlement maths,
        payouts, token burns, votes) runs as real Solana program calls on devnet.
      </p>
    </main>
  );
}
