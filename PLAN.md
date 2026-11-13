# verity — Implementation Plan

A legality-aware Pokémon save editor for Gen 4 and Gen 5 that runs as **Nintendo DS homebrew** (`.nds`), with a portable Rust core.

This document is the hand-off spec. It records decisions already made with the project owner, the architecture, the work broken into phases with acceptance criteria, and the open questions that must be resolved by research rather than assumption.

> **Project name:** `verity` (formerly `pkds`).

---

## 0. Rules for the implementing agent

1. **PKHeX is the source of truth.** For every format, offset, algorithm or legality rule, read the corresponding PKHeX source (https://github.com/kwsch/PKHeX, `PKHeX.Core/`) and cite the file in the commit or PR description. Details in this document marked **(verify)** are from memory and must be checked before use.
2. **Licence is GPLv3.** Porting PKHeX logic and data is allowed and expected. Add GPLv3 headers/LICENSE. Credit PKHeX (kwsch and contributors) and BW_tool (suloku) in the README.
3. **Never commit or download ROMs, BIOS, firmware or user saves.** The owner supplies them locally. Reference them via environment variables (§9). `corpus/` and `roms/` are gitignored.
4. **Never generate anything that risks crashing a game.** There is no raw/hex edit mode (§2).
5. **Automate verification.** The owner wants minimal manual involvement. Every phase ends with automated tests. Prefer the PKHeX oracle (§6) and the emulator harness (§7) over manual checking.
6. **Code style:** minimal comments, only where the code isn't self-explanatory. Do not write comments that narrate changes. Short commit messages; reference the phase/task ID.
7. When this plan and PKHeX disagree, PKHeX wins. Note the discrepancy in `docs/discrepancies.md`.

---

## 1. Goals and non-goals

### Goals
- Runs on DS/DSi/3DS as homebrew (flashcart, TWiLight Menu++/nds-bootstrap, melonDS). Edits `.sav` files on the SD card.
- Supported games: **Diamond, Pearl, Platinum, HeartGold, SoulSilver, Black, White, Black 2, White 2**.
- **Legal mode:** every Pokémon generated or edited in legal mode is reported **Legal by PKHeX's legality checker**.
- **Wild-faithful RNG generation:** roll PID/IVs exactly as the game's encounter method would, including shinies, Synchronize, Cute Charm and Poké Radar chains (Gen 4).
- **Manual IV editing** with Hidden Power awareness. In Gen 4 legal mode, impossible requests resolve to the **closest legal IV spread**, shown as a diff before applying.
- **Add Pokémon** to box or party, legally (encounter-backed) or in game-safe mode.
- **Transfers in legal mode:** Gen 3 → 4 (Pal Park origin) and Gen 4 → 5 (Poké Transfer origin).
- **Mystery Gifts:** inject genuine event gifts, **filtered to the save's region and language**, delivered the way the game expects (§P8).
- **Memory Link (B2W2):** build the B2W2 Memory Link data from a real BW save on the same SD card. Also **Key System** unlocks.
- Polished, game-like two-screen UI, fully usable with buttons or touch.

### Non-goals
- Raw hex editing, or anything producing invalid IDs, Bad Eggs or crash risks.
- Reading or writing retail Slot-1 cartridges, or Slot-2 GBA carts.
- Gen 1–3 and Gen 6+ **save** editing. Gen 3 exists only as a legal *origin* for Pal Park transfers.
- Online server legality (Wiimmfi etc.). The bar is PKHeX.
- Distributing Nintendo art. Optional runtime loading of graphics from the user's own ROM is a stretch goal (§P9).

---

## 2. Decisions already made

| Topic | Decision |
|---|---|
| Platform | NDS homebrew only. Web UI is a possible later front end, not in scope. |
| Core language | **Rust**, `#![no_std]` + `alloc`, compiled for `armv5te-none-eabi` (ARM9) via nightly `-Zbuild-std`. |
| DS front end | **C++ on BlocksDS** by default. An all-Rust UI is an optional spike (P9.0) with explicit go/no-go criteria. |
| Modes | **Legal** (PKHeX says Legal) and **Game-safe** (unchecked but guaranteed valid for the game). No raw mode. |
| Events | Only those distributed in the save's region and language. |
| Impossible manual IVs | Find the closest legal spread, then show the diff before applying. |
| Shinies | Supported in legal mode by constructive RNG inversion, not brute-force search (§P4). |
| Transfers | Included (Pal Park, Poké Transfer). |
| Licence | GPLv3, porting from PKHeX. |
| Existing illegal Pokémon in a save | Flagged in the UI, never modified or blocked. |

---

## 3. Architecture

```
┌──────────────────────────────────────────────────────────────┐
│ verity  (Rust, no_std + alloc, no I/O, no globals)           │
│  rng · text · pkm · save · data · legality · gen · gift      │
│  · memlink · safety                                          │
└───────────────┬───────────────────────────────┬──────────────┘
                │ rlib                          │ rlib
      ┌─────────┴─────────┐           ┌─────────┴───────────┐
      │ verity-ffi        │           │ native test/fuzz    │
      │ staticlib + C ABI │           │ (cargo test, fuzz,  │
      │ cbindgen → .h     │           │  proptest, oracle)  │
      └─────────┬─────────┘           └─────────────────────┘
                │ libverity.a + verity.h
      ┌─────────┴────────────────────────────────────────────┐
      │ nds/  (C++, BlocksDS)                                │
      │ scenes · widgets · input · assets (grit) · NitroFS   │
      │ file I/O · backups · data-blob loading               │
      └──────────────────────────────────────────────────────┘

tools/
  pkhex-oracle/   C# console app on PKHeX.Core (NuGet): legality,
                  round-trips, data export, event DB export
  emu-harness/    C++ headless runner on the melonDS core: boots
                  games, scripted input, multi-instance local link,
                  save dumps, screenshots
  datagen/        Rust: converts oracle exports into compact
                  binary blobs + generated Rust tables
```

### Core rules
- **Byte buffers only.** The front end reads files and passes `&[u8]`/`&mut [u8]` in. Large data tables are loaded by the front end from NitroFS and handed to the core as byte slices. The core never touches files.
- **No floating point** (the DS has no FPU).
- **No `core::fmt` in the DS build path.** Errors are enums or codes. Use `panic = "abort"`, and nightly `-Zbuild-std-features=panic_immediate_abort` for the DS build.
- **Generation feature flags:** `gen3-origin`, `gen4`, `gen5`. The DS build enables all of them, but data blobs are per-generation and loaded lazily.
- **Memory budget:** main RAM is 4MB on DS (16MB on DSi). Target ≤ 1.5MB peak heap for the core in the worst case (Gen 5 save open + Gen 5 encounter tables + legality analysis). Measure it in the emulator harness (§7).

---

## 4. Repository layout

```
verity/
  PLAN.md                      (this file)
  LICENSE                      GPL-3.0
  README.md
  rust-toolchain.toml          pinned nightly + rust-src
  Cargo.toml                   workspace
  crates/
    verity/
      src/
        lib.rs
        rng/       lcrng.rs, lcrng_reversal.rs, arng.rs, mt.rs,
                   lcg64.rs, methods/{gen3,gen4,gen5}.rs
        text/      gen3.rs, gen4.rs, gen5.rs
        pkm/       pk3.rs (origin only), pk4.rs, pk5.rs, crypto.rs,
                   stats.rs, derived.rs (gender/ability/shiny/HP/
                   unown/spinda/wurmple)
        save/      common.rs, crc16.rs, gen4/{dp,pt,hgss}.rs,
                   gen5/{bw,b2w2,crypto}.rs, detect.rs
        data/      blob.rs (zero-copy readers), personal.rs,
                   learnset.rs, encounters.rs, events.rs, ids.rs
        legality/  mod.rs, encounter_match.rs, pidiv.rs, moves.rs,
                   level.rs, ball.rs, items.rs, ribbons.rs,
                   trainer.rs, transfer.rs, misc.rs
        gen/       request.rs, wild.rs, static_.rs, egg.rs,
                   transfer.rs, shiny.rs, iv_search.rs
        gift/      pgt.rs, pcd.rs, pgf.rs, inject.rs
        memlink/   mod.rs, keysystem.rs
        safety/    mod.rs (game-safe validator)
    verity-ffi/
      src/lib.rs   extern "C" API, opaque handles
      cbindgen.toml
    datagen/       build-time tool (std), oracle export → blobs
  data/
    exports/       raw oracle exports (generated, gitignored)
    blobs/         compiled blobs shipped in NitroFS (generated)
  tools/
    pkhex-oracle/  C# .NET (net8.0+) console app
    emu-harness/   C++ (CMake), links melonDS core as submodule
  nds/
    Makefile       BlocksDS; builds verity first via cargo
    source/        C++ front end
    graphics/      original art → grit
    nitrofiles/    data blobs copied here at build time
  corpus/          gitignored: owner-supplied & harness-made saves
  docs/
    discrepancies.md
    memlink-findings.md
    research/      notes per open question
  .github/workflows/ci.yml
```

---

## 5. Toolchain

| Tool | Notes |
|---|---|
| Rust nightly | Pinned in `rust-toolchain.toml` (e.g. `nightly-2026-10-01`) with `components = ["rust-src"]`. |
| DS build of the core | `cargo build -p verity-ffi --release --target armv5te-none-eabi -Zbuild-std=core,alloc -Zbuild-std-features=panic_immediate_abort` |
| Release profile | `opt-level="z"`, `lto=true`, `codegen-units=1`, `panic="abort"` |
| BlocksDS | Latest stable. Uses **picolibc**. The Rust `#[global_allocator]` wraps picolibc `malloc`/`free`. Do **not** enable `compiler-builtins-mem` (it would duplicate `memcpy`). |
| C header | `cbindgen` → `nds/include/verity.h` |
| Oracle | .NET 8+ SDK, `PKHeX.Core` from NuGet. Pin a version and record it. |
| Emulator harness | CMake + the melonDS core (git submodule pinned to a specific commit). |
| Size checks | `cargo bloat`, `arm-none-eabi-size` on the final ELF. |

CI (GitHub Actions or local):
- Native `cargo test` and `clippy`.
- ARM build of `verity-ffi`.
- BlocksDS build of `nds/`.
- Oracle-dependent tests run whenever the corpus is present: locally always, in CI only with synthetic saves the harness generates.

---

## 6. PKHeX oracle (`tools/pkhex-oracle`)

A C# CLI over `PKHeX.Core`. It is the reference implementation for tests and the source of data exports. All commands take and return JSON or binary on stdin/stdout or files.

| Command | Purpose |
|---|---|
| `legality <pkm-file> --game <ver> [--sav <sav>]` | Returns `{ valid, results: [{severity, identifier, comment}] }` from `LegalityAnalysis`. |
| `legality-batch <dir>` | The same, for many files. JSONL output. |
| `sav-info <sav>` | Detected game, region, language, TID/SID/OT, boxes summary. |
| `sav-roundtrip <sav>` | Load and re-export. Used to confirm PKHeX's own byte-stability. |
| `sav-apply <sav> <ops.json>` | Apply a list of edits via PKHeX (set box slot, set item, set flag…). Writes the output save. Used for differential tests against our core. |
| `pkm-decode <pkm>` | Full field dump as JSON. |
| `pidiv <pkm>` | PKHeX's `MethodFinder` result (method type, seed). |
| `export-data --gens 3,4,5 --out data/exports` | Exports personal info, learnsets (level-up, TM/HM, tutors, egg moves), evolution trees, item/move/ability IDs and validity per game, encounter slots/areas/statics/gifts/trades/eggs, Pal Park and Poké Transfer rules, ball permissions, and string tables per language. |
| `export-events --gens 4,5 --out data/exports/events` | Exports PKHeX's mystery gift database (PCD/PGT/PGF) with language/region metadata where available (§P8). |

Notes:
- Exporting via PKHeX.Core's public objects is preferred over parsing PKHeX's internal resource formats. It stays robust across PKHeX versions.
- Every export includes a manifest (PKHeX.Core version, timestamp, counts).

---

## 7. Emulator harness (`tools/emu-harness`)

A headless C++ program that links the melonDS emulation core (not the Qt front end). It is used for:
- the Memory Link research (P10)
- "real game accepts our save" acceptance tests (P11)
- DS-side performance and memory measurement (P9)

### Requirements
- **Instances:** one or two emulated DS instances in a single process, stepped in lockstep. Two instances need melonDS local multiplayer between them. Find the current API at the pinned commit; melonDS ≥1.0 supports multiple instances per process **(verify)**.
- **Determinism:** fixed RTC date/time, fixed firmware user settings, JIT off, fixed RNG seeding where melonDS exposes it.
- **Inputs:** owner-supplied ROMs, saves, **real BIOS7/BIOS9/firmware dumps** (needed for DS Download Play), and a DLDI SD image for running homebrew.
- **Script format** (JSON/YAML), steps such as:
  - `press {keys, frames}`
  - `touch {x, y, frames}`
  - `wait_frames n`
  - `wait_until {screen_hash | ram: {addr, value, mask}} timeout`
  - `screenshot path`
  - `dump_save instance path`
  - `assert_ram …`
- **Outputs:** save dumps, PNG screenshots per step, and a JSON run log with frame numbers.
- **Debug:** optional GDB stub passthrough (melonDS has one) for P10's fallback path.

### Authoring scripts without manual input
The agent finds menu paths itself:
1. Run with frequent screenshots.
2. Inspect the images to see where the game is.
3. Adjust the script.
4. Freeze the script once it reaches the goal.

Use `wait_until` on screen hashes or RAM values rather than fixed frame counts so scripts survive small timing differences.

### Phase H acceptance (prerequisite for P10/P11)
- Boots Platinum and B2W2 from the owner's ROMs to the title screen and the "Continue" screen deterministically. Two runs produce identical screenshots.
- Two-instance local wireless connects: any DS Download Play session works end to end.

---

## 8. Phases

Each phase lists tasks and **acceptance criteria**. Phases P2–P7 can partly overlap once P1 exists.

### P0 — Skeleton and toolchain
- Create the repository layout (§4), LICENSE, README with credits, and the pinned toolchain.
- `verity` exports `verity_crc16_ccitt(data, len) -> u16`.
- `verity-ffi` builds for `armv5te-none-eabi`. A BlocksDS app links it and prints a CRC on screen.
- Global allocator via picolibc, plus a panic handler that calls `extern "C" verity_on_panic()` (the C++ side shows a red error screen and halts).
- **Accept:** `make` at the repo root builds the `.nds`. It boots in melonDS (or the harness) and shows the expected CRC. Native `cargo test` passes.

### P1 — Oracle and corpus
- Implement `pkhex-oracle` with the commands in §6, except the exports, which come in P3.
- Corpus sources (all gitignored under `corpus/`):
  - owner-supplied saves
  - saves the harness creates (new game → first save, for each of the 9 games) once Phase H exists
  - publicly shared research saves the owner downloads manually, if wanted
- **Accept:** `sav-info` and `legality-batch` run on the whole corpus. A script `scripts/oracle-smoke.sh` runs them.

### P2 — Save I/O and PKM codec (Gen 4/5)
Port from PKHeX: `SAV4DP`, `SAV4Pt`, `SAV4HGSS`, `SAV5BW`, `SAV5B2W2`, `PK4`, `PK5`, `PokeCrypto`, `StringConverter4/5`, the Gen 5 block accessors and their checksum tables.

1. **Detection:** identify the game from size and block signatures, the way PKHeX's `SaveUtil` does.
2. **Gen 4 partitions:** two partitions, each with general and storage blocks and footers (save counter, size, CRC16). Pick the active partition the way PKHeX does; write back the same way.
3. **Gen 5 blocks:** the block table and checksum block (BW and B2W2 have different tables; see `SaveBlockAccessor5*`). Handle the encrypted Gen 5 sub-blocks PKHeX handles (e.g. mystery gift).
4. **PKM codec:**
   - checksum-seeded LCRNG encryption of bytes `0x08..0x88`
   - block shuffle by `((PID >> 13) & 0x1F) % 24` **(verify)**
   - party battle-stat region encrypted with a PID seed
   - Gen 4 and Gen 5 differences
5. **Text:** Gen 4 character table (all languages, including Japanese and Korean), and Gen 5 UTF-16 with special-character remaps.
6. **Model:** boxes (names and wallpapers too), party, trainer (OT, TID, SID, gender, language, money, badges), items/bag, Pokédex (seen/caught plus Gen 4/5 form and gender flags), event flags and work values.
7. **Derived values:** stat calculation, level from EXP, gender from PID and ratio, ability slot, nature, shiny, Hidden Power type/power, Unown form, Spinda, Wurmple.
8. **Party writes:** recompute stats when writing to the party.
9. **Backups:** the front end handles them, but the core provides `export()` that rewrites only dirty blocks and checksums.

**Accept:**
- Open → export with no edits gives byte-identical output for 100% of corpus saves.
- Differential edit tests:
  1. Apply N random valid edits (move a Pokémon, change an item, rename) through both our core and `oracle sav-apply`.
  2. Outputs must be byte-identical, or differ only in documented, PKHeX-equivalent ways.
- `pkm-decode` parity for every Pokémon in the corpus.
- `cargo fuzz` targets for save open and PKM decode run for 10 minutes without panics. All parsing returns `Result`; nothing panics on garbage input.

### P3 — Data pipeline
- Implement `oracle export-data`/`export-events`.
- `datagen` converts the exports into **compact little-endian binary blobs** with a fixed header (magic, version, generation, table offsets), readable zero-copy by `verity::data`. Small hot tables (species count, move/item validity bitsets) may be generated as Rust `static`s. Big tables (encounters, learnsets, events) go in blobs.
- Blobs, one per generation and per concern:
  - `g4_personal.bin`, `g4_learn.bin`, `g4_enc_dppt.bin`, `g4_enc_hgss.bin`
  - `g5_personal.bin`, `g5_learn.bin`, `g5_enc_bw.bin`, `g5_enc_b2w2.bin`
  - `g3_origin.bin` (the Gen 3 encounters/learnsets needed for Pal Park legality)
  - `g4_events.bin`, `g5_events.bin`
  - `strings_<lang>.bin`
- **Accept:**
  - Round-trip test: every exported record can be read back by `verity::data` and matches the JSON export.
  - Total blob size per generation is reported. The front end loads only the blobs for the open save's game.

### P4 — RNG library
Port from PKHeX `PKHeX.Core/Legality/RNG/`: `LCRNG`, `LCRNGReversal`, `LCRNGReversalSkip`, `ARNG`, `MethodFinder`, `PIDGenerator`, the Gen 4 chain-shiny and Cute Charm logic, the Gen 5 `LCRNG64` and `MT` handling, and anything else needed.

1. **LCRNG:** `s' = s*0x41C64E6D + 0x6073`, reverse `s = s'*0xEEB9EB65 + 0x0A3561A1`, jump-ahead.
2. **Reversal:** given two consecutive 16-bit outputs (PID halves) or two 15-bit outputs (IV halves), return all seeds. Use PKHeX's constant-time method, not a 2^16 brute force (keep the brute force as a test oracle).
3. **Method implementations (forward):**
   - Gen 3: Methods 1/2/4, H1/H2/H4 (for Pal Park origins)
   - Gen 4: Method 1, Method J (DPPt) and K (HGSS), including Synchronize and Cute Charm variants; Poké Radar chain shiny; event/gift PID methods PKHeX recognises for PGT
   - Gen 5: wild PID rules (ability bit, bit-31 rule, shiny), MT for IVs where relevant **(verify all against PKHeX)**
4. **Backward validation:** given a PKM, recover method and seed exactly like `MethodFinder`.
5. **Constructive generators** (the key performance idea: choose the outcome, then invert):
   - **Gen 4 wild shiny, Method J/K:**
     1. Pick PID low half satisfying gender and ability.
     2. Pick the PID high half from the 8 values making `(TID^SID^hi^lo) < 8`.
     3. Filter by `PID % 25 == nature`.
     4. Invert to the seed and generate the IVs forward.
     5. Walk backward through the nature roll and any rejected PID attempts (plus the Synchronize check) to confirm a valid frame exists.
     6. If not, try the next candidate.
   - **Chain shiny:** forward generation from a random seed; shiny by construction.
   - **Cute Charm:** enumerate the small fixed PID set for the target gender/nature. Check shiny feasibility for the save's TID/SID.
   - **Gen 5 wild:** construct a PID meeting the rules; IVs are independent.
   - **IV-targeted (Gen 4):** invert the IV halves → candidate seeds → walk back to the PID → check constraints.
6. **Closest-legal IV search (Gen 4):**
   - Enumerate IV spreads in ascending distance from the request. Distance = sum of absolute stat differences; tie-break on fewest changed stats.
   - Apply hard constraints: nature, gender, ability, shiny, Hidden Power type if requested, and the encounter's method.
   - Return the first legal match plus a per-stat diff.
   - Cap the work with an iteration budget. If the budget runs out, return the best found and "not exhaustive".
7. **Hidden Power** type/power helpers, and HP-type-constrained searches.

**Accept:**
- Forward/backward property tests: forward then backward recovers the seed, for every method.
- Reversal outputs match the brute-force oracle for 10^6 random inputs.
- Every corpus Pokémon: our method detection equals `oracle pidiv`.
- Constructive generators: 10^4 generated Pokémon per method pass `oracle legality` with zero failures. Includes shiny Method J/K with Synchronize, Cute Charm, chain shiny, and Gen 5 shiny.
- Performance, measured on DS via the harness (P9) once available:
  - a typical legal shiny generation ≤ 100ms
  - a worst-case closest-IV search ≤ 2s, with a progress callback

### P5 — Legality checker
Port the parts of PKHeX's `LegalityAnalysis` that apply to Gen 3-origin, Gen 4 and Gen 5 Pokémon. Order of work:
1. **PID-derived consistency:** gender, ability slot (and Gen 5 hidden ability flag), nature, shiny, Unown form, Spinda, Wurmple evolution, Gen 4 encounter-type byte.
2. **Encounter matching:** wild slots (all methods, including swarms, Poké Radar, Great Marsh/Safari Zone, honey trees and HGSS headbutt trees with TID-dependent tables, dual-slot, roamers), statics, gifts, in-game trades, eggs, events, Pal Park (Gen 3 origin), Poké Transfer (Gen 4 → 5), and Gen 5 Dream World / Dream Radar / hidden grottoes as PKHeX models them.
3. **PID/IV correlation** for the matched encounter (uses P4).
4. **Moves:** current moves learnable via the encounter's initial moves, level-up up to the current level, TM/HM, tutors (Pt, HGSS, B2W2), egg moves (eggs only), event moves, transfer-era moves.
5. **Level/EXP:** met level ≤ current level, EXP within the level's range, EV caps (≤ 255 each, ≤ 510 total).
6. **Ball** rules per encounter.
7. **Held item** validity and wild item tables.
8. **Trainer data:** OT, TID, SID, gender and language consistent with the encounter type (own catch vs trade vs event); nickname rules.
9. **Ribbons, Pokérus, contest stats, HGSS Shiny Leaves, friendship, met/egg dates and locations.**

Output: `LegalityReport { verdict: Legal | Fishy | Invalid, checks: [Check { id, severity, args }] }`. IDs are stable enums; the front end maps them to localized strings.

**Accept:**
- **Verdict parity** with `oracle legality` on:
  - the whole corpus
  - ≥ 10^5 fuzzed Pokémon: random mutations of legal ones, plus random generated requests
- Target 100% parity on verdict. Every mismatch must be fixed or listed in `docs/discrepancies.md` with justification.
- Analysis of one Pokémon on DS ≤ 50ms typical (measured in P9).

### P6 — Generator (legal mode)
- `GenRequest`:
  - game, target (box/party slot), species, form
  - encounter selector (list from data: location × method × slot), or "any legal encounter"
  - level (within the slot's range), nature, gender, ability
  - shiny (none / any / specific type, e.g. square, if relevant)
  - IV request (exact / roll naturally / constraints, e.g. "HP Fire, max Speed")
  - Synchronize/Cute Charm lead, ball (from the allowed set)
  - nickname/OT behaviour (caught-by-save-OT, or traded-in for in-game trades and events)
  - moves (default: the encounter's natural moves; or chosen from the legal set)
  - transfer origin (Gen 3 game for Pal Park, Gen 4 game for Poké Transfer)
- `generate(request) -> Result<Pkm, GenError>`. GenError names the impossible constraint, e.g. `ShinyImpossibleForCuteCharm`, `NatureNotInSlot`.
- `nearest_legal_ivs(pkm, target_ivs) -> IvProposal { ivs, diff, exhaustive }` (Gen 4). For Gen 5, return the target unchanged when legal.
- **Eggs:** legal egg generation and hatching (egg moves, hatch location/date, Gen 4/5 egg PID rules), per PKHeX.
- **Pokédex side-effects:** adding a Pokémon updates seen/caught and form/gender dex flags, as the game would on capture. Implement as an explicit `apply_capture_side_effects`, applied by default.
- **Accept:**
  - Each generator path produces ≥ 10^4 samples (random requests): all `oracle legality` Legal, and all pass the safety validator (P7).
  - Impossible requests return a specific error, never an illegal Pokémon.

### P7 — Game-safe validator
A separate module from legality: "this cannot crash or corrupt the game".
- Species/form IDs exist in that game. Move, item and ability IDs exist in that game. No move duplicates causing issues. PP within bounds.
- Strings terminated, length-bounded, and encodable in the game's charset.
- PKM checksum correct; no Bad Egg flags; stats computed for party slots.
- Level 1–100 and EXP within bounds. Held item valid. No "glitch" Pokémon.
- Save-level checks: box counts, party count 1–6, at least one non-egg party Pokémon, and checksums for every block.
- Game-safe mode allows illegal but valid values (e.g. IVs not matching the PID). Every write in **both** modes goes through this validator. A failing write is rejected with a reason.
- **Accept:**
  - The fuzzer cannot produce a validator-approved PKM/save that the harness (P11) shows crashing or Bad-Egging the game.
  - The validator rejects all known crash classes (curated tests).

### P8 — Mystery gifts
- **Formats:** Gen 4 PGT, PCD (a wonder card containing a PGT; PHC for HGSS Pokéwalker where relevant), Gen 5 PGF. Port PKHeX's `PGT`, `PCD`, `PGF` and the save mystery blocks (`MysteryBlock4`, `MysteryBlock5`, encrypted on Gen 5) **(verify names)**.
- **Event DB:** from `oracle export-events`. Each record: gift bytes, gen, games, language(s), region and distribution metadata.
  - **Research task R3:** determine how to get reliable region/language availability per event. Options: PKHeX's embedded DB fields (card language, games); the EventsGallery repository (https://github.com/projectpokemon/EventsGallery) folder and file naming, which encodes region/language. Document the chosen method in `docs/research/events-region.md`.
- **Filter:** only events matching the save's game, language and region.
- **Delivery approach (preferred):**
  - Inject the gift into the save's pending-gift / wonder-card slots exactly as receiving it via Wi-Fi or local wireless would.
  - Ensure Mystery Gift is unlocked where the game requires it (research **R4**: which unlock flag(s) per game; see PKHeX's `SAV4`/`SAV5` mystery properties).
  - The player collects it in-game (Gen 4 deliveryman in Poké Marts; Gen 5 deliveryman in Pokémon Centers), so **the game sets its own event flags**.
  - Key-item events (Member Card, Oak's Letter, Azure Flute, Secret Key, Enigma Stone, Liberty Pass…) use this same path. Do not set story flags directly unless the research shows injection alone can't work for a specific event; record any such exception in `docs/research/`.
  - Respect "already received" bitflags; refuse duplicates the game wouldn't allow.
- **Event Pokémon generation:** follow each gift's fixed data (OT, TID, SID, ball, ribbons, fateful flag, met data). Where the gift leaves PID/IVs to be generated at receipt, generate them per PKHeX's PGT/PGF rules (shiny locks included).
- **Accept:**
  - For every event in the filtered DB: inject → `oracle sav-info` shows the gift in the right slot. The harness (P11) collects a sample of ≥ 20 gifts across all 9 games in-game, and the received Pokémon/items are `oracle legality` Legal.
  - Duplicate prevention covered by tests.

### P9 — DS application (front end)

#### P9.0 UI stack spike (time-box: 3 days)
1. bindgen/libnds in Rust vs C++, with NFLib and µLibrary (both BlocksDS-compatible).
2. Build: both-screen sprites, touch and buttons, `fopen` of a `.sav` from SD, and a scrolling list widget driven by D-pad and touch.
3. **Decision rule:** use an all-Rust UI only if the spike completes all items within the time-box with no unresolved toolchain blockers. Otherwise use **C++** (default).
4. Write the decision in `docs/research/ui-stack.md`.

#### P9.1 Front-end architecture
- **Scene stack** with push/pop and fade transitions (master brightness). The main screens:
  1. **File picker:** scans common save locations plus a browsable tree, and remembers the last opened file.
  2. **Save overview:** game, OT, play time, legality summary.
  3. **Box view:** 30-slot grid, drag-and-drop, L/R to change box, party panel.
  4. **Pokémon editor:**
     - **Tabs:** Main, Stats/IVs/EVs, Moves, Met/Origin, Ribbons/Misc.
     - **Top screen:** live legality status and details.
  5. **Add Pokémon wizard:** species → mode (Legal/Game-safe) → encounter picker (location/method/level, filtered to legal ones) → options (nature, gender, ability, shiny, IVs/HP type, lead ability, ball) → preview → place.
  6. **IV editor:** spinners plus a numpad. In Gen 4 legal mode, "Find closest legal" shows a diff before applying.
  7. **Mystery Gift browser:** filtered list; card preview on the top screen; inject.
  8. **Memory Link** (B2W2) and **Key System:** pick a BW save on the SD → preview of what will unlock → apply.
  9. **Save/export:** confirm, write a `.bak`, write, verify by re-reading.
- **View-model layer** in C++ over the FFI. The UI never touches save bytes.
- **Input:** every action works with buttons alone and with touch alone.
  - A confirms, B goes back.
  - L/R change tabs or boxes.
  - Start opens the main menu; Select switches mode.
  - Hold-to-repeat with acceleration on spinners.
- **Text:** proportional bitmap fonts at 1× (Latin, Japanese kana/kanji subset, Korean). Never scale text. Ellipsize labels that don't fit. On-screen keyboards only offer characters the game can encode.
- **Look:** original art in a Gen 4/5-inspired style (rounded panels, 2px borders, limited palettes, type-coloured badges), authored at 256×192 1× and converted with grit. No Nintendo assets in the repo.
- **Stretch:** optionally load fonts/frames/icons from the user's own ROM on SD (Gen 4/5 NARCs; locations per the pret decomps and PKHeX). Pokémon box icons and sprites likewise. Without a ROM, fall back to simple original placeholder icons.
- **Safety UX:**
  - "Unsaved changes" indicator.
  - Automatic `<name>.sav.bak` (rotating, keep 3) before any write.
  - Write to a temp file, fsync, rename.
  - Re-open and validate after writing.
  - Never write if the validator fails.
- **Performance:**
  - 60fps UI.
  - Long operations (closest-IV search, full-box legality) run in slices with a progress bar, keeping the UI responsive (cooperative: the core exposes step-wise APIs with a budget).
  - Load data blobs lazily per game, behind fades.

#### P9.2 FFI surface (sketch; refine during implementation)
```c
verity_err verity_save_open(const uint8_t* buf, size_t len, verity_save** out);
void       verity_save_free(verity_save*);
verity_err verity_save_info(const verity_save*, verity_save_info*);
verity_err verity_data_attach(verity_save*, verity_blob_kind, const uint8_t* buf, size_t len);
verity_err verity_slot_get(const verity_save*, verity_loc loc, verity_pkm_view* out);
verity_err verity_slot_set(verity_save*, verity_loc loc, const verity_pkm_edit* edit, verity_mode mode, verity_report* out);
verity_err verity_legality(const verity_save*, verity_loc loc, verity_report* out);
verity_err verity_gen_begin(verity_save*, const verity_gen_request*, verity_job** out);
verity_err verity_job_step(verity_job*, uint32_t budget_us, verity_job_status* out);
verity_err verity_job_result_pkm(verity_job*, verity_pkm_view* out);
verity_err verity_gift_list(const verity_save*, verity_gift_iter**);
verity_err verity_gift_inject(verity_save*, uint32_t gift_id);
verity_err verity_memlink_preview(const verity_save* b2w2, const uint8_t* bw, size_t len, verity_memlink_preview* out);
verity_err verity_memlink_apply(verity_save* b2w2, const uint8_t* bw, size_t len);
verity_err verity_save_export(verity_save*, uint8_t* buf, size_t len);
```
- All structs `#[repr(C)]`, fixed-size arrays for strings (`uint16_t[13]` etc.), no Rust types across the boundary.
- Errors are `enum verity_err`.

**Accept:**
- All screens reachable and usable in the harness with scripted input, for one save per supported game.
- Harness-measured: core heap peak within budget (§3); 60fps in box view; legality per Pokémon ≤ 50ms.

### P10 — Memory Link and Key System (B2W2)

#### Known (from suloku/BW_tool, GPL-3.0; `BW_tool/Editors/MemoryLink.cs`)

The Memory Link region is at B2W2 save offset **`0x7E000`**, size **`0xA20`**:

| Part | Offset | Size | Format |
|---|---|---|---|
| Block 1 | `+0x000` | `0x374` | Encrypted with the Gen 5 save crypto (`SAV5.cryptoArray(data, 0xC, 0x364, 0x370)` as used there; seed `u32` at `+0x370`). CRC16-CCITT over `+0xC..+0x374` stored at `+0x8`. |
| Block 1 mirror | `+0x400` | `0x374` | Same format. BW_tool just copies block 1 here. |
| Block 2 | `+0x800` | `0x2A0` | Unencrypted. CRC16 over `+0xC..+0x220` stored at `+0x8`. |

Block 2 fields:
- trainer name at `0x48`, `0x60`, `0x78` (UTF-16, `0xFFFF`-terminated, 7 chars)
- TID at `0x44`, `0x5C`, `0x74`; SID at `0x46`, `0x5E`, `0x76`
- starter
- 8 flashback flags
- Hall of Fame (`set_hof`, `0x168` bytes)
- "props" at `0x94`, `0x13` bytes

BW_tool's import from a BW save:
| Field | BW save location |
|---|---|
| Name | `0x19404` (`0xF` bytes) |
| TID | `0x19414` |
| SID | `0x19416` |
| Starter | `0x20160` |
| Hall of Fame | `0x23B00` (`0x168`) |
| Props | `0x1F958` (`0x13`) |

It sets a random seed `(rand16 << 16)`. BW_tool's own notes say this unlocks all 8 flashbacks, the Cheren/Bianca battles, NPCs using the BW name, and Pokédex/Subway certificates.

**Verify every offset above against the actual BW_tool source before use**, and port its structures.

#### Gaps to fill (research R1, automated)
1. The mapping from each of the 8 flashback flags to the BW story progress / event flags that earn it.
2. The contents of block 1: what it encodes (likely certificates, Subway trophies, other BW records) and how each is derived from the BW save.
3. What the real game writes to the mirror.
4. PKHeX labels `0x7E000` as `Link1Data` (`0x400`, "3DS link data") in `SAV5B2W2`. Resolve the conflict. If PKHeX is wrong, prepare an upstream issue/PR.

#### Method (automated via §7)
1. **Go/no-go:** a harness script performs a genuine Memory Link: instance A runs B2W2 (fresh or early save), instance B receives via DS Download Play with BW ROM + save in its slot. Confirm the B2W2 save gains data at `0x7E000`.
2. **Base saves:** obtain BW saves at key story points. Prefer harness-driven natural progression where cheap (owner-supplied saves are fine). Otherwise create variants by editing BW event flags with `verity` (BW flags as mapped in PKHeX `EventWork5`).
3. **Differential runs:** change one BW variable per run: each flashback-related flag, name, TID/SID, starter, Hall of Fame, Pokédex completion, Battle Subway records, props source. Link each, dump B2W2, **decrypt block 1**, and diff.
4. Produce `docs/memlink-findings.md`: field-by-field map, derivation rules, and evidence (run IDs and diffs).
5. **Fallback** if Download Play linking fails in melonDS:
   - Find the B2W2 routine writing `0x7E000`: set a write watch via the GDB stub during a legitimate flow (Download Play itself excepted), or analyse statically with Ghidra headless on the ARM9 binary/overlays.
   - Derive the rules from the code.
   - Document which path was used.

#### Implementation
- `verity::memlink::build(bw: &BwSave) -> MemoryLinkData` derives everything from the BW save, never from canned data. It refuses (with a reason) if the BW save hasn't reached the minimum progress, or its region/language doesn't match the B2W2 save.
- `apply(b2w2, data)` writes block 1, the mirror (as the real game does) and block 2, with fresh crypto seeds and correct CRCs.
- **Key System:** port PKHeX `KeySystem5`. Unlock keys only per the rules the game uses for Unova Link sharing (research R5: which keys are shareable vs earned; document them).

**Accept:**
- For ≥ 10 diverse BW saves, our generated region decrypts to the same content as the genuine Memory Link from the harness. Seeds may differ.
- The harness confirms in B2W2: the Memory Link indicator shows, and at least one flashback is triggerable via script.

### P11 — In-game acceptance tests (harness)
- For each of the 9 games, scripted runs:
  1. Boot with a verity-edited save → Continue → open PC → open a box containing generated Pokémon → open a summary → save in-game → dump the save.
  2. Assert: no crash/freeze (frame progress plus a screen-hash watchdog), and no Bad Egg. The re-dumped save opens in `verity` and the oracle, and our Pokémon are still Legal.
- Mystery gift collection runs (P8) and Memory Link runs (P10).
- Run nightly when ROMs are available locally. Store the results in `corpus/reports/`.

### P12 — Web version (proposed, user decision 2026-11-10)

A browser build of the core, after the DS loads its font and graphics from the user's ROM.

- **WASM holds code only.** Per-game data blobs are separate files, fetched for the open save's game
  only, served Brotli-compressed and kept in the browser cache.
- **Text** uses the browser's fonts, with Noto Sans JP/KR loaded only when a name needs them, so the
  web version needs no ROM for Japanese or Korean.
- **Sprites** load at runtime from a third-party host, never bundled, proxied or re-hosted by verity.
  The URL template is configurable and pinned to a commit; the default is the PokeAPI sprites
  repository (`sprites/pokemon/versions/generation-iv/{diamond-pearl,platinum,heartgold-soulsilver}`
  and `generation-v/black-white`) through a CDN. If a sprite fails to load, show the species name and
  a generic icon.
- **Risk:** sprites depend on that host staying up and on its copy of Nintendo's art.
- **Goal (decided 2026-11-10): feature parity with the DS build.** Everything the DS app does, the
  web app does, except what only exists because of DS hardware (ROM font and icon caches, the
  NitroFS/SD file handling, the touch keyboard's charset limits where browser input covers it).
  Parity checklist (DS screens and `verity-ffi` entry points):
  - Open and probe a save, checksums, trainer overview, box and party browsing (done).
  - Summary and legality report per Pokémon, issue text, whole-save check before saving
    (`verity_legality`, `verity_issue_text`, `verity_save_check`) (done).
  - Editor with preview and apply: species, form, level, nickname, OT, IVs/EVs (closest legal IVs),
    nature, ability, moves with descriptions and type/category, held item, ball, met data,
    markings, ribbons with the ribbon check (`verity_edit_*`, `verity_closest_ivs`,
    `verity_move_*`, `verity_items_*`, `verity_ribbons_check`, `verity_location_name`) (done
    except move descriptions and power/accuracy/category, which the DS reads from the ROM).
  - Add Pokémon: wild generation (Gen 4 and Gen 5 wizards), eggs, Pal Park and Poké Transfer
    origins (`verity_gen_insert`, `verity_wild_*`, `verity_egg_*`) (done).
  - Mystery Gift list and delivery, Gen 4 and Gen 5 (`verity_events_list`, `verity_event_inject`)
    (done).
  - Bag/items (Gen 4 and Gen 5) (done 2026-11-11, on both front ends: the DS had no bag screen,
    and the user chose to build one there too). The misc tabs are the editor's Misc tab (done).
  - Memory Link and Key System screen, including "Enable all keys" (`verity_unova_*`) (done
    2026-11-11).
  - Japanese and Korean names (browser fonts), the leave-without-saving warning, backups before
    overwrite (`.bak1` on DS; on the web, keep the original download or a second file handle)
    (done 2026-11-11: Noto Sans JP/KR self-hosted, fetched per unicode-range subset).
  - Save: finalize, verify read-back, write via the File System Access API in Chromium, download
    elsewhere (done).
- **Event data (decided 2026-11-10):** ship the Mystery Gift blobs (`gifts4/5.bin`,
  `events4/5.bin`) as they are, full cards included, fetched only when the gift or legality
  screens need them. The user judged the legal risk acceptable (most likely outcome is a DMCA
  takedown, not a lawsuit); no stripping of card text.
- **Stack (decided 2026-11-10):** SvelteKit 3 (static adapter), strict TypeScript, Tailwind v4,
  shadcn-svelte, Storybook, Vitest, Playwright; `crates/verity-wasm` (wasm-bindgen + tsify) built
  with wasm-pack; hosted on Cloudflare Workers (static assets), deployed from a GitHub Action. Node via mise,
  pinned in `web/.node-version`.

---

## 9. Local configuration

Environment variables (or `pkds.local.toml`, gitignored):

| Variable | Contents |
|---|---|
| `PKDS_ROM_DIR` | `diamond.nds`, `pearl.nds`, `platinum.nds`, `heartgold.nds`, `soulsilver.nds`, `black.nds`, `white.nds`, `black2.nds`, `white2.nds` (owner-dumped) |
| `PKDS_BIOS_DIR` | `bios7.bin`, `bios9.bin`, `firmware.bin` (owner-dumped) |
| `PKDS_CORPUS_DIR` | Defaults to `./corpus` |
| `PKDS_SD_IMAGE` | FAT image for running homebrew in the harness |

Tests needing ROMs or BIOS skip with a clear message when they're absent.

---

## 10. Research items (resolve with evidence, write up in `docs/research/`)

| ID | Question | Method |
|---|---|---|
| R1 | Memory Link gaps (P10) | Harness differential runs; fallback to GDB/Ghidra |
| R2 | Does melonDS (pinned commit) support two in-process instances with DS Download Play using real firmware? | Phase H go/no-go |
| R3 | Per-event region/language availability | PKHeX DB fields vs EventsGallery naming |
| R4 | Mystery Gift unlock flag(s) per game, and the "received" bitfields | PKHeX `SAV4*`/`SAV5*`; confirm in harness |
| R5 | Key System: which keys are shareable vs earned | PKHeX `KeySystem5`, Bulbapedia; confirm in harness |
| R6 | Gen 5 wild PID construction rules that PKHeX validates (ability bit, bit 31, shiny) | PKHeX source |
| R7 | Exact Gen 4 PGT/PCD and Gen 5 PGF PID/IV generation at receipt, and shiny locks | PKHeX source |
| R8 | PKHeX's `0x7E000` "Link1Data" vs BW_tool's Memory Link mapping | R1 results |
| R9 | All-Rust UI viability | P9.0 spike |

---

## 11. Suggested order and milestones

| Milestone | Contents | Demonstrates |
|---|---|---|
| **M1** | P0, P1, P2 (Gen 4 Platinum first, then the other Gen 4 games, then Gen 5), Phase H boot-only | Core reads and writes every save byte-identically; the toolchain is proven on DS |
| **M2** | P3, P4, P5 for **Platinum wild grass encounters**, P6 for the same scope (incl. shinies), P7 | End-to-end legal generation on the hardest Gen 4 RNG path, validated by the oracle |
| **M3** | P9.0 spike + P9.1 minimal (file picker, box view, editor, add-Pokémon wizard for M2 scope) on DS | Usable DS app |
| **M4** | Broaden legality/generation to all Gen 4 encounter types, eggs, HGSS, Pal Park | Gen 4 complete |
| **M5** | Gen 5 (BW, B2W2), Poké Transfer, Dream World/Radar/grotto sources | Gen 5 complete |
| **M6** | P8 mystery gifts | Events |
| **M7** | Phase H full (two instances), P10 Memory Link + Key System | B2W2 extras |
| **M8** | P11 nightly suite, polish, ROM font/graphics loading (`verity-rom`, chosen 2026-11-10) | Release candidate |

P10's research can run in parallel with M3–M5 once Phase H's two-instance support is proven.

---

## 12. References

- **PKHeX:** https://github.com/kwsch/PKHeX (`PKHeX.Core/Saves`, `PKHeX.Core/PKM`, `PKHeX.Core/Legality`, `PKHeX.Core/Legality/RNG`, `PKHeX.Core/MysteryGifts`)
- **PKHeX.Core on NuGet** (oracle)
- **BW_tool** (suloku, GPL-3.0): https://github.com/suloku/BW_tool, `BW_tool/Editors/MemoryLink.cs`
- **PKHeXPluginPile** (unmaintained BW_tool port): https://github.com/foohyfooh/PKHeXPluginPile
- **PKSM / PKSM-Core** (C++ reference, 3DS): https://github.com/FlagBrew/PKSM
- **PokeLib + PPSE-DS** (Gen 4 DS editor reference; **no licence**, read only, do not copy): https://github.com/Chase-san/PokeLib, https://github.com/projectpokemon/PPSE-DS
- **pret decomps** (Gen 4 data/graphics locations): https://github.com/pret
- **EventsGallery:** https://github.com/projectpokemon/EventsGallery
- **melonDS:** https://github.com/melonDS-emu/melonDS
- **BlocksDS:** https://github.com/blocksds; NFLib, µLibrary (BlocksDS ports)
- **Rust DS references:** https://github.com/BlueTheDuck/nds-rs, https://squidtwl.readthedocs.io
- **Memory Link background:** https://bulbapedia.bulbagarden.net/wiki/Memory_Link, https://projectpokemon.org/home/forums/topic/27904-activate-memory-link-features-b2w2/
