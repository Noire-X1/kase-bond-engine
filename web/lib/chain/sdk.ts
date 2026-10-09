import { PublicKey } from "@solana/web3.js";
import { AnchorProvider, Program, type Idl } from "@anchor-lang/core";

export const BOND_CORE_ID = new PublicKey("GsYLz1ieJSo267DW3oga1k4hAmVoqTHgKaEw2K5HxJPT");
export const ACTIONS_ID = new PublicKey("59hoGMNeum386CpHQpbsUStq51vfXS9iYxPkq5apthNg");
export const GOVERNANCE_ID = new PublicKey("D4Zh5HNxQwuzqFJFsw5BFNBfkAGtW7Efk5VRTnFNwEqg");

const enc = new TextEncoder();
const seed = (s: string) => enc.encode(s);

/** Little-endian bytes of an unsigned integer (matches Rust to_le_bytes). */
export function le(value: number | bigint, size: number): Uint8Array {
  let v = BigInt(value);
  const out = new Uint8Array(size);
  const mask = BigInt(255);
  const eight = BigInt(8);
  for (let i = 0; i < size; i++) {
    out[i] = Number(v & mask);
    v = v >> eight;
  }
  return out;
}

const find = (seeds: Uint8Array[], programId: PublicKey) =>
  PublicKey.findProgramAddressSync(seeds, programId)[0];

export const pda = {
  bond: (issuer: PublicKey, bondId: number | bigint) =>
    find([seed("bond"), issuer.toBytes(), le(bondId, 8)], BOND_CORE_ID),
  bondMint: (bond: PublicKey) => find([seed("bond_mint"), bond.toBytes()], BOND_CORE_ID),
  holder: (bond: PublicKey, wallet: PublicKey) =>
    find([seed("holder"), bond.toBytes(), wallet.toBytes()], BOND_CORE_ID),
  snapshot: (bond: PublicKey, epoch: number) =>
    find([seed("snapshot"), bond.toBytes(), le(epoch, 4)], BOND_CORE_ID),
  ledger: (bond: PublicKey) => find([seed("ledger"), bond.toBytes()], ACTIONS_ID),
  action: (bond: PublicKey, epoch: number) =>
    find([seed("action"), bond.toBytes(), le(epoch, 4)], ACTIONS_ID),
  vault: (action: PublicKey) => find([seed("vault"), action.toBytes()], ACTIONS_ID),
  entitlement: (action: PublicKey, wallet: PublicKey) =>
    find([seed("entitlement"), action.toBytes(), wallet.toBytes()], ACTIONS_ID),
  proposal: (bond: PublicKey, id: number) =>
    find([seed("proposal"), bond.toBytes(), le(id, 4)], GOVERNANCE_ID),
  vote: (proposal: PublicKey, wallet: PublicKey) =>
    find([seed("vote"), proposal.toBytes(), wallet.toBytes()], GOVERNANCE_ID),
};

export type Idls = { bondCore: Idl; actions: Idl; governance: Idl };

export function loadPrograms(provider: AnchorProvider, idls: Idls) {
  return {
    bondCore: new Program(idls.bondCore, provider),
    actions: new Program(idls.actions, provider),
    governance: new Program(idls.governance, provider),
  };
}

export const explorerTx = (sig: string) =>
  `https://explorer.solana.com/tx/${sig}?cluster=devnet`;
export const explorerAddr = (a: PublicKey | string) =>
  `https://explorer.solana.com/address/${a.toString()}?cluster=devnet`;
