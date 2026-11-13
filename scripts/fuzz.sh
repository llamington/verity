#!/usr/bin/env bash
# Run each fuzz target (default 600 s each, in parallel). Needs cargo-fuzz.
set -euo pipefail
cd "$(dirname "$0")/../fuzz"
SECS="${1:-600}"
LOG="${VERITY_CORPUS_DIR:-../corpus}/reports/fuzz"
mkdir -p "$LOG"
cargo fuzz build -O
cargo fuzz run -O save_patch -- -max_total_time="$SECS" -max_len=4096 >"$LOG/save_patch.log" 2>&1 & a=$!
cargo fuzz run -O pkm_decode -- -max_total_time="$SECS" -max_len=300 >"$LOG/pkm_decode.log" 2>&1 & b=$!
fail=0
wait $a || { echo "save_patch FAILED (see $LOG/save_patch.log)"; fail=1; }
wait $b || { echo "pkm_decode FAILED (see $LOG/pkm_decode.log)"; fail=1; }
grep -h '^Done' "$LOG"/*.log
exit $fail
