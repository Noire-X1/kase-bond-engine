import "./tamga.css";
import Fx from "@/components/Fx";
import { Hero, Problem, HowItWorks } from "@/components/Landing";
import type { Metadata } from "next";
import type { ReactNode } from "react";
import { Fraunces, Instrument_Sans, JetBrains_Mono } from "next/font/google";
import "./globals.css";
import "./kase.css";

const display = Fraunces({ subsets: ["latin"], variable: "--font-display", style: ["normal", "italic"] });
const sans = Instrument_Sans({ subsets: ["latin"], variable: "--font-sans" });
const mono = JetBrains_Mono({ subsets: ["latin"], variable: "--font-mono" });

export const metadata: Metadata = {
  title: "Tamga: every bond event, stamped on-chain",
  description: "Automated corporate actions for tokenised bonds on Solana",
};

export default function RootLayout({ children }: { children: ReactNode }) {
  return (
    <html lang="en" className={`${display.variable} ${sans.variable} ${mono.variable}`}>
      <body><Fx />
<Hero />
<Problem />
<HowItWorks />
<div id="dashboard">{children}</div></body>
    </html>
  );
}
