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
    // A short name for tests (the emulator harness reads it as verity_test_scene).
    virtual const char* name() const = 0;
};

// Push takes ownership. The new scene is entered behind a fade.
void push(Scene* s);
// Runs the stack until it is empty.
void run();

// ---- text ----------------------------------------------------------------------------------

// A screen's text layer: proportional text drawn into its own tiles (the RAM copy in display.h;
// set a row's bit in *dirty when changing it).
struct Surface
{
    uint16_t* tiles = nullptr;
    uint32_t* dirty = nullptr;
    int screen = 0;
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
// Toggle blue and red text on a panel (gender symbols).
#define UI_BLUE "\x02"
#define UI_RED "\x03"

enum class Badge : uint8_t
{
    Legal,
    Fishy,
    Invalid,
    Safe,
};

void frame_clear(Surface* c);
// Back to the backdrop (frame and text) for rows [row0, row1).
void frame_clear_rows(Surface* c, int row0, int row1);
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
// A rounded pill of a colour with its text centred (white or dark ink to suit the colour), for
// types and move categories on a panel; returns the column after it. Up to 15 colours per screen
// between frame_clear calls; past that, pills are white.
int pill(Surface* c, int col, int row, uint16_t color, const char* text);
// A type's colour (the placeholder icons' palette).
uint16_t type_color(int type);
// Text at a pixel position (no clearing), for layouts off the 8 px row grid. Panels, badges and
// pills stay on the grid.
void text(Surface* c, int x, int y, const char* fmt, ...) __attribute__((format(printf, 4, 5)));
// Shortens UTF-8 text in place to fit `width` px, ending it with "..." when cut.
void fit(char* text, int width);
// Text ending at pixel x_end; returns where it starts.
int text_right(Surface* c, int x_end, int y, const char* fmt, ...) __attribute__((format(printf, 4, 5)));
// Text from x that ends before x_end, cut with "..." when longer: for a column with a neighbour
// on its right (draw the neighbour first, e.g. with text_right). The cut is expected, not a fault.
void text_in(Surface* c, int x, int x_end, int y, const char* fmt, ...) __attribute__((format(printf, 5, 6)));

// Spacing rules for text in panels (see docs/research/ui-stack.md, Spacing): plain lines on a
// 12 px pitch (1.5x the 8 px font), lines that carry a badge or pill on the 8 px tile grid 16 px
// apart, and 12 px of space before a new block. Text sits kInsetTop / kInsetBottom px inside a
// panel's content (top-screen panels start at column 1, bottom-screen panels span the screen).
constexpr int kPitch = 12, kGridPitch = 16, kBlockGap = 20;
constexpr int kInsetTop = 16, kInsetBottom = 12;

// A cursor down a panel: line() gives the y for the next plain line, grid_row() the tile row
// for the next line with a badge or pill (draw its text at row * 8), block() starts a new block.
struct Lines
{
    int y;  // top of the next line, px
    bool pending_gap = false;
    // Text starts `inset` px below the panel content's top (4: about the side padding).
    explicit Lines(int row, int inset = 4) : y(row * 8 + inset) {}
    int line();
    int grid_row();
    void block() { pending_gap = true; }
};

// A line of text and badges on tile row `row` of a panel, laid out like a CSS flex row: items
// queued with text(), shrink(), badge() and pill() flow from pixel x; after push_right(), the rest are
// packed against the panel's right edge (margin-left: auto). Badges and pills land on whole cells. A shrink()
// item (last on the left; on the right it is plain text) is cut with "..." to the room left;
// anything else that doesn't fit is a layout fault ("crowded"). draw() lays the row out and draws it.
//   ui::Row r(top(), row, kX);
//   r.badge(Badge::Invalid, "ILLEGAL").push_right().text(UI_MUTED "Game-safe").badge(Badge::Legal, "OK");
//   r.draw();
class Row
{
public:
    Row(Surface* c, int row, int x) : c_(c), row_(row), x_(x) {}
    Row& text(const char* fmt, ...) __attribute__((format(printf, 2, 3)));
    Row& shrink(const char* fmt, ...) __attribute__((format(printf, 2, 3)));
    Row& badge(Badge b, const char* text);
    Row& pill(uint16_t color, const char* text);
    Row& push_right();
    void draw();

private:
    enum Kind : uint8_t
    {
        Text,
        Shrink,
        BadgeItem,
        PillItem
    };
    struct Item
    {
        Kind kind;
        Badge badge;
        uint16_t color;
        bool right;
        char s[64];
    };
    Row& add(Kind kind, Badge b, const char* text);
    Surface* c_;
    int row_, x_;
    bool right_ = false;
    Item items_[6];
    int n_ = 0;
};

// Clears text pixel rows [y0, y1) across the screen.
void clear_px(Surface* c, int y0, int y1);
// Width in pixels of text as drawn.
int text_width(const char* s);
// Draws text from (col, row), word-wrapped to width px, on at most max_rows rows; returns the rows
// used. A word that doesn't fit on the last row ends it with "...". Muted: the label shade.
int wrap(Surface* c, int col, int row, int width, int max_rows, const char* text, bool muted = false);
// The same at a pixel position with a line pitch (no clearing).
int wrap_px(Surface* c, int x, int y, int pitch, int width, int max_rows, const char* text,
            bool muted = false);

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
// Waits for VBlank and shows what has been drawn: for a message before blocking work (the scene
// loop shows each frame by itself).
void present();
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
// A Pokémon's front sprite on the top screen (96x96: 4608 tile bytes in 1D blocks 64x64, 32x64,
// 64x32, 32x32, and 16 colours); null tiles hide it. Drawn over the item icon (a ball behind it).
void pokemon_sprite(int x, int y, const uint8_t* tiles, const uint16_t* colors);
// A 32x16 icon (type or move category: 256 tile bytes, 16 colours) on the top screen in one of
// kSmallIcons slots; null tiles hide the slot.
constexpr int kSmallIcons = 6;
void small_icon(int slot, int x, int y, const uint8_t* tiles, const uint16_t* colors);
// Hides the sprite and the small icons.
void top_art_hide();
// verity's mark (48x48 inside a 64x64 sprite) on the top screen, in the Pokémon sprite's slot; a
// scene change hides it.
void logo(int x, int y);
// A 7x7 shape drawn into the text layer: the six box markings (circle, triangle, square, heart,
// star, diamond), kShiny (the shiny star), kMale or kFemale. Tone 0 ink, 1 muted (an unset
// mark), 2 gold, 3 blue, 4 red. (The ROM fonts' gender signs need a second shade.)
constexpr int kShiny = 6, kMale = 7, kFemale = 8;
void shape(Surface* c, int x, int y, int kind, int tone);
void cursor(int oam_id, int x, int y);
// Highlight bar (8 px high, full width) on the bottom screen.
void bar(int oam_first, int y, bool visible, int height = 8);

}  // namespace ui
