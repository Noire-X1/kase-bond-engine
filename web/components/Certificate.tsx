"use client";
import { useRef } from "react";
import type { MouseEvent } from "react";
import { type DemoState, money, short } from "@/lib/demo";

type Props = { demo: DemoState; regularPaid: number; redeemed: boolean };

const FREQ: Record<number, string> = { 1: "annual", 2: "semi-annual", 4: "quarterly", 12: "monthly" };

export default function Certificate({ demo, regularPaid, redeemed }: Props) {
  const ref = useRef<HTMLDivElement>(null);
  const b = demo.bond;

  const move = (e: MouseEvent<HTMLDivElement>) => {
    const el = ref.current;
    if (!el) return;
    const r = el.getBoundingClientRect();
    const px = (e.clientX - r.left) / r.width;
    const py = (e.clientY - r.top) / r.height;
    el.style.setProperty("--rx", `${((0.5 - py) * 14).toFixed(2)}deg`);
    el.style.setProperty("--ry", `${((px - 0.5) * 18).toFixed(2)}deg`);
    el.style.setProperty("--gx", `${(px * 100).toFixed(1)}%`);
    el.style.setProperty("--gy", `${(py * 100).toFixed(1)}%`);
  };
  const leave = () => {
    const el = ref.current;
    if (!el) return;
    el.style.setProperty("--rx", "0deg");
    el.style.setProperty("--ry", "0deg");
    el.style.setProperty("--gx", "50%");
    el.style.setProperty("--gy", "50%");
  };

  const tabs = Array.from({ length: b.totalCoupons }, (_, i) => {
    const isFinal = i === b.totalCoupons - 1;
    return { n: i + 1, isFinal, paid: isFinal ? redeemed : i < regularPaid };
  });
  const bonds = demo.holders.reduce((s, h) => s + h.bonds, 0);

  return (
    <div className="cert-wrap">
      <div ref={ref} className="cert" onMouseMove={move} onMouseLeave={leave}>
        <div className="seal"><i /><span>KASE</span></div>
        <p className="cert-kicker">Tokenised corporate bond · Series {short(b.address)}</p>
        <h2 className="cert-title">Bond Certificate</h2>
        <p className="cert-sub">Issued on Solana {demo.cluster}. Every payment and burn below is a verifiable transaction.</p>
        <dl className="cert-grid">
          <div><dt>Face value</dt><dd>{money(b.faceValue, b.decimals)} <small>KZT*</small></dd></div>
          <div><dt>Coupon</dt><dd>{(b.couponRateBps / 100).toFixed(2)}% · {FREQ[b.periods] ?? `${b.periods}x per year`}</dd></div>
          <div><dt>Maturity</dt><dd>{new Date(b.maturityTs * 1000).toLocaleString()}</dd></div>
          <div><dt>Bonds issued</dt><dd>{bonds}</dd></div>
        </dl>
        <div className="coupons">
          {tabs.map((t) => (
            <div key={t.n} className={`coupon ${t.paid ? "paid" : ""}`}>
              COUPON {t.n}
              <span>{t.paid ? "PAID" : "DUE"}{t.isFinal ? " · with principal" : ""}</span>
            </div>
          ))}
        </div>
        <div className={`stamp ${redeemed ? "on" : ""}`}>REDEEMED</div>
        <p className="cert-foot">Issuer {short(b.issuer)} · Mint {short(b.mint)}</p>
      </div>
    </div>
  );
}
