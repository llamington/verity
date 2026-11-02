# Handoff (2026-11-01)

Start here when resuming. The spec is `PLAN.md`; deviations are in `docs/discrepancies.md`; P5 scope
and gaps are in `docs/research/p5-plan.md`.

## Environment

- The .NET oracle needs `DOTNET_ROOT=$HOME/.dotnet`. It is built by `scripts/oracle-smoke.sh` /
  `scripts/build-data.sh`; the binary is `tools/pkhex-oracle/bin/Release/net9.0/PkhexOracle`.
- PKHeX source reference (read-only): `~/src/PKHeX` at tag 25.12.21. Port from it and cite files.
- Data: `scripts/build-data.sh` regenerates `data/exports` and `data/blobs` (gitignored). Most
  tests skip without the blobs, so rebuild them first.
- Corpus: `scripts/oracle-smoke.sh` writes `corpus/reports/oracle-smoke` (dumps used by tests).
  The real saves are in `corpus/real` (gitignored, see `docs/research/synthetic-corpus.md`).
- DS: `make` builds `nds/verity.nds` (BlocksDS in `/opt/wonderful`); `cargo build-ds` builds the
  ARM9 core. melonDS: `flatpak run net.kuribo64.melonDS`.
- Gate before committing: `cargo fmt --all`, `cargo test --workspace`,
  `cargo clippy --workspace --all-targets -- -D warnings`, `cargo build-ds`. Commit only once
  these pass. Don't chain commits after test commands.

## Done

- P0–P3 complete. P4 is complete for the M2 scope: reversals, MethodFinder, Method J, and the
  generator `gen/wild4.rs` (regular/shiny/Sync/Cute Charm/radar chain, IV-targeted,
  closest-legal-IVs).
- P5 (M2 scope): `legality/analysis4.rs` matches PKHeX on 1e5 fuzzed and 939 corpus D/P/Pt wild
  PK4 (`tests/p5_fuzz.rs`).
- P6, started: `gen/build4.rs` builds the full PK4. 1e4 built mons are Legal per verity and
  PKHeX (`p6_built_mons_legal_1e4`).

## Next steps

1. P6 API: `GenRequest` → generate → build → `analyze4` must be valid → insert into a box or
   party slot (party: `reset_party_stats`) → `Save::register_dex`. Return specific errors
   (`GenError`). Cover D/P learnsets (pick `dp.bin` vs `pt.bin` by version) and Unown (port
   `SolaceonRuins4`).
2. P7: game-safe validator (`safety/`) per PLAN P7; every write goes through it.
3. M3: P9.0 UI spike, then the minimal DS UI (file picker, box view, editor, add-Pokémon wizard).
   Load blobs from NitroFS.
4. Later: widen legality (statics/gifts/trades/eggs, HG/SS Method K, Pal Park, Gen 5), the word
   filter, the emulator harness (Phase H), mystery gifts, Memory Link.
