#!/usr/bin/env bash
# Usage: scripts/deploy-program.sh <program_name> <max_len>
# Resumable deploy for a flaky devnet. A persistent buffer keypair means a failed upload
# never strands SOL, and every attempt writes only the chunks still missing.
# Attempts alternate between the direct-to-leader path (TPU) and the RPC path, and each
# is stopped after 8 minutes so a stall can never hang forever.
# Optional: RPC_URL=<private endpoint> scripts/deploy-program.sh <name> <max_len>
set -uo pipefail
trap 'echo; echo "interrupted. Progress is kept in the buffer; rerun the same command to resume."; exit 130' INT
cd "$(dirname "$0")/.."
NAME="$1"; MAXLEN="$2"
SO="target/litesvm/${NAME}.so"
KP="target/deploy/${NAME}-keypair.json"
BUF="$HOME/keys-backup/${NAME}-buffer.json"
URL_ARGS=()
[ -n "${RPC_URL:-}" ] && URL_ARGS=(--url "$RPC_URL")
FLAGS=(--max-sign-attempts 60 --with-compute-unit-price 100000)

[ -f "$SO" ] || { echo "missing $SO (run ./scripts/test.sh first)"; exit 1; }
[ -f "$KP" ] || { echo "missing $KP"; exit 1; }
[ -f "$BUF" ] || solana-keygen new --no-bip39-passphrase --silent -o "$BUF"

BUF_PUB="$(solana-keygen pubkey "$BUF")"
PAYER_PUB="$(solana address)"
if [ "$BUF_PUB" = "$PAYER_PUB" ]; then
  echo "ABORT: $BUF holds your deployer key, not a buffer key. Delete it and rerun."
  exit 1
fi
echo "program: $NAME   buffer: $BUF_PUB"

ok=0
for i in 1 2 3 4 5 6 7 8 9 10 11 12; do
  if [ $((i % 3)) -eq 0 ]; then MODE=(--use-rpc); LABEL="rpc"; else MODE=(); LABEL="tpu"; fi
  echo "== write-buffer attempt $i of 12 ($LABEL path)"
  if timeout --foreground 480 solana program write-buffer "$SO" --buffer "$BUF" "${MODE[@]}" "${FLAGS[@]}" "${URL_ARGS[@]}"; then ok=1; break; fi
  sleep 5
done
[ $ok -eq 1 ] || { echo "write-buffer did not finish; progress is kept, rerun the same command."; exit 1; }

for j in 1 2 3; do
  echo "== deploy attempt $j of 3"
  if timeout --foreground 300 solana program deploy --buffer "$BUF" --program-id "$KP" --max-len "$MAXLEN" "${FLAGS[@]}" "${URL_ARGS[@]}"; then exit 0; fi
  sleep 5
done
echo "deploy step did not finish. Run 'solana program show <program id>' to check whether it actually landed."
exit 1
