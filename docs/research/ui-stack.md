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

Builds with `make` (BlocksDS 1.24.0). **Verified in melonDS 1.1** (2026-11-01) with
`tools/emu/melon.py`: all five items work. Found 42 saves; Down ×3 + A opened the JP HG/SS save
(game, IDs, play time, party, checksums OK; non-ASCII OT prints as `?` in the spike's console);
a double tap opened `junk.sav` ("not a Gen 4/5 save"); a 100 px drag scrolled 12 rows with the
selection clamped on screen.

## Visual loop (tools/emu/melon.py)

melonDS runs under X11 (`flatpak run --nosocket=wayland --env=QT_QPA_PLATFORM=xcb`) so `xwd`
can capture its window and XTest can send keys and touch. `melon.py launch|shot|key|hold|tap|
drag|quit`; keyboard mapping is melonDS's default (set in the melonDS config). Window at 1x,
vertical layout. For scripted, headless and deterministic checks use `tools/emu-harness`
(the spike walk-through was replaced by `scripts/ui-boxes.json` once P9.1 began).

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

## Spacing (2026-11-10)

The user asked for more generous spacing ("start with more space"); the editor was prototyped,
then every screen was converted. The rules (`ui.h`, `Lines`, `spaced_list`):

- **Line spacing:** plain text lines on a 12 px pitch, 1.5x the 8 px font. Every glyph of the ROM
  fonts and the built-in ASCII font fits rows 0-7 (descenders included), so 12 px is the smallest
  whole-pixel pitch that meets WCAG 1.4.8's 1.5 line spacing; 11 px would be 1.375.
- **Grid lines:** panel frames, badges and pills are 8 px tiles, so a line that carries one sits on
  the tile grid, 16 px from the next.
- **Blocks:** 12 px of space before a new block (`Lines::block()`: 20 px top to top), e.g. a
  description after a stats row.
- **Insets:** text starts 4 px below a panel's content top, 8 px in from top-screen panels
  (`kInsetTop`, x 16) and 12 px on full-width bottom panels (`kInsetBottom`).
- **Lists:** long lists on 12 px rows (13 visible in a full panel), menus and editor tabs with up
  to 8 rows on 16 px rows; the highlight is as tall as the pitch and centred on the text.
- **Less on screen:** where a screen no longer fit, content went first. The box view's top screen
  is one panel grouped by space (verdict, identity, moves, IVs, origin); EVs and the PID are in the
  editor only. The editor's top screen has a details panel and one edit panel (mode, verdict,
  status, what's wrong). The file picker's "drag or L/R" line went (the hint bar says it).

Text drawn with `ui::text` doesn't clear; screens clear by pixel rows (`ui::clear_px`). `at()`
remains for the keyboard's key grid, whose keys are tiles. Mixing `at()` with pixel text on the
same rows caused bugs during the conversion (it clears the rest of its 8 px row).

## Display (2026-11-13)

Drawing used to write straight into VRAM, palettes and OAM while the screen was being scanned out,
so on hardware a slow redraw showed as a horizontal tear on the bottom screen. Now only
`nds/source/display.cpp` writes them after start-up. The UI draws into RAM copies (text tiles,
frame maps, palettes, sprite graphics) and marks what changed, by text row for the text layer.
The scene loop calls `display::commit()` right after VBlank: it copies the OAM, palettes, maps and
sprite graphics, then the changed text rows top down, so a frame is shown whole or not at all. A
scene change commits while the screen is black, so it takes no extra frame. Code that draws a
message before blocking work calls `ui::present()` to show it.

Drawing reaches the screen at the next VBlank. Emulator tests that captured a screen a fixed few
frames after input were sometimes capturing half-drawn frames; such checks should use
`wait_screen` (with a timeout) rather than `wait` and `assert_screen`.
