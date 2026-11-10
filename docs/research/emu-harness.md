# Emulator harness (Phase H)

`tools/emu-harness`: a headless C++ program linking the melonDS 1.1 core (tag `1.1`, cloned at
`~/src/melonDS`; no Qt front end, no OpenGL, interpreter only). Script format, step types and
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
