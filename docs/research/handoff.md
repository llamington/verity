# Handoff (2026-11-02)

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
- Emulator: `make emu-harness` builds the headless melonDS 1.1 harness (`~/src/melonDS`);
  `make emu-accept` boots retail Pt/B2 (owner ROMs in `roms/`, BIOS in `bios/`, gitignored);
  `make emu-ui` runs the DS walk-throughs (`tools/emu-harness/scripts/ui-*.json`) on a fresh copy
  of `sd/` in `build/emu/sd`. Screenshots land in `build/emu/<script>/`; look at them with Read.
  `tools/emu/melon.py` drives the GUI melonDS (X11 mode) if needed.
- Oracle extras: `gen-wild4` (PKHeX-generated legal wild PK4s), `legality-batch`, `sav-dump`.
- Gate before committing: `cargo fmt --all`, `cargo test --workspace`,
  `cargo clippy --workspace --all-targets -- -D warnings`, `cargo build-ds`. Commit only once
  these pass. Don't chain commits after test commands.

## Done

- P0–P3 complete. P4 is complete for the M2 scope: reversals, MethodFinder, Method J, and the
  generator `gen/wild4.rs` (regular/shiny/Sync/Cute Charm/radar chain, IV-targeted,
  closest-legal-IVs).
- P5 (M2 scope): `legality/analysis4.rs` matches PKHeX on 1e5 fuzzed and 939 corpus D/P/Pt wild
  PK4 (`tests/p5_fuzz.rs`).
- P6 (M2 scope, D/P/Pt wild): `gen/build4.rs` builds the PK4; `gen/api4.rs` is the request API
  (`GenRequest` → `generate`/`generate_for` → legality gate → `insert` + `apply_capture_side_effects`).
  Specific slot or "any encounter"; level/nature/gender/ability/shiny/lead/HP type; IVs random,
  exact or nearest; ball from the D/P/Pt wild set; moves natural or chosen (gated). D/P use
  `dp.bin` learnsets, Pt uses `pt.bin`. Unown: `rng/unown4.rs` ports `SolaceonRuins4`, wired into
  the matcher (partial match) and the generator (`Request::form`). `tests/p6_api.rs`: 10,552
  results from 16k random requests across D/P/Pt (1,239 Unown) all PKHeX-legal
  (`p6_api_random_requests_1e4`, ignored by default); specific errors; insertion into the real
  Pt and Diamond saves dumps as legal with valid checksums and party stats equal to PKHeX's.
- P7 (first pass): `safety/` — `check_pkm` (format, checksum, sanity word, species/form/ability/
  held item/move IDs, duplicates, PP/PP Ups, nature, EXP/level, nickname/OT termination and
  charset, language, origin game, ball, met/egg locations, party level/HP), `check_party`,
  `check_save`, and `checked_set_pkm` (the write path: validates, keeps the party contiguous,
  requires exactly computed party stats, rolls back a write that leaves no usable party member).
  `api4::insert` writes through it. `tests/p7_safety.rs`: all 4,884 Pokémon in the real saves pass
  except 7 documented PKMDS editor artifacts (still rejected); curated crash classes are rejected;
  rejected writes leave the save byte-identical. Bounds and the stale-party-stats rule are in
  `docs/discrepancies.md`. P7's harness acceptance (fuzzer vs the game) waits on Phase H.
- Fixed: a met level between the slot max and the Pressure level was accepted by the generator
  (PKHeX allows only the range or exactly the Pressure level, which needs a Pressure lead).

- Phase H (partial): `tools/emu-harness` runs melonDS 1.1 headless from JSON scripts,
  deterministically (see `docs/research/emu-harness.md`). Use it for DS visual checks
  (`scripts/ui-boxes.json`, `make emu-ui`). Retail acceptance passes: Platinum and Black 2 boot to
  title and Continue with real saves, deterministically (`make emu-accept`). Two-instance
  multiplayer is still open. Owner ROMs (Pt, HG, W, B2) and BIOS are in `roms/`, `bios/`.

- M4 (started): HG/SS wild slots. `rng/method_k.rs` (PKHeX MethodK/SlotMethodK, incl. Safari
  and Bug-Catching Contest re-rolls), Ruins of Alph Unown (`unown4::alph_*`), Safari Ball at
  location 202, Apricorn Balls; `Context4::enc_hgss`. Parity: leads/seeds on 600 corpus HG/SS
  wild PK4 (`oracle_method_k`), corpus legality now 1,597 PK4 (was 939), fuzz 600/600
  (`p5_parity_fuzz_hgss`, PKHeX-generated via oracle `gen-wild4`). The ignored 1e4 run has 1
  mismatch in 10,000: a mutated Dragon's Den Dratini where PKHeX falls back to the static gift
  (statics are not modelled yet).

