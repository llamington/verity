#!/usr/bin/env bash
# P1 acceptance: run sav-info, sav-roundtrip, sav-dump and legality-batch over the corpus.
set -euo pipefail
cd "$(dirname "$0")/.."

export DOTNET_ROOT="${DOTNET_ROOT:-$HOME/.dotnet}"
CORPUS="${VERITY_CORPUS_DIR:-corpus}"
ORACLE=tools/pkhex-oracle/bin/Release/net9.0/PkhexOracle
OUT="${CORPUS}/reports/oracle-smoke"

dotnet build tools/pkhex-oracle -c Release -v q --nologo >/dev/null

if [ ! -d "$CORPUS/synthetic" ] || [ -z "$(ls -A "$CORPUS/synthetic" 2>/dev/null)" ]; then
    "$ORACLE" generate-synthetic "$CORPUS/synthetic" >/dev/null
fi

rm -rf "$OUT" && mkdir -p "$OUT"
fail=0
while IFS= read -r -d '' sav; do
    name=$(basename "$sav" .sav)
    if ! "$ORACLE" sav-info "$sav" >"$OUT/$name.info.json" 2>"$OUT/$name.err"; then
        echo "FAIL sav-info $sav"; fail=1; continue
    fi
    if ! "$ORACLE" sav-roundtrip "$sav" --out "$OUT/$name.rt" | grep -q '"byte_identical": true'; then
        echo "FAIL roundtrip $sav"; fail=1
    fi
    "$ORACLE" sav-dump "$sav" "$OUT/$name.pkm" >"$OUT/$name.dump.json"
    "$ORACLE" legality-batch "$OUT/$name.pkm" >"$OUT/$name.legality.jsonl"
    total=$(wc -l <"$OUT/$name.legality.jsonl")
    legal=$(grep -c '"valid":true' "$OUT/$name.legality.jsonl" || true)
    echo "ok   $name: $legal/$total legal"
    rm -f "$OUT/$name.rt"
done < <(find "$CORPUS" -name '*.sav' -not -path "*/reports/*" -print0 | sort -z)

exit $fail
