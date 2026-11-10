# Handoff (2026-11-09)

Start here when resuming. The spec is `PLAN.md`; deviations are in `docs/discrepancies.md`; P5 scope
and gaps are in `docs/research/p5-plan.md`.

## Resume here

Phase H is complete: two linked consoles run through `{"instances": [...]}` scripts, and
`make emu-link` performs a genuine Memory Link (Black 2 host, White receiving over Download Play).
That passes P10's go/no-go: the save changes only in `0x7E000..0x7EA1F`, and two runs give identical
bytes, so differential runs can compare saves directly. `verity::memlink` (`build`/`apply`) matches 17 of 17 genuine links byte for byte: run
`tools/emu-harness/memlink_accept.py`, then `cargo test -p verity --test p10_memlink`. See
`docs/memlink-findings.md`. The `fuddlesworth/pokebw2` decompilation is the reference for field
meanings, read for facts only and proved against genuine links. Next in **P10**: the Key System, the
in-game flashback check and a DS screen, then **M8**: P11 in-game suite.
Read only the PLAN sections for those (`grep -n "P10\|Phase H" PLAN.md`), not the whole file.

Open items, none blocking P10:

- Japanese/Korean text has no font (shows "?"; the keyboard refuses those Pokémon). Needs a freely
  licensed font or ROM font loading; the user hasn't decided.
- Colosseum/XD origins stay "not checked yet" (the user deferred them), in Gen 4 and Gen 5 saves.
- The Gen 5 wizard makes wild Pokémon only. Gen 5 eggs and statics aren't planned in it yet.
- `Test-Save-White-2.sav` can't be saved from the DS: its Moltres (box slot 145) has an impossible
  met location and the whole-save validator refuses it. That is intended.
- On hatched gift eggs that are already illegal, PKHeX can blame different checks after Poké
  Transfer; the verdict is the same. The test accepts this only for the `egg` source.

Working economically: read narrowly (`grep`, `offset`/`limit`) and cap command output. Most token
use has been whole-file reads of `analysis4.rs`, PKHeX sources and repeated screenshots.

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
- Oracle for Gen 5: `gen-origin5 <n> <seed> <dir> <mode>`, `convert5 <in> <out> <seed>` (PKHeX's Poké
  Transfer, then random level-ups and evolutions), `sav-gifts5 <sav>`. Rebuild after editing:
  `DOTNET_ROOT=$HOME/.dotnet dotnet build tools/pkhex-oracle -c Release -v q --nologo`.
- Blobs after changing exports or `data/*.tsv`: `cargo run -q -p datagen -- build data/exports data/blobs`.
- Events: `~/src/EventsGallery` is a sparse checkout; `scripts/gen-events{4,5}-manifest.py` regenerate
  the committed `data/events{4,5}.tsv`.
- Key tests (`crates/verity/tests/p5_fuzz.rs`): `p9_transfer*`, `p9_transfer_1e4` (ignored, 86,086
  Pokémon), `p9_transfer_corpus`, `p6_gen5_wild_legal*`, `p5_parity_*gen5*`; `p8_gen5_gifts.rs`.
- Test gotcha: PKHeX reads a 136-byte file with a met location below 0x4000 and version below 0x10 as
  a PK4, so mutated Poké Transfer files must keep a high met location.
- Harness gotchas: reset `build/emu/sd` before a run (`make emu-ui` does). The DS file picker order
  is gen5_white2 (0), Test-Save-White-2 (1), HeartGold (2), JP HeartGold (3), Black Full (4). The
  box view drops quick presses: use 4-frame presses with 30-frame waits. Saving needs about 240
  frames. `gen5_white2`'s last box is full.
- In-game Gen 5 check (Black 2, `Test-Save-White-2`): the save starts upstairs in a Pokémon Center.
  The deliveryman is the blue-capped man near the entrance; he needs a free party slot.
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
  multiplayer: see Resume here. Owner ROMs (Pt, HG, W, B2) and BIOS are in `roms/`, `bios/`.

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
PKHeX).

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
D Ranch gifts are done: `legality/ranch4.rs` (table generated from PKHeX), oracle `gen-ranch4`,
`p5_parity_ranch_1e4` 10,000/10,000. Fixed along the way: a deferred encounter now follows yielded
statics and trades (only a queued slot ends the search), and `VerifyFixedAbility` for statics.

Legality perf: the slot scan was the cost, not Unown. `EncounterData::slots_at` reads only area
indexes; Ruins of Alph Unown went from 107 ms to 15 ms on the DS.

Egg generation is done in the core: `gen/egg4.rs` (bred eggs, unhatched or hatched, chosen moves put
in game order). `p6_eggs_random_requests_1e4`: 12,582 eggs, all PKHeX-legal. There is no DS screen
for it yet (P9.1).

1. M4 is done apart from Colosseum/XD origins (version 15), deferred by the user on 2026-11-09. They
   stay "not checked yet" on the DS. R/S/E/FR/LG Pal Park is complete (`analysis3.rs`, `gift3.rs`,
   `contest.rs`). If picked up later: port `EncounterGenerator3GC`, `EncounterEnumerator3GC`,
   `TeamLockResult` (shadow locks), `MethodPokeSpot` and `VerifyTrainerID_CXD`.
2. P9.1 is done bar Japanese/Korean text: the UI draws proportional Latin text from libnds' console
   font, so kana, kanji and Hangul still show as "?" and the nickname keyboard is Latin-only (it
   refuses Japanese and Korean Pokémon). That needs a font with a suitable licence, or the
   plan's stretch goal of loading fonts from the user's ROM. The closest-IV search blocks the UI;
   a search that runs out of budget is untimed on the DS.
3. M5 Gen 5 is done. `analysis5.rs` covers Pokémon that originated in B/W and B2/W2: wild slots,
   statics, Dream World, Dream Radar, N, in-game trades, bred eggs and PGF events. Wild generation (`gen/wild5.rs`, the wizard), Wonder Card
   delivery (`gift/inject5.rs`, see `events-region.md`) and Poké Transfer origins are done: `analyze_transfer4`
   reuses the Gen 3/4 matching with `Inputs.fmt5` set and `Evo4::transfer_history`. Colosseum/XD transfers stay
   unchecked, as on Gen 4. On a Gen 5 save the DS loads Gen 3, 4 and 5 data together.