- M6 (legality, first part): Gen 4 Mystery Gifts. The oracle exports PKHeX's `MGDB_G4`
  (`export-gifts4`, 590 cards; the 247 Pokémon cards go into `gifts4.bin`, ~210 KB, loaded from
  NitroFS). `gift/pcd.rs` parses PCD/PGT; `legality/gift4.rs` ports `PCD.IsMatchExact` (template
  level from EXP, not the stat byte), the version-partial rule (cards store version D or HG, so a
  gift on another game is a PartialMatch flagged Encounter, as in PKHeX), `PGT.IsCompatible`,
  `IsRangerManaphy`/`IsG4ManaphyPIDValid` and the Event states (events first for fateful mons;
  once any card matches nothing else is tried; a partial joins the enumerator's deferral).
  `enc4::Enc4` is the candidate type (blob template, gift, Ranger Manaphy); `Matched::Gift` /
  `Matched::RangerManaphy`. Gift verifiers: met/current level, PCD ability, fateful,
  receivability, Shiny rule + fixed PID, fixed IVs, fixed ball, no player-OT check, nicknamed
  gift, event ribbons (Event3/Event4 for every encounter). Also new for all encounters: OT
  length (6 JPN/KOR, 12 else) and `char.IsNumber` digit counting. Parity: corpus 2,922 PK4
  incl. 61 PCD + 550 Ranger Manaphy (same MGDB index asserted); fuzz `p5_parity_fuzz_gifts`
  600 (oracle `gen-gift4`, mutations incl. event ribbons/version), ignored 1e4 run 9,999/9,999.
- Gen 4 ribbon verifier (`legality/ribbons4.rs`): PKHeX's PK4 ribbon sets for Gen 4 origins
  (Event3/4 as the encounter had them, no Gen 3 contest/Winning/Victory/Champion G3/Artist,
  Record never, frontier ability ribbons by ban list, Sinnoh contest ranks in sequence,
  Gorgeous -> Royal -> Gorgeous Royal, spiky Pichu static exclusions, eggs carry nothing). All
  fuzzers now flip any ribbon bit: 1e5 wild, HG/SS 1e4 and gifts 1e4 clean.

## Next steps

M4 status (2026-11-02): Gen 4 legality covers wild slots (D/P/Pt Method J, HG/SS Method K),
statics, Pokéwalker, in-game trades, bred eggs and gift eggs (hatched). Corpus parity 2,318 PK4
(`p5_parity_corpus`, 2,922 since M6 gifts); real saves `gen4_*`: every PK4 has a verdict
matching PKHeX (D 531, Pt 531, HG 537). Still out of scope in the corpus: Gen 3 origins (Pal
Park, ~99 in the JP HG save) and D Ranch trades (6). Remaining M4: unhatched eggs, D Ranch gifts,
Pal Park (Gen 3 origins), egg generation. HG/SS generation is done: the generator dispatches
Method J/K by `Request::version`; P6 acceptance 10,562 PKHeX-legal across D/P/Pt/HG/SS; the DS
wizard offers HeartGold slots on HG/SS saves (verified end to end with `ui-add-hgss.json` +
PKHeX). Perf note: Ruins of Alph Unown legality takes ~107 ms on DS (the form recursion);
optimise before P9 acceptance (budget 50 ms).

**M6 starting point (legality done, see Done):** PKHeX's Gen 4 event DB is `Resources/legality/mgdb/wc4.pkl` (590 PCD
records of 0x358 bytes, `EncounterEvent.MGDB_G4`); legality matching lives in
`MysteryGifts/PGT.cs` (`IsMatchExact`, PID types/shiny locks, egg gifts) and `PCD.cs`, and the
enumerator checks events first for fateful Pokémon (`EncounterEnumerator4` Event/EventLocal,
plus `PGT.IsRangerManaphy`). Plan: export the DB through the oracle into a gift blob, port
PGT/PCD parsing + matching, extend corpus parity to PGT/PCD, then P8 injection (R3 region
research, R4 unlock flags, pending-gift slots, duplicate prevention, harness collection test).

Front end (P9.1) so far: scene stack, file picker (remembers last file), save overview + Save
to SD (validator, rotating `.bak1..3`, temp+rename, read-back), box view with legality verdicts,
editor (Main/Stats/Moves, Legal/Game-safe mode), add-Pokémon wizard (D/P/Pt/HG wild, nature/
shiny/level/gender/lead/ball). Details: `docs/research/ui-stack.md`, `emu-harness.md`.

M6 P8 delivery (Gen 4) is done: `gift/inject4.rs`, the Mystery gifts screen, `data/events4.tsv`
(language/region/games per card; regenerate with `scripts/gen-events4-manifest.py`), research in
`docs/research/events-region.md`. In-game: `tools/emu-harness/collect_gifts4.py`, 21 of 21 on retail
Platinum and HeartGold. `tests/p8_events.rs` covers the rest. Open: D, P and SS have no ROM here, so
they have not been collected in-game. The `ConvertToPKM` port was dropped, since the deliveryman
generates the Pokémon himself. Gen 5 PGF delivery waits on M5.

Unhatched eggs are done: `legality/breed4.rs` ports `MoveBreed4` (move order, expected moves), and
`analysis4` applies PKHeX's egg rules (hatch counter, PP, contest stats, name, gender bit for
Nidoran/Volbeat). Oracle `gen-egg4` builds bred, gift-egg and Ranger eggs; `p5_parity_eggs_1e4`
matches 10,000/10,000. The DS shows verdicts for eggs (`ui-egg.json`, 8 ms on Manaphy Pearl).

Order from here:
1. Remaining M4: Pal Park (Gen 3 origins), D Ranch gifts, egg generation; Ruins of Alph Unown
   legality perf (107 ms on DS, budget 50).
2. P9.1 polish: IV "closest legal" in the editor, nickname keyboard, Met/Misc tabs,
   proportional font, HG vs SS choice in the wizard.
3. M5 Gen 5 (BW/B2W2 legality + generation, then PGF delivery), M7 two-instance harness + P10 Memory Link,
   M8 P11 in-game suite.
