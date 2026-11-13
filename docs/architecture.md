# Architecture

Verity is one Rust core shared by two front ends: a DS homebrew app and a web app. The core
knows saves, Pokémon and PKHeX's legality rules; the front ends only draw screens and move bytes.

```
            PKHeX.Core (NuGet)
                  │  tools/pkhex-oracle export-*
                  ▼
        data/exports/*.json ── crates/datagen ──▶ data/blobs/*.bin
                                                     │ (read at runtime)
                                                     ▼
 nds/ (C++, BlocksDS) ── crates/verity-ffi ──┐   crates/verity   ┌── crates/verity-wasm ── web/ (SvelteKit)
        │                                    └──▶  (no_std core) ◀─┘
        └── crates/verity-rom (fonts, icons, sprites from the user's ROM)
```

## The core (`crates/verity`)

`#![no_std]` with `alloc`, no I/O: every function takes byte buffers. Features `gen4` and `gen5`
select the generations. The main modules:

- `save`: detection of each game's save, partitions, block checksums, slots, trainer, bag, Pokédex,
  event flags. Reads `.sav` and DeSmuME `.dsv`.
- `pkm`: the PK4/PK5 format (encryption, fields, derived stats) and `text` (Gen 4/5 character sets).
- `legality`: ports of PKHeX's analysis, one file per origin generation (`analysis3/4/5.rs`) plus
  encounter matching, moves, evolutions, ribbons, gifts and breeding. `legality::verdict` is the
  entry point both front ends call.
- `rng`: the games' RNGs and PID/IV methods (Method J/K and the reversals used to match them).
- `generate`: builds new legal Pokémon (wild encounters, eggs) the way the games do, and
  `generate::wizard`, the request flow both front ends use.
- `safety`: the game-safe checks (`check_pkm`, `check_save`) and `checked_set_pkm`, the only write
  path into a save. It refuses a write that would crash or corrupt the game and leaves the save
  byte-identical when it does.
- `edit`, `bag`, `gift`, `memlink`: field edits, the bag, Mystery Gift delivery, and the Black/White
  to Black 2/White 2 Memory Link and keys.

Logic shared by both front ends lives in the core, never in a front end.

## Data blobs

The core needs game tables (personal data, learnsets, encounters, Mystery Gift records). They
come from PKHeX: `tools/pkhex-oracle` exports them as JSON, and `crates/datagen` packs them into
compact binary blobs (`data/blobs`, gitignored; `scripts/build-data.sh`). The DS loads them from
NitroFS, the web app fetches the blobs for the open game, and the core's `data` module parses them. `data/events4.tsv` and
`data/events5.tsv` add each event's region and language metadata.

## The oracle (`tools/pkhex-oracle`)

A .NET console app on `PKHeX.Core`. Besides exporting data, it is the reference in tests: it
dumps saves, judges Pokémon, and generates legal (and mutated) Pokémon for the parity tests in
`crates/verity/tests`. A change to legality code is checked against what PKHeX says.

## DS front end (`nds/`, `crates/verity-ffi`, `crates/verity-rom`)

`verity-ffi` exposes the core through a C ABI as an ARM9 static library (`cargo build-ds`), and
`nds/` is the C++ app on BlocksDS that links it. It is the only crate allowed `unsafe`.
`verity-rom` reads the NDS file system, NARCs and compression to pull fonts, box icons, sprites,
item icons and move data out of a Gen 4/5 ROM it finds on the SD card. It caches them under
`/_nds/verity/`, so no Nintendo assets ship with Verity. Saving writes a temporary file, renames
it, keeps rotating `.bak` copies and reads the result back.

`tools/emu-harness` runs the DS build headless on the melonDS core from JSON scripts and compares
screens with reference images (`make emu-test`).

## Web front end (`web/`, `crates/verity-wasm`)

`verity-wasm` wraps the core with wasm-bindgen and tsify, so the TypeScript types are generated
from Rust. It is split into `Editor` (plain Rust, tested natively in `crates/verity-wasm/tests`)
and `Session` (the JavaScript-facing wrapper). The SvelteKit app is static and prerendered: the
landing page at `/`, and the editor under `/app`, which runs only in the browser. Saves are
opened and written with the File System Access API where available (with a read-back check),
and downloaded elsewhere. A service worker makes the app work offline. Sprites are hotlinked from
the PokeAPI sprites repository, pinned to a commit.

## Tests

- Rust unit and integration tests, including parity tests against PKHeX over a corpus of real
  saves and randomized fuzzing through the oracle. Long runs are `#[ignore]`d.
- `fuzz/`: cargo-fuzz targets for PKM decoding and save patching.
- Web: Vitest with Storybook play functions for components, and Playwright for end-to-end journeys.
- DS: the emulator harness.

Tests that need the blobs or the real-save corpus skip when the files are absent.
