import type { CSSProperties } from "react";
import { TamgaMark, Ornament } from "./TamgaMark";

const d = (i: number) => ({ "--i": i }) as CSSProperties;

export function Hero() {
  return (
    <header className="tg-hero" id="top">
      <div className="tg-lamp" aria-hidden="true"><i className="l" /><i className="r" /><i className="glow" /><i className="core" /><i className="bar" /></div>
      <div className="tg-hero-inner">
        <TamgaMark size={104} className="tg-hero-mark" />
        <h1 className="tg-wordmark">Tamga</h1>
        <p className="tg-tagline">Every bond event, stamped on-chain.</p>
        <p className="tg-sub">
          The servicing layer for tokenised bonds on Solana: snapshot holders on a record date,
          calculate what each is owed, pay coupons, retire bonds at maturity and run bondholder
          votes, with a permanent on-chain record of every step.
        </p>
        <div className="tg-actions">
          <a className="tg-btn" href="#dashboard">See the live lifecycle</a>
          <a className="tg-btn tg-btn-ghost" href="#problem">Why it matters</a>
        </div>
        <ul className="tg-counters">
          <li><b data-count="3">3</b><span>Solana programs on devnet</span></li>
          <li><b data-count="25">25</b><span>integration tests passing</span></li>
          <li><b data-count="43">43</b><span>devnet transactions, one full lifecycle</span></li>
        </ul>
      </div>
      <a className="tg-cue" href="#problem" aria-label="Scroll to next section">
        <svg width="26" height="26" viewBox="0 0 24 24" fill="none" stroke="currentColor" strokeWidth="2" strokeLinecap="round" strokeLinejoin="round">
          <path d="M12 5v14M5 12l7 7 7-7" />
        </svg>
      </a>
    </header>
  );
}

export function Problem() {
  return (
    <section className="tg-sec" id="problem">
      <Ornament id="tg-kosh-a" />
      <div className="tg-wrap">
        <p className="tg-kicker" data-reveal>The problem</p>
        <h2 className="tg-h2" data-reveal style={d(1)}>
          Paying a bond coupon is still a chain of manual steps.
        </h2>
        <div className="tg-bento">
          <article className="tg-card b-wide" data-reveal style={d(2)}>
            <div className="tg-stat"><b data-count="46">46</b><i>%</i></div>
            <p>of corporate-action data is still processed manually.</p>
            <small>Source: SIX Group</small>
          </article>
          <article className="tg-card b-narrow" data-reveal style={d(3)}>
            <div className="tg-stat"><b>T+2</b></div>
            <p>KASE corporate bonds settle two business days after trade.</p>
            <small>Source: KASE</small>
          </article>
          <article className="tg-card b-full" data-reveal style={d(4)}>
            <p>
              A corporate action is any event that changes what a bondholder owns or is owed:
              a coupon (interest payment), redemption (the bond is repaid and retired) or a vote.
              Tamga does each one the same way, in code, so the answer to who is owed what is
              calculated from one on-chain record and cannot quietly change afterwards.
            </p>
          </article>
        </div>
      </div>
    </section>
  );
}

const STEPS = [
  { n: "01", t: "Snapshot", c: "b-wide", p: "On the record date, every holder's balance is recorded and their bond token account is frozen. The window only closes when the snapshot adds up to the full supply, so nobody can be missed or counted twice." },
  { n: "02", t: "Calculate", c: "b-narrow", p: "Each holder's entitlement is computed with integer-safe maths and stored on-chain. Example: face 1,000, 10% a year, paid twice a year, 10 bonds = 500 per coupon." },
  { n: "03", t: "Pay", c: "b-narrow", p: "The issuer funds a vault; holders claim their own payment. Withholding tax is split off the coupon part only, never the principal." },
  { n: "04", t: "Record", c: "b-wide", p: "Every action keeps a permanent on-chain record, and at maturity the bonds are burned so the supply returns to zero. Anyone can verify it on Solana Explorer." },
];

export function HowItWorks() {
  return (
    <section className="tg-sec tg-sec-alt" id="how">
      <Ornament id="tg-kosh-b" />
      <div className="tg-wrap">
        <p className="tg-kicker" data-reveal>How it works</p>
        <h2 className="tg-h2" data-reveal style={d(1)}>Snapshot, calculate, pay, record.</h2>
        <div className="tg-bento">
          {STEPS.map((s, i) => (
            <article key={s.n} className={`tg-card ${s.c}`} data-reveal style={d(i + 2)}>
              <span className="tg-step">{s.n}</span>
              <h3>{s.t}</h3>
              <p>{s.p}</p>
            </article>
          ))}
        </div>
        <p className="tg-note" data-reveal>
          Implemented on devnet: issuance, holder registry, record-date window, coupons, redemption,
          voting. Simulated: the KZT* payment token, tax rates and tax authority, and any KASE or
          depository integration.
        </p>
      </div>
      <Ornament id="tg-kosh-c" />
    </section>
  );
}
