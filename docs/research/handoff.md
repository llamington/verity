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

## Next steps

1. P6 remainder (later milestones): eggs, statics/gifts/trades, HG/SS, Gen 5, nickname choice,
   held items.
2. M3: P9.0 spike is built (`docs/research/ui-stack.md`: C++ over the FFI; libnds direct,
   NFLib/µLibrary/libdsf installed). Verified headless with `tools/emu-harness` and in the GUI with
   `tools/emu/melon.py`. P9.1 slice 1 done
   (read-only): scene stack with fades (`ui.cpp`), file picker that remembers the last file
   (`/_nds/verity/last.txt`), save overview, box view (6x5 grid, L/R boxes, party panel by
   Select or tap, details on the top screen); blobs from NitroFS (`nds/Makefile` copies
   `data/blobs`); FFI `verity_data_attach`, `verity_box_name`, `verity_slot_view`,
   `verity_name`. Walk-through: `make emu-ui`. Placeholder icons are original type-tinted art;
   text is the 8x8 console font for now. Slice 2: legality verdict on the top screen
   (`verity_legality`, Gen 4 context built once from NitroFS blobs). It judges D/P/Pt wild
   origins only and says "not checked yet" for eggs, hatched, gifts/statics/trades and other
   games instead of guessing; on the real Pt save its 415 verdicts all match PKHeX. 37 ms per
   Pokémon in the harness (release `opt-level = 2`; `"z"` gave 51 ms, over the P9 budget of 50).
   Slice 3: add-Pokémon wizard (X in the box view: species with wild slots → encounter or
   any → nature/shiny/level → generate legal + insert through the P7 validator) and Save to SD
   (finalise, validate, rotate `.bak1..3`, temp file + rename, re-read and compare). FFI
   `verity_wild_species`, `verity_wild_encounters`, `verity_gen_insert`, `verity_save_finalize`,
   `verity_save_check`. `make emu-ui` runs it on a fresh copy of `sd/` in `build/emu/sd`; PKHeX
   confirms the written save (checksums valid, added Pokémon legal, `.bak1` = original), and
   retail Platinum loads it to Continue and into the game (`scripts/pt-loads-written.json`).
   Slice 4: location names (PKHeX Gen 4/5 location banks in the strings blob,
   `verity_location_name`), wizard gender/lead/ball options, and the Pokémon editor (A on a
   slot: Main item/level/friendship, Stats IVs/EVs, Moves with pickers; live game-safe and
   legality preview; Select toggles Legal/Game-safe mode; legal mode refuses illegal results;
   FFI `verity_edit_preview`/`verity_edit_apply`). Walk-throughs: `ui-add.json`, `ui-edit.json`.
   Next slices: IV "closest legal" in the editor, nickname keyboard, Met/Misc tabs, add-Pokémon wizard (P6 API over the FFI), save/export with
   backups, proportional font.
3. Later: widen legality (statics/gifts/trades/eggs, HG/SS Method K, Pal Park, Gen 5), the word
   filter, the emulator harness (Phase H), mystery gifts, Memory Link.
