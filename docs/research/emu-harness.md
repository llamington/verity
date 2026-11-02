# Emulator harness (Phase H)

`tools/emu-harness`: a headless C++ program linking the melonDS 1.1 core (tag `1.1`, cloned at
`~/src/melonDS`; no Qt front end, no OpenGL, interpreter only). Script format, step types and
outputs are documented at the top of `src/main.cpp`.

```sh
make emu-harness                       # configure + build (tools/emu-harness/build)
tools/emu-harness/build/emu-harness tools/emu-harness/scripts/spike.json [--out DIR]
```

## Status (2026-11-01)

| Requirement (PLAN §7) | Status |
|---|---|
| Headless, links the core only | done |
| Determinism: fixed RTC, fixed firmware, JIT off | done; two runs of `spike.json` give identical frame counts, screen hashes and PNGs |
| Steps: press/hold, touch, drag, wait, wait_until (screen hash / RAM), assert_ram, screenshot, hash, dump_save | done |
| Outputs: PNGs, save dumps, `run.jsonl` (frame number + screen hashes per step) | done |
| Homebrew with a DLDI SD card synced from a folder | done (`sd_dir`/`sd_image`) |
| Real BIOS7/BIOS9/firmware (`bios_dir`) | implemented, untested: no dumps on this machine |
| Retail ROMs; boot Platinum/B2W2 to title and Continue deterministically | **blocked: needs owner-dumped ROMs and BIOS** (PLAN §9 `PKDS_ROM_DIR`, `PKDS_BIOS_DIR`) |
| Two instances with local multiplayer (Download Play) | not started; `MP_*` hooks are stubs |
| GDB stub passthrough | compiled in (`GDBSTUB_ENABLED`), not wired to script options yet |

Speed: about 440 frames/s on this machine for the spike (7x real time).

## Notes

- melonDS sets `GDBSTUB_ENABLED` with directory-scoped `add_definitions`; it changes the `NDS`
  and `ARM` class layouts, so the harness must define it too (it crashed in `GPU::GPU` without).
- With FreeBIOS the ROM is direct-booted (`NDS::NeedsDirectBoot`).
- `tools/emu/melon.py` remains for driving the GUI melonDS on the desktop (X11 mode).
