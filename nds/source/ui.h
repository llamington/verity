// SPDX-License-Identifier: GPL-3.0-only
// Front-end primitives: input, scene stack with fades, text consoles, sprites.
#pragma once

#include <nds.h>
#include <stdint.h>

#include "verity.h"

namespace ui {

// ---- input ---------------------------------------------------------------------------------

struct Input
{
    uint32_t down = 0, held = 0, rep = 0;
    bool touching = false;
    int tx = 0, ty = 0;
    // A completed tap (pen up without dragging), in bottom-screen pixels.
    bool tapped = false;
    int tap_x = 0, tap_y = 0;
    // Vertical drag since pen down, in pixels (valid while touching).
    bool dragging = false;
    int drag_dy = 0;

    void poll();

private:
    int start_y_ = -1, last_x_ = 0, last_y_ = 0;
};

// ---- scenes --------------------------------------------------------------------------------

class Scene
{
public:
    virtual ~Scene() = default;
    // Called when the scene becomes the top of the stack (first time or after a pop).
    virtual void enter() = 0;
    // One frame. Returns false to pop this scene.
    virtual bool update(const Input& in) = 0;
};

// Push takes ownership. The new scene is entered behind a fade.
void push(Scene* s);
// Runs the stack until it is empty.
void run();

// ---- text ----------------------------------------------------------------------------------

// A screen's text layer: proportional text drawn into its own tiles.
struct Surface
{
    uint16_t* tiles = nullptr;
};

Surface* top();
Surface* bottom();
void clear(Surface* c);
// printf at the 8x8 cell (col, row); clears to the end of the line. Runs of two or more spaces
// are table gaps and line the next text up with the cell grid.
void at(Surface* c, int col, int row, const char* fmt, ...) __attribute__((format(printf, 4, 5)));
// UTF-16 to drawable UTF-8: ASCII as is; other characters as UTF-8 when the ROM font has them,
// otherwise common accented letters folded and the rest '?'. Cut short rather than split one.
void utf16_to_text(const uint16_t* s, char* out, int cap);
// The font cache built from the user's ROM (stays owned by the caller), or null for none.
void set_rom_font(const uint8_t* cache, size_t len);
bool has_rom_font();
// Whether c can be drawn (ASCII, or in the ROM font).
bool has_glyph(uint16_t c);
// Characters (not bytes) in UTF-8 text, for centring.
int text_len(const char* s);
// A name from the strings blob as ASCII ("?" when unknown).
const char* name(uint32_t category, uint32_t id);
// A location name as ASCII ("Loc <id>" when unknown).
const char* location(uint8_t generation, uint16_t id);

// ---- sprites -------------------------------------------------------------------------------

void sprites_init();
void sprites_hide_all();
void sprites_commit();
// Placeholder Pokémon icon (original art), coloured by type; egg and shiny variants.
void icon(int oam_id, int x, int y, uint8_t type, bool egg, bool shiny);
// ROM icon palettes (16 BGR555 colours each, at most 4).
void set_icon_palettes(const uint16_t (*pal)[16], int count);
// A box icon from the ROM: 4 bpp tiles (1D 32x32) drawn with ROM palette `palette`; shiny star.
void icon_rom(int oam_id, int x, int y, const uint8_t* tiles, uint8_t palette, bool shiny);
// Selection cursor (32x32 frame) on the bottom screen.
void cursor(int oam_id, int x, int y);
// Highlight bar (8 px high, full width) on the bottom screen.
void bar(int oam_first, int y, bool visible);

}  // namespace ui
