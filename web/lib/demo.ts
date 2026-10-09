export type StepLog = { step: string; sig: string; url: string; ts: number };

export type HolderView = {
  name: string;
  wallet: string;
  jurisdiction: "Domestic" | "Foreign";
  bonds: number;
  coupon: { gross: number; withheld: number; net: number };
  redemption: { principal: number; couponGross: number; gross: number; withheld: number; net: number };
  vote: { choice: string; weight: number };
  finalBalance: number;
};

export type DemoState = {
  live: boolean;
  cluster: string;
  generatedAt: string;
  programs: { bondCore: string; actions: string; governance: string };
  bond: {
    address: string; mint: string; paymentMint: string; issuer: string; taxCollector: string;
    faceValue: number; decimals: number; couponRateBps: number; periods: number;
    maturityTs: number; status: string; totalCoupons: number;
  };
  holders: HolderView[];
  proposal: {
    address: string; title: string; status: string; votesFor: number; votesAgainst: number;
    votesAbstain: number; voters: number; quorumBps: number;
  };
  steps: StepLog[];
};

const MOCK_LABELS = [
  "Issue bond", "Close record window 1", "Fund coupon action", "Claim coupon: Investor A",
  "Finalize proposal", "Open record window 2", "Close record window 2", "Redeem: Investor A",
  "Mark bond redeemed",
];

export const MOCK: DemoState = {
  live: false,
  cluster: "devnet",
  generatedAt: "",
  programs: {
    bondCore: "GsYLz1ieJSo267DW3oga1k4hAmVoqTHgKaEw2K5HxJPT",
    actions: "59hoGMNeum386CpHQpbsUStq51vfXS9iYxPkq5apthNg",
    governance: "D4Zh5HNxQwuzqFJFsw5BFNBfkAGtW7Efk5VRTnFNwEqg",
  },
  bond: {
    address: "SampleBond1111111111111111111111111111111111", mint: "SampleMint1111111111111111111111111111111111",
    paymentMint: "SamplePay1111111111111111111111111111111111", issuer: "SampleIssuer11111111111111111111111111111111",
    taxCollector: "SampleTax111111111111111111111111111111111",
    faceValue: 100000, decimals: 2, couponRateBps: 1000, periods: 2,
    maturityTs: Math.floor(Date.now() / 1000), status: "redeemed", totalCoupons: 2,
  },
  holders: [
    { name: "Investor A (domestic)", wallet: "SampleA1111111111111111111111111111111111111", jurisdiction: "Domestic", bonds: 10,
      coupon: { gross: 50000, withheld: 0, net: 50000 },
      redemption: { principal: 1000000, couponGross: 50000, gross: 1050000, withheld: 0, net: 1050000 },
      vote: { choice: "For", weight: 10 }, finalBalance: 1100000 },
    { name: "Investor B (foreign)", wallet: "SampleB1111111111111111111111111111111111111", jurisdiction: "Foreign", bonds: 5,
      coupon: { gross: 25000, withheld: 3750, net: 21250 },
      redemption: { principal: 500000, couponGross: 25000, gross: 525000, withheld: 3750, net: 521250 },
      vote: { choice: "Against", weight: 5 }, finalBalance: 542500 },
    { name: "Investor C (domestic)", wallet: "SampleC1111111111111111111111111111111111111", jurisdiction: "Domestic", bonds: 20,
      coupon: { gross: 100000, withheld: 0, net: 100000 },
      redemption: { principal: 2000000, couponGross: 100000, gross: 2100000, withheld: 0, net: 2100000 },
      vote: { choice: "For", weight: 20 }, finalBalance: 2200000 },
  ],
  proposal: {
    address: "SampleProposal11111111111111111111111111111", title: "Approve covenant waiver",
    status: "passed", votesFor: 30, votesAgainst: 5, votesAbstain: 0, voters: 3, quorumBps: 5000,
  },
  steps: MOCK_LABELS.map((step, i) => ({ step, sig: "", url: "", ts: Date.now() - (MOCK_LABELS.length - i) * 60000 })),
};

export async function loadDemo(): Promise<DemoState> {
  try {
    const res = await fetch("/demo-state.json", { cache: "no-store" });
    if (!res.ok) throw new Error("no demo file");
    const data = await res.json();
    return { ...data, live: true } as DemoState;
  } catch {
    return MOCK;
  }
}

export const money = (base: number, decimals = 2) =>
  (base / 10 ** decimals).toLocaleString("en-US", { minimumFractionDigits: decimals, maximumFractionDigits: decimals });
export const short = (a: string) => (a.length > 10 ? `${a.slice(0, 4)}…${a.slice(-4)}` : a);
export const explorerAddr = (a: string) => `https://explorer.solana.com/address/${a}?cluster=devnet`;
