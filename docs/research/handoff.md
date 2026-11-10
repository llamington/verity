# Handoff (2026-11-10)

Start here when resuming. The spec is `PLAN.md`; deviations are in `docs/discrepancies.md`; P5 scope
and gaps are in `docs/research/p5-plan.md`.

## Resume here

**Next tasks (user, 2026-11-10), in order:**

1. **Open question for the user:** the stats hexagon. The games chart EVs (Gen 6 on) and give IVs
   as Judge words ("Best" ... "No Good"). The user asked whether it should show EVs, IVs or both;
   not answered yet. Ask before building it.

**Done 2026-11-10 (all committed and pushed):**

- **Ribbon images:** `verity-rom::ribbons` (`ribbons.vrib` from the best ROM; Gen 5 tables are in
  overlays, read with `Rom::overlay`); the ribbon list shows the highlighted ribbon on the top
  screen. `docs/research/rom-icons.md` (Ribbons). `make emu-font` runs `ui-ribbons.json` too; the
  stand-in ROMs now keep the overlays.
- **Fixed: Gen 5 saves failed their read-back with ROM data loaded** ("verify failed: restore
  from .bak1"; the file itself was written). The read-back allocated a second 512 KB buffer, which
  no longer fit next to the save, the Gen 3-5 data and the ROM caches (move descriptions add about
  100 KB). It now compares in 4 KB chunks. Found by `make emu-tour`, which runs every `ui-*.json`
  with stand-in ROMs; `make emu-ui` runs without ROMs and missed it. DS heap is getting tight:
  watch large allocations.
- **Fixed: "Checksums BAD" on an edited, unsaved save.** Edits leave the block checksums in memory
  stale until `verity_save_finalize` runs on save, and the overview re-probed the buffer. It now
  shows the file's state (`Doc::file_checksums_ok`) and "updated on save" while there are unsaved
  edits. Also fixed: `core::open` kept `dirty` from the previous save. Still open: backing out of
  an edited save with B discards the edits without asking.
- **Mystery Gift list repeats explained and labelled:** repeated titles are different cards (regional
  or shop releases of one event, or a series sharing a title) whose Pokémon differ in OT name and
  trainer ID. Rows now show what the card gives, the OT, the ID and the region; the top panel adds
  OT and ID (`VerityEventView` gained `item`, `tid`, `sid`, `ot`).
- **Move descriptions:** from the ROM's message bank into the move cache (version 2, so old
  caches rebuild). The move panel has six rows (name, stats, four rows of wrapped description);
  the editor's preview panel moved to rows 18-21. `docs/research/rom-text.md`. `make emu-font`
  runs `ui-moves.json` (Gen 4) and `ui-moves5.json` (Gen 5).
