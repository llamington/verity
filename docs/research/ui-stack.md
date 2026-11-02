# UI stack decision (P9.0)

**Decision: C++ front end over the Rust core's C FFI** (the PLAN default). An all-Rust UI is
rejected under the P9.0 decision rule because the spike hit an unresolved toolchain blocker.

## Rust (bindgen over libnds)

Probe (2026-11-01): `bindgen` 0.70 with libclang 14 over `<nds.h>` (BlocksDS 1.24.0,
`--target=armv5te-none-eabi -DARM9`) generates ~9,000 lines of bindings, but:

- `static inline` functions are not emitted: `videoSetMode`, `videoSetModeSub`, `vramSetBank*`,
  `dmaCopy` and most of the video/VRAM/DMA helpers. Using them from Rust needs bindgen's
  `wrap_static_fns` plus a C shim compiled per build, or hand ports.
- Register macros (`REG_DISPCNT`, `BG_PALETTE`, `SPRITE_PALETTE`, `OAM` ...) are casts of
  addresses, which bindgen does not translate; they would be hand-written volatile constants.
- NFLib, µLibrary and libdsf are C libraries; the same shim problem applies to their inline APIs.

That shim layer is a maintained second copy of libnds's inline API: an unresolved toolchain
blocker, so the rule selects C++. Extern (non-inline) functions do bind (`oamSet`, `scanKeys`,
`touchRead`, `swiWaitForVBlank`), so Rust stays the core and the boundary is the FFI of PLAN P9.2.

## Libraries

Installed with `wf-pacman -S blocksds-nflib blocksds-ulibrary blocksds-libdsf`.

- **µLibrary** draws 2D through the 3D GPU, which renders to one screen per frame (both screens
  need display capture at 30 fps each). Not used: verity needs both screens at 60 fps.
- **NFLib** wraps tiled backgrounds, sprites and text on both 2D engines, loading assets from
  NitroFS in its own formats. Useful later for asset loading; the spike uses libnds directly,
  which covers sprites/backgrounds/OAM with no extra layer.
- **libdsf** renders BMFont fonts into buffers or 3D. Candidate for the proportional fonts of
  P9.1 (needs an original or OFL-licensed font converted to BMFont format).

## Spike (nds/source/main.cpp)

| Item | Status |
|---|---|
| Sprites on both screens | 16-colour OAM sprites on the main and sub engines (procedural original art) |
| Buttons | D-pad with hold-to-repeat, L/R page, A open, Start quit |
| Touch | tap selects, tap on the selection opens, vertical drag scrolls |
| `fopen` of a `.sav` from SD | `fatInitDefault`, scan of `sd:/` and `fat:/` (depth 2) for `.sav`/`.dsv`, read, `verity_save_probe` (Rust) shows game, OT, IDs, play time, party count, checksums |
| Scrolling list | 20 visible rows, D-pad and touch, highlight bar sprite |

Builds with `make` (BlocksDS 1.24.0). Visual verification in melonDS: see below.

### melonDS setup for SD access

DLDI with folder sync, in `~/.var/app/net.kuribo64.melonDS/config/melonDS/melonDS.toml`:

```toml
[DLDI]
Enable = true
ImagePath = "~/verity/sd.img"   # absolute path in the real file
ImageSize = 0
ReadOnly = false
FolderSync = true
FolderPath = "~/verity/sd"
```

`sd/saves` holds copies of the real saves, a non-save `junk.sav` and 30 copies under
`many/` (gitignored).
