#!/usr/bin/env bash
# P3: export data from PKHeX and build the blobs the core reads (data/blobs/*.bin).
set -euo pipefail
cd "$(dirname "$0")/.."
export DOTNET_ROOT="${DOTNET_ROOT:-$HOME/.dotnet}"
dotnet build tools/pkhex-oracle -c Release -v q --nologo >/dev/null
ORACLE=tools/pkhex-oracle/bin/Release/net9.0/PkhexOracle
"$ORACLE" export-data data/exports >/dev/null
"$ORACLE" export-encounters data/exports >/dev/null
"$ORACLE" export-gifts4 data/exports/gifts4.json >/dev/null
"$ORACLE" export-text4 data/exports/text4.json
cargo run -q -p datagen -- build data/exports data/blobs
