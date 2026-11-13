# Emulator harness (Phase H)

`tools/emu-harness`: a headless C++ program linking the melonDS 1.1 core (tag `1.1`; set `MELONDS_DIR` to a
local checkout, or CMake fetches it; no Qt front end, no OpenGL, interpreter only). Script format, step types and
outputs are documented at the top of `src/main.cpp`.

```sh
make emu-harness                       # configure + build (tools/emu-harness/build)
make emu-accept                        # Phase H boot acceptance (skips without roms/ and bios/)
tools/emu-harness/build/emu-harness tools/emu-harness/scripts/ui-boxes.json [--out DIR]
```

## Status (2026-11-01)

| Requirement (PLAN §7) | Status |
|---|---|
| Headless, links the core only | done |
| Determinism: fixed RTC, fixed firmware, JIT off | done; two runs of a script give identical frame counts, screen hashes and PNGs |
| Steps: press/hold, touch, drag, wait, wait_until (screen hash / RAM), assert_ram, screenshot, hash, dump_save | done |
| Outputs: PNGs, save dumps, `run.jsonl` (frame number + screen hashes per step) | done |
| Homebrew with a DLDI SD card synced from a folder | done (`sd_dir`/`sd_image`) |
| Real BIOS7/BIOS9/firmware (`bios_dir`) | done (owner dumps in `bios/`) |
| Retail ROMs; boot Platinum/B2W2 to title and Continue deterministically | **done**: `scripts/boot-pt.json`, `scripts/boot-b2.json` (`make emu-accept`); real saves load (Pt: RoC 74:10, 8 badges; B2 with a W2 save: Castelia City, 8 badges); two runs identical, screen hashes locked in the scripts |
| Two instances with local multiplayer (Download Play) | **done**: `{"instances": [...]}` scripts, `make emu-link` runs a genuine Memory Link |
| GDB stub passthrough | compiled in (`GDBSTUB_ENABLED`), not wired to script options yet |

Speed: about 440 frames/s on this machine for the spike (7x real time).

Owner files (gitignored): `roms/platinum.nds` (CPUE), `roms/black2.nds` (IREO),
`roms/heartgold.nds` (IPKE), `roms/white.nds` (IRAO), `bios/bios7.bin`, `bios/bios9.bin`,
`bios/firmware.bin`.

Exploration step `{"every": N, "frames": M, "prefix": "p"}` screenshots every N frames, for
finding menu paths before freezing a script. `direct_boot` (default true) skips the DS menu.

## Tests: needles, scene waits, save states

`make emu-test` runs the DS UI tests (`EMU_TESTS` in the Makefile, 18 scripts; `emu-ui` is an
alias) and writes `build/emu/tests/junit.xml`; `make emu-test UPDATE=1` rewrites needles that don't
match. Review rewritten needles before committing them. `make emu-font` runs the six scripts that
need ROM fonts and icons (`EMU_FONT_TESTS`), once per stand-in ROM, against gitignored needles in
`needles-local/<rom>` (`--needles`): those screens show Nintendo glyphs and icons. The step reference is at the top of `src/main.cpp`.

- **Needles** (`tools/emu-harness/needles/`, after openQA): a PNG of both screens plus a JSON file
  with tags, match areas and the share of pixels that must match (default 1.0: runs are
  deterministic) and `exclude` rectangles to ignore (the box view's "checked in N ms" timing, so a
  faster or slower core doesn't fail every box screen). `assert_screen` / `wait_screen` take a
  tag. A failure writes the actual screens
  and a diff (differences in red, areas outlined) to the run's output; a tag with no needle gets
  one written, and the step fails until rerun. Needles must not show Nintendo art: the test SD
  card (`sd/`) holds no game ROM, so sprites and ROM fonts never appear.
- **Scene waits**: the DS app publishes the top scene's name in `verity_test_scene` and sets
  `verity_test_busy` during transitions (`nds/source/ui.cpp`). Scripts name them through the ELF
  (`"elf"`): `{"wait_until": {"ram": {"addr": "verity_test_scene", "text": "editor"}}}`. Prefer
  these and `wait_screen` to fixed frame counts.
- **Template taps**: `tap_image` finds a small PNG (`tools/emu-harness/templates/`) on the bottom
  screen and touches its centre, so a script picks "gen4_platinum.sav" by name, not by position.
- **Isolation**: `"sd_fresh": true` gives each run its own copy of `sd_dir`, so scripts can't see
  each other's writes (the picker remembers the last opened save) and the source never changes.
- **Save states**: `save_state` writes the console, cartridge, displayed screens and frame number;
  a script with `"state"` continues exactly where that run was (same frames, same screens). The
  SD card image isn't part of a state.
- **Runs**: several scripts per invocation, exit status 0 / 1 (a step failed) / 2 (error), and
  `--junit FILE`. A failed RAM wait logs the value it last read.

Not converted: `ui-rom-font` (it builds the font caches the font tests copy) and the retail-game
scripts (`boot-*`, `explore-*`, `memlink`, `pt-loads-written`), whose screens can't be committed.
Converted scripts still use fixed `{"wait": N}` inside a scene; prefer scene waits and
`wait_screen` in new steps. Keep a short press away from
heavy frames (switching boxes runs a legality check that takes over a frame): wait for the result
with `wait_screen` first, or the press can fall between two key scans.

## Linked runs

Each console runs its steps on its own thread through melonDS's `LocalMP`, because its receive calls
block on semaphores. PLAN §7's single-threaded lockstep can't work with that. Linked runs aren't
deterministic, so scripts need generous waits (pressing A before the host appears does nothing).
Linked runs raise LocalMP's receive timeout to 150 ms, because 25 ms drops links under load.
An instance more than 8 frames ahead pauses for at most 2 ms. An unbounded pause stalls the link:
the paused console can't reply, and each unanswered command costs the sender a full receive timeout.
A Memory Link takes about 2.5 minutes. `"direct_boot": false` boots to the DS menu.

## Notes

- melonDS DLDI folder sync decides what to copy back by file timestamps. The harness clock
  is fixed, so a file rewritten in a later run keeps the same timestamp and is not synced back.
  Reset the harness image (`sd-harness.img` and `.idx`) before runs that check files written
  on the SD card (`make emu-ui` does this).

- melonDS sets `GDBSTUB_ENABLED` with directory-scoped `add_definitions`; it changes the `NDS`
  and `ARM` class layouts, so the harness must define it too (it crashed in `GPU::GPU` without).
- With FreeBIOS the ROM is direct-booted (`NDS::NeedsDirectBoot`).
- `tools/emu/melon.py` remains for driving the GUI melonDS on the desktop (X11 mode).
