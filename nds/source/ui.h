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
// ---- frames (the layer under the text) -----------------------------------------------------
// Scene changes reset both screens to the backdrop. Text on a panel, chip or light badge is drawn
// in dark ink, elsewhere in light ink; "\x01" in a string toggles the muted label shade.

// Toggles the muted label shade inside a string: "Ability" UI_MUTED ... (a macro, so the next
// letter can't be read as part of a hex escape).
#define UI_MUTED "\x01"

enum class Badge : uint8_t
{
    Legal,
    Fishy,
    Invalid,
    Safe,
};

void frame_clear(Surface* c);
// Row 0 as a dark bar with a title (and optional right-aligned text).
void title(Surface* c, const char* fmt, ...) __attribute__((format(printf, 2, 3)));
void title_right(Surface* c, const char* text);
// Row 0 as a tab bar: n equal segments, the active one a cream chip.
void tabs(Surface* c, const char* const* names, int n, int active);
// A 3-cell peach pill on a panel behind a selected key.
void key_highlight(Surface* c, int col, int row);
// Row 23 as a dark bar of button hints: "A:Open|B:Back|L/R:Box".
void hints(Surface* c, const char* spec);
// A cream panel over cells [col, col+w) x [row, row+h) (w, h >= 2); its border is drawn just
// outside them.
void panel(Surface* c, int col, int row, int w, int h);
// A verdict on a panel: a coloured dot, then `text` in the verdict's colour. Returns the first
// column after it.
int badge(Surface* c, int col, int row, Badge b, const char* text);
// A box slot, 5x4 cells (40x32 px) at a cell: style 0 filled, 1 empty, 2 selected. Draw the
// game's icon at kSlotIconX/Y from the slot's corner (its frames draw the Pokemon low).
void slot(Surface* c, int col, int row, int style);
constexpr int kSlotIconX = 4, kSlotIconY = -4;
// Width in pixels of text as drawn.
int text_width(const char* s);
// Draws text from (col, row), word-wrapped to width px, on at most max_rows rows; returns the rows
// used. A word that doesn't fit on the last row ends it with "...".
int wrap(Surface* c, int col, int row, int width, int max_rows, const char* text);

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
// A held item's icon from the ROM (4 bpp 1D 32x32 tiles, its own 16 colours) at x, y on the top
// or bottom screen; one per screen. Null tiles hide it.
void item_icon(bool top, int x, int y, const uint8_t* tiles, const uint16_t* colors);
// Selection cursor (32x32 frame) on the bottom screen.
void cursor(int oam_id, int x, int y);
// Highlight bar (8 px high, full width) on the bottom screen.
void bar(int oam_first, int y, bool visible);

}  // namespace ui
