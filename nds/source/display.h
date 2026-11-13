// SPDX-License-Identifier: GPL-3.0-only
// The only code that writes VRAM, palettes and OAM after start-up. Everything else draws into the
// RAM copies here and marks what it changed; commit() copies the changes during VBlank, so a frame
// is never shown half drawn (no tearing). See docs/research/ui-stack.md, Display.
#pragma once

#include <nds.h>
#include <stdint.h>

namespace display {

constexpr int kScreens = 2;  // 0 top (main engine), 1 bottom (sub engine)
constexpr int kRows = 24, kCols = 32;
constexpr int kTiles = kRows * kCols;

// Sets up both screens' text and frame layers and points them at VRAM. `text_tiles` and
// `frame_map` are the VRAM addresses the RAM copies stand in for.
void init(int screen, uint16_t* text_tiles_vram, uint16_t* frame_map_vram);

// RAM copies. Writers mark what they change (rows of the text layer, or the whole map/palette).
uint16_t* text_tiles(int screen);  // kTiles 4 bpp tiles, 16 halfwords each
uint32_t* text_dirty(int screen);  // bit r: text row r changed
void touch_text_rows(int screen, int row0, int row1);
uint16_t* frame_map(int screen);  // 32x32 map entries
void touch_frame_map(int screen);
uint16_t* bg_palette(int screen);  // 256 colours
void touch_bg_palette(int screen);
uint16_t* sprite_palette(int screen);  // 256 colours
void touch_sprite_palette(int screen);

// Sprite graphics with a RAM copy: write `ram`, then touch(). `vram` is what oamSet takes.
struct SpriteGfx
{
    uint16_t* vram = nullptr;
    uint16_t* ram = nullptr;
    int halfwords = 0;
    bool dirty = false;
};
// Allocates VRAM and a RAM copy (never freed, like oamAllocateGfx); null when VRAM is full.
SpriteGfx* sprite_gfx(OamState* oam, SpriteSize size, SpriteColorFormat format);
inline void touch(SpriteGfx* g) { g->dirty = true; }

// Copies everything marked since the last commit, OAM first, then text rows top down (so if it ever
// overran VBlank, the copy would stay ahead of the display). Call right after swiWaitForVBlank().
void commit();
// Waits for VBlank and commits: for showing a message before blocking work.
void present();

}  // namespace display