- **Move panel look** (user: "visual hierarchy is pretty bad", wanted type colour): name, PP and
  a type pill on row 1; a category pill (Physical red, Special blue, Status grey), Power and Acc
  on row 2; the description in the label shade. `ui::pill` draws a rounded pill on the frame
  layer with its own palette bank (15 colours per screen between `frame_clear` calls; the text
  ink is white or dark by the colour's luma). `at()` clears the rest of its row, so draw pills
  after any text to their left. Pills aren't used in lists yet: the row highlight sprite would
  cover them.
- **Held item icons:** `verity-rom::items` builds `items4.vitm` / `items5.vitm` from the best ROM
  of each generation; the editor shows the icon beside the Main tab's rows and in a top-screen
  panel of the held-item picker. Research and files: `docs/research/rom-icons.md` (Item icons).
  `make emu-font` runs `ui-items.json` and `ui-items5.json`. A Platinum ROM serving a HeartGold
  save has no icons for items 468-536 (Apricorns etc.); they fall back to the note.
- **DS UI restyle** (inspired by the user's Claude Design artifact "Verity DS UI",
  claude.ai/artifact/6wuwLfxbvrMiPk6YUKs1Pg; the user said not to copy its screens). A frame layer
  (BG1) under the text: navy backdrop, dark title and hint bars with key chips, cream panels with a
  rounded border and 2 px padding (panels at least two rows apart), verdict dots, 40x32 box slots.
  Text picks dark or light ink from the frame cell (`ui.cpp` theme block); `UI_MUTED` toggles the
  label shade (a macro so a following hex letter isn't eaten by `\x01`). The text layer is
  scrolled 2 px right, so `plot` clips at x 254. Every scene is converted. Icons sit 4 px right and
  4 px up in slots (the games draw Pokemon low in their 32x32 frames). Touch was re-checked
  (`ui-touch.json`, in `make emu-ui`).
- **Move details:** `verity-rom::moves` reads category, power and accuracy into
  `moves4.vmov` / `moves5.vmov` from the best ROM of each generation (values differ: Tackle 35/95 in
  Gen 4, 50/100 in Gen 5). Type and PP come from verity's data. ROM PP matches PKHeX for every move.
  Files: Pt `poketool/waza/pl_waza_tbl.narc`, D/P `waza_tbl.narc`, HG/SS `a/0/1/1`, Gen 5
  `a/0/2/1`. `make emu-font` runs `ui-moves.json`.
- **Box icons, ROM font, JP/KR keyboard** (below).
- An intermittent `cargo test --workspace` failure was seen twice in 8 runs, then 30 clean runs;
  the failing output wasn't kept. If it recurs, save the log before rerunning.

**Earlier M8 work:** `crates/verity-rom` (used only by `verity-ffi`; the core crate doesn't depend
on it) reads the NDS file system, NARCs, LZ10/11, BLZ (in place) and the fonts; findings in
`docs/research/rom-font.md`. On boot the DS loads its caches from `/_nds/verity/` (`font.vfnt`,
`icons.vicn`, `moves4.vmov`, `moves5.vmov`, `items4.vitm`, `items5.vitm`) or scans the SD card (depth 3) for the best `.nds`
(Gen 5 > Korean Gen 4 > Gen 4) plus the best of each generation, and builds them (2-4 s, reading
each file whole). `make emu-font` checks it with stand-in ROMs holding only what verity reads
(`tools/emu-harness/standin_rom.py`, written under `build/`). Decisions (user): no Nintendo assets
in the repo or a release; the user never picks a ROM; a separate crate rather than a Cargo feature.

The nickname keyboard follows the Pokemon's language (needs the ROM font): Japanese kana pages
with voicing-mark keys, Korean jamo composing into syllables (`verity-ffi/src/keyboard.rs`).

Box icons from the ROM: `docs/research/rom-icons.md`.

Rest of M8: battle sprites (summary screen) are not planned yet; decide whether they're worth it
before P12 (the DS has no screen that shows one today).

After that: **P12 web version** (in PLAN, user decisions 2026-11-10). WASM holds code only; data
blobs are fetched per game; text uses browser fonts; sprites are hotlinked at runtime from the
PokeAPI sprites repo through a CDN, pinned to a commit, with the species name and a generic icon as
the fallback. Web builds don't use `verity-rom`.

Done this session (2026-11-10), all committed and pushed:

- **P10 complete.** In-game flashback check (`make emu-flashback`), Key System exchange under the
  game's rules (`memlink::keys`, `make emu-keys`), the DS screen "Memory Link and keys"
  (`make emu-unova`), and "Enable all keys" without a partner (user request; enabled only, never
  earned; `make emu-keys-all` checks Challenge Mode on a new game's first save). Details:
  `docs/memlink-findings.md`.
- **Validator:** an empty party is accepted only when every box is empty too (a game before its first
  Pokémon). See `docs/discrepancies.md`.
- **P11** (`make emu-p11`, `docs/research/p11.md`): passes on Platinum, HeartGold, White and Black 2.
  The user chose to build on these four; Diamond, Pearl, SoulSilver, Black and White 2 skip for lack
  of ROMs and setup. Nothing schedules it nightly.
- Fixed: the UI scripts' Down counts to "Save to SD card" were stale (Mystery gifts row, then the
  Memory Link row). `make emu-ui` can't detect a missed save by itself; P11 does.

Open items:

- Gen 5 ROM font: "é" (Poké Ball) draws as a blob. The small Gen 5 font's shadow pixels are part
  of the letter shape, and the DS text renderer is 1 bit, so é's shaded counter fills in. A fix
  needs a second text shade for shadow pixels (`VerityGlyph` carries one mask). Found 2026-11-10,
  not fixed.
- Colosseum/XD origins stay "not checked yet" (the user deferred them), in Gen 4 and Gen 5 saves.
- The Gen 5 wizard makes wild Pokémon only. Gen 5 eggs and statics aren't planned in it yet.
- `Test-Save-White-2.sav` can't be saved from the DS: its Moltres (box slot 145) has an impossible
  met location and the whole-save validator refuses it. That is intended.
- On hatched gift eggs that are already illegal, PKHeX can blame different checks after Poké
  Transfer; the verdict is the same. The test accepts this only for the `egg` source.
- Releasing a `.nds` would embed PKHeX-derived data, including Mystery Gift event records. The user
  was advised to get legal advice before publishing builds.
- The Refusal policy (user, 2026-11-09) still applies: match the game where it is sensible, don't copy
  anything silly, and don't add complexity.

Working economically: read narrowly (`grep`, `offset`/`limit`) and cap command output. Most token
use has been whole-file reads of `analysis4.rs`, PKHeX sources and repeated screenshots. For
emulator checks, compare screenshot hashes and view small contact sheets rather than many full
images. The decompilations aren't on disk; read single files with
`gh api -H 'Accept: application/vnd.github.raw' repos/<owner>/<repo>/contents/<path>`.

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
