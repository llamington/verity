// SPDX-License-Identifier: GPL-3.0-only
#include "display.h"

#include <string.h>

#include <vector>

namespace display {

namespace {
struct Screen
{
    // Word aligned for word DMA.
    alignas(4) uint16_t text[kTiles * 16];
    alignas(4) uint16_t map[32 * 32];
    alignas(4) uint16_t bg_pal[256];
    alignas(4) uint16_t sprite_pal[256];
    uint16_t* text_vram = nullptr;
    uint16_t* map_vram = nullptr;
    uint32_t dirty_rows = 0;  // bit r: text row r changed
    bool map_dirty = false, bg_pal_dirty = false, sprite_pal_dirty = false;
};

Screen g_screens[kScreens];
std::vector<SpriteGfx*> g_gfx;

uint16_t* bg_pal_vram(int s) { return s ? BG_PALETTE_SUB : BG_PALETTE; }
uint16_t* sprite_pal_vram(int s) { return s ? SPRITE_PALETTE_SUB : SPRITE_PALETTE; }

void copy(void* vram, const void* ram, size_t bytes)
{
    DC_FlushRange(ram, bytes);
    dmaCopyWords(3, ram, vram, bytes);
}
}  // namespace

void init(int s, uint16_t* text_tiles_vram, uint16_t* frame_map_vram)
{
    Screen& sc = g_screens[s];
    sc.text_vram = text_tiles_vram;
    sc.map_vram = frame_map_vram;
    sc.dirty_rows = (1u << kRows) - 1;
    sc.map_dirty = sc.bg_pal_dirty = sc.sprite_pal_dirty = true;
}

uint16_t* text_tiles(int s) { return g_screens[s].text; }
uint32_t* text_dirty(int s) { return &g_screens[s].dirty_rows; }

void touch_text_rows(int s, int row0, int row1)
{
    if (row0 < 0)
        row0 = 0;
    if (row1 > kRows)
        row1 = kRows;
    for (int r = row0; r < row1; r++)
        g_screens[s].dirty_rows |= 1u << r;
}

uint16_t* frame_map(int s) { return g_screens[s].map; }
void touch_frame_map(int s) { g_screens[s].map_dirty = true; }
uint16_t* bg_palette(int s) { return g_screens[s].bg_pal; }
void touch_bg_palette(int s) { g_screens[s].bg_pal_dirty = true; }
uint16_t* sprite_palette(int s) { return g_screens[s].sprite_pal; }
void touch_sprite_palette(int s) { g_screens[s].sprite_pal_dirty = true; }

SpriteGfx* sprite_gfx(OamState* oam, SpriteSize size, SpriteColorFormat format)
{
    uint16_t* vram = oamAllocateGfx(oam, size, format);
    if (!vram)
        return nullptr;
    auto* g = new SpriteGfx;
    g->vram = vram;
    g->halfwords = SPRITE_SIZE_PIXELS(size) / (format == SpriteColorFormat_16Color ? 4 : 2);
    g->ram = new uint16_t[(size_t)g->halfwords]();
    g_gfx.push_back(g);
    return g;
}

void commit()
{
    // OAM and palettes are small and decide what every line shows, so they go first.
    oamUpdate(&oamMain);
    oamUpdate(&oamSub);
    for (int s = 0; s < kScreens; s++)
    {
        Screen& sc = g_screens[s];
        if (sc.bg_pal_dirty)
            copy(bg_pal_vram(s), sc.bg_pal, sizeof(sc.bg_pal));
        if (sc.sprite_pal_dirty)
            copy(sprite_pal_vram(s), sc.sprite_pal, sizeof(sc.sprite_pal));
        if (sc.map_dirty)
            copy(sc.map_vram, sc.map, sizeof(sc.map));
        sc.bg_pal_dirty = sc.sprite_pal_dirty = sc.map_dirty = false;
    }
    for (SpriteGfx* g : g_gfx)
        if (g->dirty)
        {
            copy(g->vram, g->ram, (size_t)g->halfwords * 2);
            g->dirty = false;
        }
    // Text rows top down on both screens together: the display reaches row r of each at once.
    for (int r = 0; r < kRows; r++)
        for (int s = 0; s < kScreens; s++)
        {
            Screen& sc = g_screens[s];
            if (!(sc.dirty_rows & (1u << r)))
                continue;
            copy(sc.text_vram + r * kCols * 16, sc.text + r * kCols * 16, kCols * 32);
        }
    for (Screen& sc : g_screens)
        sc.dirty_rows = 0;
}

void present()
{
    swiWaitForVBlank();
    commit();
}

}  // namespace display
