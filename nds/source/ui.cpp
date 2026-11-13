// SPDX-License-Identifier: GPL-3.0-only
#include "ui.h"
#include "display.h"
#include "logo.h"

#include <stdarg.h>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <vector>

namespace ui {

// ---- input ---------------------------------------------------------------------------------

void Input::poll()
{
    scanKeys();
    down = keysDown();
    held = keysHeld();
    rep = keysDownRepeat();
    tapped = false;
    touching = held & KEY_TOUCH;
    if (touching)
    {
        touchPosition tp;
        touchRead(&tp);
        tx = tp.px;
        ty = tp.py;
        if (down & KEY_TOUCH)
        {
            start_y_ = ty;
            dragging = false;
        }
        if (start_y_ >= 0 && abs(ty - start_y_) > 6)
            dragging = true;
        drag_dy = start_y_ >= 0 ? ty - start_y_ : 0;
        last_x_ = tx;
        last_y_ = ty;
    }
    else if (start_y_ >= 0)
    {
        if (!dragging)
        {
            tapped = true;
            tap_x = last_x_;
            tap_y = last_y_;
        }
        start_y_ = -1;
        dragging = false;
        drag_dy = 0;
    }
}

// ---- scenes --------------------------------------------------------------------------------

// Read by the emulator harness by symbol (tools/emu-harness), so scripts wait for a scene instead
// of counting frames: the top scene's name, and 1 while scenes change (fades, enter()).
extern "C" {
volatile char verity_test_scene[16] = "";
volatile u32 verity_test_busy = 1;
}

namespace {
std::vector<Scene*> g_stack;
Scene* g_pending = nullptr;
Surface g_top, g_bottom;

void fade(bool out)
{
    for (int i = 0; i <= 16; i += 2)
    {
        setBrightness(3, out ? -i : -(16 - i));
        swiWaitForVBlank();
    }
}
}  // namespace

namespace {
void publish_top()
{
    const char* n = g_stack.empty() ? "" : g_stack.back()->name();
    size_t i = 0;
    for (; n[i] && i + 1 < sizeof(verity_test_scene); i++)
        verity_test_scene[i] = n[i];
    verity_test_scene[i] = 0;
}
}  // namespace

// Shows a scene's first frame. The screens go black first (the fade in starts there), so the copy
// can't be seen mid-frame and needs no VBlank wait: the change takes no extra frame.
static void show_entered()
{
    setBrightness(3, -16);
    display::commit();
}

void push(Scene* s) { g_pending = s; }

void run()
{
    Input in;
    while (true)
    {
        if (g_pending)
        {
            verity_test_busy = 1;
            if (!g_stack.empty())
                fade(true);
            g_stack.push_back(g_pending);
            g_pending = nullptr;
            sprites_hide_all();
            frame_clear(top());
            frame_clear(bottom());
            g_stack.back()->enter();
            show_entered();
            fade(false);
            // The fades don't scan keys, so the last scan may still hold the key that started the
            // change; scan now, or a quick press right after the change would be no new press.
            scanKeys();
            publish_top();
            verity_test_busy = 0;
        }
        if (g_stack.empty())
            break;
        // Show the last frame's drawing while the display is between frames, then draw the next.
        swiWaitForVBlank();
        display::commit();
        in.poll();
        if (!g_stack.back()->update(in))
        {
            verity_test_busy = 1;
            fade(true);
            delete g_stack.back();
            g_stack.pop_back();
            if (g_stack.empty())
                break;
            sprites_hide_all();
            frame_clear(top());
            frame_clear(bottom());
            g_stack.back()->enter();
            show_entered();
            fade(false);
            // The fades don't scan keys, so the last scan may still hold the key that started the
            // change; scan now, or a quick press right after the change would be no new press.
            scanKeys();
            publish_top();
            verity_test_busy = 0;
            continue;
        }
    }
}

void present() { display::present(); }

// ---- text ----------------------------------------------------------------------------------

// Proportional 8-pixel-high text. Each screen is a 32x24 grid of its own tiles, so text is drawn
// as pixels. Glyphs come from libnds' console font, trimmed to their inked columns.
namespace {
constexpr int kTiles = 32 * 24;
constexpr int kSpaceWidth = 4;
int g_digit_width = 6;  // digits share one width so numbers line up

const uint8_t* g_font;
int g_font_first, g_font_count;
uint8_t g_left[128];  // inked start column per ASCII code
// Hot text state lives in DTCM (fast data memory): widths and glyph rows already shifted to
// their first inked column, per ASCII code.
DTCM_DATA uint8_t g_width[128];
DTCM_DATA uint8_t g_rows[128][8];

void measure_font()
{
    const PrintConsole* d = consoleGetDefault();
    g_font = (const uint8_t*)d->font.gfx;
    g_font_first = d->font.asciiOffset;
    g_font_count = d->font.numChars;
    for (int c = 0; c < 128; c++)
    {
        int ix = c - g_font_first;
        g_left[c] = 0;
        g_width[c] = 0;
        if (ix < 0 || ix >= g_font_count)
            continue;
        uint8_t mask = 0;
        for (int y = 0; y < 8; y++)
            mask |= g_font[ix * 8 + y];
        int lo = 8, hi = -1;
        for (int x = 0; x < 8; x++)
            if (mask & (1 << x))
            {
                lo = lo < x ? lo : x;
                hi = x;
            }
        if (hi >= 0)
        {
            g_left[c] = (uint8_t)lo;
            g_width[c] = (uint8_t)(hi - lo + 1);
        }
    }
    for (int c = '0'; c <= '9'; c++)
        if (g_width[c] + 2 > g_digit_width)
            g_digit_width = g_width[c] + 2;
    for (int c = 0; c < 128; c++)
    {
        int ix = c - g_font_first;
        for (int y = 0; y < 8; y++)
            g_rows[c][y] = ix < 0 || ix >= g_font_count
                               ? 0
                               : (uint8_t)((g_font[ix * 8 + y] >> g_left[c]) & ((1u << g_width[c]) - 1));
    }
}

DTCM_DATA uint8_t g_ink = 1;  // palette index plot() writes (see the theme below)

// Where plot() may draw, in text-layer pixels (the layer is scrolled 2 px right, so its last 2
// columns would wrap to the left edge). While a text draw tracks, plot() notes pixels it had to
// drop and ink drawn over ink.
struct Clip
{
    int x0, y0, x1, y1;
};
constexpr Clip kScreen = {0, 0, 254, 192};
DTCM_DATA Clip g_clip = kScreen;
DTCM_DATA bool g_track = false;
DTCM_DATA bool g_clipped = false;
DTCM_DATA bool g_overlap = false;

ARM_CODE ITCM_CODE void plot(Surface* c, int x, int y)
{
    if (x < g_clip.x0 || x >= g_clip.x1 || y < g_clip.y0 || y >= g_clip.y1)
    {
        g_clipped = true;
        return;
    }
    int tile = (y / 8) * 32 + x / 8;
    *c->dirty |= 1u << (y / 8);
    uint16_t* word = c->tiles + tile * 16 + (y % 8) * 2 + (x % 8) / 4;
    int shift = ((x % 8) % 4) * 4;
    if (g_track && ((*word >> shift) & 0xF))
        g_overlap = true;
    *word = (uint16_t)((*word & ~(0xF << shift)) | (g_ink << shift));
}

// Draws one row of a glyph: bit i of `bits` is the pixel at x + i. The same as plot() for each set
// bit, but each tile's row (8 pixels, one 32-bit word) is read and written once.
ARM_CODE ITCM_CODE void blit_row(Surface* c, int x, int y, uint32_t bits)
{
    const Clip clip = g_clip;
    if (y < clip.y0 || y >= clip.y1)
    {
        g_clipped = true;
        return;
    }
    // Drop the pixels left and right of the clip.
    if (x < clip.x0)
    {
        int n = clip.x0 - x;
        uint32_t keep = n >= 32 ? 0 : ~((1u << n) - 1);
        g_clipped = g_clipped || (bits & ~keep);
        bits &= keep;
    }
    if (clip.x1 - x < 32)
    {
        int n = clip.x1 - x;  // pixels that fit
        uint32_t keep = n <= 0 ? 0 : (1u << n) - 1;
        g_clipped = g_clipped || (bits & ~keep);
        bits &= keep;
    }
    if (!bits)
        return;
    *c->dirty |= 1u << (y / 8);
    uint16_t* row = c->tiles + (y / 8) * 32 * 16 + (y % 8) * 2;
    const uint32_t ink = g_ink & 0xF;
    bool overlap = false;
    // One tile at a time: shift the row so bit 0 is the tile's first pixel.
    int tile = x >> 3;
    uint64_t rest = (uint64_t)bits << (x & 7);
    for (; rest; rest >>= 8, tile++)
    {
        uint32_t span = (uint32_t)rest & 0xFF;
        if (!span)
            continue;
        uint32_t mask = 0;
        for (int p = 0; p < 8; p++)
            if (span & (1u << p))
                mask |= 0xFu << (p * 4);
        uint16_t* w = row + tile * 16;
        uint32_t old = (uint32_t)w[0] | (uint32_t)w[1] << 16;
        overlap = overlap || (old & mask);
        old = (old & ~mask) | (mask & (ink * 0x11111111u));
        w[0] = (uint16_t)old;
        w[1] = (uint16_t)(old >> 16);
    }
    if (g_track && overlap)
        g_overlap = true;
}

const uint8_t* g_rom_font = nullptr;  // font cache from the user's ROM (verity_font_*)
size_t g_rom_font_len = 0;

bool rom_glyph(uint16_t cp, verity_glyph* g)
{
    return g_rom_font && verity_font_glyph(g_rom_font, g_rom_font_len, cp, g);
}

// Draws one glyph (or only measures it when c is null); returns the x after it, with one pixel of
// spacing. ASCII comes from the console font, anything else from the ROM font when there is one,
// otherwise '?'.
ARM_CODE ITCM_CODE int glyph(Surface* c, int x, int y, uint32_t cp)
{
    if (cp >= 128)
    {
        verity_glyph g;
        if (cp > 0xFFFF || !rom_glyph((uint16_t)cp, &g))
            cp = '?';
        else
        {
            for (int row = 0; c && row < g.height && row < 16; row++)
                blit_row(c, x, y + row, g.rows[row] & ((1u << g.width) - 1));
            return x + (g.width ? g.width + 1 : kSpaceWidth);
        }
    }
    unsigned char ch = (unsigned char)cp;
    int ix = ch - g_font_first;
    if (ix < 0 || ix >= g_font_count)
        return x + kSpaceWidth;
    int digit_pad = (ch >= '0' && ch <= '9') ? (g_digit_width - 1 - g_width[ch]) / 2 : 0;
    for (int row = 0; c && row < 8; row++)
        blit_row(c, x + digit_pad, y + row, g_rows[ch][row]);
    if (ch >= '0' && ch <= '9')
        return x + g_digit_width;
    return x + g_width[ch] + 1;
}

// Decodes one UTF-8 sequence at s[*i] and advances *i; malformed or cut-off bytes give '?'.
uint32_t next_cp(const char* s, int* i)
{
    unsigned char b = (unsigned char)s[*i];
    int n = b < 0x80 ? 0 : (b & 0xE0) == 0xC0 ? 1 : (b & 0xF0) == 0xE0 ? 2 : -1;
    (*i)++;
    if (n < 0)
        return '?';
    uint32_t cp = n == 0 ? b : b & (n == 1 ? 0x1F : 0x0F);
    for (int k = 0; k < n; k++)
    {
        unsigned char t = (unsigned char)s[*i];
        if ((t & 0xC0) != 0x80)
            return '?';
        cp = (cp << 6) | (t & 0x3F);
        (*i)++;
    }
    return cp;
}

// ---- theme ---------------------------------------------------------------------------------
// Each screen has a frame layer (BG1) under the text (BG0): backdrop, dark bars, cream panels with
// a dark rounded border, badges, key chips and box slots, all from tiles generated here. Text picks
// its ink from the frame cell it starts in (dark on cream, light on navy); "\x01" in a string
// toggles the muted shade for labels.

// Palette (BG, both layers): 0 navy backdrop (transparent), then:
enum : uint8_t
{
    C_CREAM = 1,
    C_INK,
    C_MUTED,    // labels on cream
    C_MUTED_L,  // labels on navy
    C_LINE,     // backdrop scanline
    C_SHADOW,   // panel bottom inset
    C_PEACH,
    C_ORANGE,
    C_LEGAL,
    C_FISHY,
    C_INVALID,
    C_EMPTY,
    C_EMPTY_EDGE,
    C_SAFE,
    C_WHITE,
};
const uint16_t kTheme[16] = {
    RGB15(6, 8, 13),   RGB15(31, 30, 29), RGB15(4, 4, 7),    RGB15(11, 12, 15),
    RGB15(25, 25, 27), RGB15(8, 10, 16),  RGB15(27, 27, 24), RGB15(31, 28, 23),
    RGB15(30, 17, 5),  RGB15(5, 13, 24),  RGB15(30, 22, 7),  RGB15(23, 6, 5),
    RGB15(9, 11, 17),  RGB15(5, 6, 10),   RGB15(22, 10, 3),  RGB15(31, 31, 31),
};

enum Kind : uint8_t
{
    K_BACK,
    K_BAR,
    K_PANEL,
    K_CHIP,
    K_SLOT,
    K_PILL_DARK,   // a coloured pill under dark text
    K_PILL_LIGHT,  // a coloured pill under white text
};
// Tile numbers in the frame layer.
enum : uint16_t
{
    T_BACK = 1,
    T_BAR,
    T_PANEL = 3,      // 25: the 5x5 slices of a canonical panel (ring, corners, edges, middle)
    T_SLOT = 28,      // 3 styles x 20 (5x4 cells, 40x32 px)
    T_CHIP = 88,      // left, middle, right, single
    T_BADGE = 92,     // 4 verdict dots (stride 3)
    T_BETWEEN = 104,  // 5: one panel's bottom ring over the next one's top ring, in one row
    T_KEYSEL = 109,   // 3: a selected key on a panel (peach pill, orange edge)
    T_PILL = 112,     // 4: left, middle, right, single; fill 15, edge 13 (set per palette bank)
    T_COUNT = 116,
};

uint16_t* g_fmap[2];
uint8_t g_kind[2][kTiles];
DTCM_DATA bool g_muted = false;
DTCM_DATA uint8_t g_tint = 0;  // a text colour on panels (UI_BLUE, UI_RED), 0 for none

int screen_of(Surface* c) { return c == &g_bottom ? 1 : 0; }

ARM_CODE ITCM_CODE uint8_t ink_for(Surface* c, int x, int y)
{
    int col = x / 8, row = y / 8;
    uint8_t k = (col >= 0 && col < 32 && row >= 0 && row < 24) ? g_kind[screen_of(c)][row * 32 + col]
                                                               : K_BACK;
    if (k == K_PILL_DARK)
        return C_INK;
    if (k == K_PILL_LIGHT)
        return C_WHITE;
    bool dark = k == K_PANEL || k == K_CHIP || k == K_SLOT;
    if (g_tint && dark)
        return g_tint;
    if (g_muted)
        return dark ? C_MUTED : C_MUTED_L;
    return dark ? C_INK : C_CREAM;
}

// Pixel colour of a rounded rectangle [x0,x1)x[y0,y1) with a border `edge` px wide and corner
// radius r, or -1 outside it.
int rounded(int px, int py, int x0, int y0, int x1, int y1, int r, int edge, int fill, int border)
{
    auto inside = [](int px, int py, int x0, int y0, int x1, int y1, int r) {
        if (px < x0 || px >= x1 || py < y0 || py >= y1)
            return false;
        int cx = px < x0 + r ? x0 + r : px >= x1 - r ? x1 - r - 1 : px;
        int cy = py < y0 + r ? y0 + r : py >= y1 - r ? y1 - r - 1 : py;
        int dx = px - cx, dy = py - cy;
        return dx * dx + dy * dy <= r * r;
    };
    if (!inside(px, py, x0, y0, x1, y1, r))
        return -1;
    int ri = r - edge > 0 ? r - edge : 0;
    return inside(px, py, x0 + edge, y0 + edge, x1 - edge, y1 - edge, ri) ? fill : border;
}

void put_tile(uint16_t* tiles, int t, const uint8_t px[64])
{
    for (int y = 0; y < 8; y++)
        for (int h = 0; h < 2; h++)
        {
            const uint8_t* p = px + y * 8 + h * 4;
            tiles[t * 16 + y * 2 + h] = (uint16_t)(p[0] | p[1] << 4 | p[2] << 8 | p[3] << 12);
        }
}

uint8_t backdrop(int y) { return y % 8 == 0 ? C_LINE : 0; }

void make_frame_tiles(uint16_t* tiles)
{
    uint8_t px[64];
    auto solid = [&](int t, uint8_t c) {
        memset(px, c, 64);
        put_tile(tiles, t, px);
    };
    memset(tiles, 0, 32);
    for (int i = 0; i < 64; i++)
        px[i] = backdrop(i / 8);
    put_tile(tiles, T_BACK, px);
    solid(T_BAR, C_INK);
    // Panel: content cells 1..3 of a 5x5 grid. The ring cells around them hold 2 px of cream
    // padding, then the 2 px border (so text never touches it), with an inset shadow at the bottom.
    auto panel_px = [](int X, int Y) -> uint8_t {
        int v = rounded(X, Y, 4, 4, 36, 36, 6, 2, C_CREAM, C_INK);
        if (v == C_CREAM && Y >= 32)
            v = C_SHADOW;
        return v < 0 ? backdrop(Y) : (uint8_t)v;
    };
    for (int ty = 0; ty < 5; ty++)
        for (int tx = 0; tx < 5; tx++)
        {
            for (int y = 0; y < 8; y++)
                for (int x = 0; x < 8; x++)
                    px[y * 8 + x] = panel_px(tx * 8 + x, ty * 8 + y);
            put_tile(tiles, T_PANEL + ty * 5 + tx, px);
        }
    // Between two panels one row apart: the bottom ring (its upper half) over the top ring (its
    // lower half).
    for (int tx = 0; tx < 5; tx++)
    {
        for (int y = 0; y < 8; y++)
            for (int x = 0; x < 8; x++)
                px[y * 8 + x] = y < 4 ? panel_px(tx * 8 + x, 32 + y) : panel_px(tx * 8 + x, y);
        put_tile(tiles, T_BETWEEN + tx, px);
    }
    // Slots (40x32, 4 px apart): filled, empty, selected. Icon frames are 32x32 with the
    // Pokemon drawn low, so the slot is wider than the frame and the icon sits 4 px up in it.
    const uint8_t fill[3] = {C_CREAM, C_EMPTY, C_PEACH}, edge[3] = {C_INK, C_EMPTY_EDGE, C_ORANGE};
    for (int st = 0; st < 3; st++)
        for (int t = 0; t < 20; t++)
        {
            for (int y = 0; y < 8; y++)
                for (int x = 0; x < 8; x++)
                {
                    int X = (t % 5) * 8 + x, Y = (t / 5) * 8 + y;
                    int v = rounded(X, Y, 2, 0, 38, 32, 6, st == 2 ? 3 : 2, fill[st], edge[st]);
                    px[y * 8 + x] = v < 0 ? backdrop(y) : (uint8_t)v;
                }
            put_tile(tiles, T_SLOT + st * 20 + t, px);
        }
    // Key chips on a dark bar: a 24 px pill (left, middle, right) and an 8 px round one.
    for (int t = 0; t < 4; t++)
    {
        for (int y = 0; y < 8; y++)
            for (int x = 0; x < 8; x++)
            {
                int v = t < 3 ? rounded(t * 8 + x, y, 0, 0, 24, 8, 3, 0, C_CREAM, C_CREAM)
                              : rounded(x, y, 0, 0, 8, 8, 3, 0, C_CREAM, C_CREAM);
                px[y * 8 + x] = v < 0 ? C_INK : (uint8_t)v;
            }
        put_tile(tiles, T_CHIP + t, px);
    }
    // Pills on a cream panel, coloured by palette bank (see pill()).
    for (int t = 0; t < 4; t++)
    {
        for (int y = 0; y < 8; y++)
            for (int x = 0; x < 8; x++)
            {
                int v = t < 3 ? rounded(t * 8 + x, y, 0, 0, 24, 8, 3, 1, 15, 13)
                              : rounded(x, y, 0, 0, 8, 8, 3, 1, 15, 13);
                px[y * 8 + x] = v < 0 ? C_CREAM : (uint8_t)v;
            }
        put_tile(tiles, T_PILL + t, px);
    }
    // A selected key on a cream panel.
    for (int t = 0; t < 3; t++)
    {
        for (int y = 0; y < 8; y++)
            for (int x = 0; x < 8; x++)
            {
                int v = rounded(t * 8 + x, y, 0, 0, 24, 8, 3, 1, C_PEACH, C_ORANGE);
                px[y * 8 + x] = v < 0 ? C_CREAM : (uint8_t)v;
            }
        put_tile(tiles, T_KEYSEL + t, px);
    }
    // Verdict dots on a cream panel (the verdict word follows in its colour).
    const uint8_t badge[4] = {C_LEGAL, C_FISHY, C_INVALID, C_SAFE};
    for (int b = 0; b < 4; b++)
    {
        for (int y = 0; y < 8; y++)
            for (int x = 0; x < 8; x++)
            {
                int v = rounded(x, y, 2, 1, 8, 7, 3, 1, badge[b], C_INK);
                px[y * 8 + x] = v < 0 ? C_CREAM : (uint8_t)v;
            }
        put_tile(tiles, T_BADGE + b * 3, px);
    }
}

void set_cell(Surface* c, int col, int row, uint16_t tile, uint8_t kind)
{
    if (col < 0 || col >= 32 || row < 0 || row >= 24)
        return;
    int s = screen_of(c);
    g_fmap[s][row * 32 + col] = tile;
    g_kind[s][row * 32 + col] = kind;
    display::touch_frame_map(s);
}

// Clears text rows [row0, row1) of a surface.
void clear_text_rows(Surface* c, int row0, int row1)
{
    memset(c->tiles + row0 * 32 * 16, 0, (size_t)(row1 - row0) * 32 * 32);
    display::touch_text_rows(c->screen, row0, row1);
}

// Lays text out from (x, y), drawing it (or only measuring when c is null), and returns the x after
// it. grid_col >= 0 lines runs of two or more spaces up with the cell grid counted from that column
// (at()'s table gaps). Stops before a glyph whose ink would pass stop_x and draws `suffix` there.
// Leaves the muted and tint state as the text left it. ink_end (when set) gets the x just past the
// last glyph's ink (trailing spaces draw nothing).
ARM_CODE ITCM_CODE int layout(Surface* c, int x, int y, const char* text, int grid_col, int stop_x, const char* suffix,
                              int* ink_end = nullptr)
{
    constexpr int kNoStop = 1 << 30;
    int chars = 0;  // characters so far (table gaps line up by character, not byte)
    for (int i = 0; text[i];)
    {
        if (text[i] == '\x01')
        {
            g_muted = !g_muted;
            i++;
            continue;
        }
        if (text[i] == '\x02' || text[i] == '\x03')
        {
            uint8_t t = text[i] == '\x02' ? C_LEGAL : C_INVALID;
            g_tint = g_tint == t ? 0 : t;
            i++;
            continue;
        }
        if (text[i] == ' ')
        {
            int run = 1;
            while (grid_col >= 0 && text[i + run] == ' ')
                run++;
            x += kSpaceWidth;
            chars += run;
            if (run > 1)
            {
                int grid = (grid_col + chars) * 8;
                x = x > grid ? x : grid;
            }
            i += run;
            continue;
        }
        uint32_t cp = next_cp(text, &i);
        if (stop_x != kNoStop && glyph(nullptr, x, y, cp) - 1 > stop_x)
        {
            if (suffix)
                x = layout(c, x, y, suffix, -1, kScreen.x1, nullptr);
            return x;
        }
        if (c)
            g_ink = ink_for(c, x, y);
        x = glyph(c, x, y, cp);
        if (ink_end)
            *ink_end = x - 1;
        chars++;
    }
    return x;
}

int measure(const char* text, int grid_col = -1)
{
    int w = layout(nullptr, 0, 0, text, grid_col, 1 << 30, nullptr);
    g_muted = false;
    g_tint = 0;
    return w;
}

// Layout faults, read by the emulator harness (tools/emu-harness fails a step that adds one): text
// cut short where its caller didn't plan for it, pixels dropped outside its region, or ink drawn
// over other text. The text holds the latest fault.
extern "C" {
volatile u32 verity_test_layout = 0;
volatile char verity_test_layout_text[96] = "";
}

void fault(const char* what, Surface* c, int x, int y, const char* text, int over = 0)
{
    char buf[sizeof(verity_test_layout_text)];
    int n = snprintf(buf, sizeof(buf), over ? "%s %s %d,%d +%dpx: " : "%s %s %d,%d: ", what,
                     screen_of(c) ? "bottom" : "top", x, y, over);
    for (int i = 0; text[i] && n + 1 < (int)sizeof(buf); i++)
        if ((unsigned char)text[i] >= ' ')
            buf[n++] = text[i];
    buf[n < (int)sizeof(buf) ? n : (int)sizeof(buf) - 1] = 0;
    for (size_t i = 0; i < sizeof(buf); i++)
        verity_test_layout_text[i] = buf[i];
    verity_test_layout = verity_test_layout + 1;
}

// Panels (with what sits on them), bars, slots and the backdrop: text stays in the region it starts
// on.
int region_group(uint8_t k)
{
    switch (k)
    {
    case K_BAR:
    case K_CHIP: return 1;
    case K_PANEL:
    case K_PILL_DARK:
    case K_PILL_LIGHT: return 2;
    case K_SLOT: return 3;
    default: return 0;
    }
}

// The region around the cell where text drawn at (x, y) starts, in text-layer pixels: the run of
// cells of its group along that row and down that column. Text stays off a panel's edge pixels.
Clip region_at(Surface* c, int x, int y)
{
    const uint8_t* kind = g_kind[screen_of(c)];
    int col = (x + 2) / 8, row = y / 8;
    col = col < 0 ? 0 : col > 31 ? 31 : col;
    row = row < 0 ? 0 : row > 23 ? 23 : row;
    int g = region_group(kind[row * 32 + col]);
    int l = col, r = col + 1, t = row, b = row + 1;
    while (l > 0 && region_group(kind[row * 32 + l - 1]) == g)
        l--;
    while (r < 32 && region_group(kind[row * 32 + r]) == g)
        r++;
    while (t > 0 && region_group(kind[(t - 1) * 32 + col]) == g)
        t--;
    while (b < 24 && region_group(kind[b * 32 + col]) == g)
        b++;
    int pad = g == 2 ? 1 : 0;
    Clip clip = {l * 8 - 2 + pad, t * 8, r * 8 - 2 - pad, b * 8};
    clip.x0 = clip.x0 < 0 ? 0 : clip.x0;
    clip.x1 = clip.x1 > kScreen.x1 ? kScreen.x1 : clip.x1;
    return clip;
}

// The x of text's first glyph (after leading spaces and colour toggles).
int first_glyph_x(int x, const char* text, int grid_col)
{
    x = layout(nullptr, x, 0, text, grid_col, -(1 << 30), nullptr);
    g_muted = false;
    g_tint = 0;
    return x;
}

// Cells from (col, row) to the right edge of the region that cell is in (at least 1).
int region_cells(Surface* c, int col, int row)
{
    if (col < 0 || col >= 32 || row < 0 || row >= 24)
        return 1;
    const uint8_t* kind = g_kind[screen_of(c)] + row * 32;
    int g = region_group(kind[col]), r = col + 1;
    while (r < 32 && region_group(kind[r]) == g)
        r++;
    return r - col;
}

// Draws text inside `clip`, cut with "..." when it doesn't fit; a cut is a fault unless `planned`.
// Returns the end x. `tint` colours the text (on panels), as a UI_RED or UI_BLUE run would.
int draw_in(Surface* c, int x, int y, const char* text, int grid_col, Clip clip, bool planned, uint8_t tint = 0)
{
    int ink_end = x;
    layout(nullptr, x, y, text, grid_col, 1 << 30, nullptr, &ink_end);
    g_muted = false;
    g_tint = 0;
    bool cut = ink_end > clip.x1;
    int end;
    int stop = cut ? clip.x1 - measure("...") - 1 : 1 << 30;
    g_clip = clip;
    g_track = true;
    g_clipped = g_overlap = false;
    g_tint = tint;
    end = layout(c, x, y, text, grid_col, stop, cut ? "..." : nullptr);
    g_clip = kScreen;
    g_track = false;
    g_muted = false;
    g_tint = 0;
    if (cut && !planned)
        fault("cut", c, x, y, text, ink_end - clip.x1);
    if (g_clipped)
        fault("clipped", c, x, y, text);
    if (g_overlap)
        fault("overlap", c, x, y, text);
    return end;
}

// Draws text at a pixel position without clearing, within the region it starts on (inks follow
// the frame), or measures it when c is null. Returns the end x.
int draw_text(Surface* c, int x, int y, const char* text)
{
    if (!c)
        return x + measure(text);
    return draw_in(c, x, y, text, -1, region_at(c, first_glyph_x(x, text, -1), y), false);
}
}  // namespace

Surface* top() { return &g_top; }
Surface* bottom() { return &g_bottom; }

void clear(Surface* c) { clear_text_rows(c, 0, 24); }

void at(Surface* c, int col, int row, const char* fmt, ...)
{
    char text[128];
    va_list ap;
    va_start(ap, fmt);
    vsnprintf(text, sizeof(text), fmt, ap);
    va_end(ap);
    if (row < 0 || row >= 24 || col < 0 || col >= 32)
        return;
    memset(c->tiles + (row * 32 + col) * 16, 0, (size_t)(32 - col) * 32);
    display::touch_text_rows(c->screen, row, row + 1);
    draw_in(c, col * 8, row * 8, text, col, region_at(c, first_glyph_x(col * 8, text, col), row * 8), false);
}

void utf16_to_text(const uint16_t* s, char* out, int cap)
{
    int n = 0;
    for (; *s && n + 1 < cap; s++)
    {
        uint16_t c = *s;
        char o;
        verity_glyph g;
        if (c >= 0x20 && c < 0x7F)
            o = (char)c;
        else if (c == 0x2642)
            o = 'M';  // male sign (Nidoran)
        else if (c == 0x2640)
            o = 'F';
        else if (c == 0x3000)
            o = ' ';
        else if (c >= 0x80 && rom_glyph(c, &g))
        {
            int len = c < 0x800 ? 2 : 3;
            if (n + len + 1 > cap)
                break;
            if (len == 2)
                out[n++] = (char)(0xC0 | (c >> 6));
            else
            {
                out[n++] = (char)(0xE0 | (c >> 12));
                out[n++] = (char)(0x80 | ((c >> 6) & 0x3F));
            }
            out[n++] = (char)(0x80 | (c & 0x3F));
            continue;
        }
        else if ((c >= 0xE8 && c <= 0xEB) || (c >= 0xC8 && c <= 0xCB))
            o = 'e';
        else
            o = '?';
        out[n++] = o;
    }
    out[n] = 0;
}

void set_rom_font(const uint8_t* cache, size_t len)
{
    g_rom_font = cache;
    g_rom_font_len = len;
}

bool has_rom_font() { return g_rom_font != nullptr; }

bool has_glyph(uint16_t c)
{
    verity_glyph g;
    return (c >= 0x20 && c < 0x7F) || rom_glyph(c, &g);
}

namespace {
// Pill colours per screen: palette bank b (1..15) is the theme with colour 15 = g_pill[s][b].
uint16_t g_pill[2][16];
int g_pills[2];
}  // namespace

void frame_clear(Surface* c)
{
    for (int i = 0; i < kTiles; i++)
        set_cell(c, i % 32, i / 32, T_BACK, K_BACK);
    g_pills[screen_of(c)] = 0;
}

void frame_clear_rows(Surface* c, int row0, int row1)
{
    for (int row = row0; row < row1 && row < 24; row++)
    {
        for (int col = 0; col < 32; col++)
            set_cell(c, col, row, T_BACK, K_BACK);
        clear_text_rows(c, row, row + 1);
    }
}

int pill(Surface* c, int col, int row, uint16_t color, const char* text)
{
    int s = screen_of(c);
    int bank = 0;
    for (int b = 1; b <= g_pills[s] && !bank; b++)
        if (g_pill[s][b] == color)
            bank = b;
    if (!bank && g_pills[s] < 15)
    {
        bank = ++g_pills[s];
        g_pill[s][bank] = color;
        uint16_t* pal = display::bg_palette(s) + 16 * bank;
        display::touch_bg_palette(s);
        for (int i = 0; i < 16; i++)
            pal[i] = kTheme[i];
        int r = color & 31, g = color >> 5 & 31, b = color >> 10 & 31;
        pal[15] = color;
        pal[13] = RGB15(r * 3 / 4, g * 3 / 4, b * 3 / 4);  // edge
    }
    // White text unless the colour is light (luma on a 0..31 scale).
    int r = color & 31, g = color >> 5 & 31, b = color >> 10 & 31;
    uint8_t kind = bank && (r * 3 + g * 6 + b) / 10 < 18 ? K_PILL_LIGHT : K_PILL_DARK;
    int w = draw_text(nullptr, 0, 0, text);
    int cells = (w + 8 + 7) / 8;
    int room = region_cells(c, col, row);  // a long pill stops at its panel's edge (its text is cut)
    cells = cells < room ? cells : room;
    uint16_t pb = (uint16_t)(bank << 12);
    for (int k = 0; k < cells; k++)
        set_cell(c, col + k, row,
                 pb | (T_PILL + (cells == 1 ? 3 : k == 0 ? 0 : k == cells - 1 ? 2 : 1)), kind);
    draw_text(c, col * 8 + (cells * 8 - w) / 2 - 2, row * 8, text);
    return col + cells;
}

void title(Surface* c, const char* fmt, ...)
{
    char text[96];
    va_list ap;
    va_start(ap, fmt);
    vsnprintf(text, sizeof(text), fmt, ap);
    va_end(ap);
    for (int col = 0; col < 32; col++)
        set_cell(c, col, 0, T_BAR, K_BAR);
    clear_text_rows(c, 0, 1);
    draw_text(c, 4, 0, text);
}

void title_right(Surface* c, const char* text)
{
    int w = draw_text(nullptr, 0, 0, text);
    draw_text(c, 250 - w, 0, text);
}

namespace {
// Lays out button hints on row 23 with at least `gap` px before each chip after the first; draws
// them when c is set. Returns the x after the last label.
int hint_layout(Surface* c, const char* spec, int gap)
{
    const int row = 23;
    int col = 0, x = 0;
    char buf[96];
    snprintf(buf, sizeof(buf), "%s", spec);
    char* save = nullptr;
    for (char* item = strtok_r(buf, "|", &save); item && col < 31; item = strtok_r(nullptr, "|", &save))
    {
        char* label = strchr(item, ':');
        if (label)
            *label++ = 0;
        int kw = measure(item);
        int cells = kw <= 6 ? 1 : (kw + 9) / 8;
        if (c && cells == 1)
            set_cell(c, col, row, T_CHIP + 3, K_CHIP);
        else if (c)
            for (int k = 0; k < cells; k++)
                set_cell(c, col + k, row, T_CHIP + (k == 0 ? 0 : k == cells - 1 ? 2 : 1), K_CHIP);
        if (c)
            draw_text(c, col * 8 + (cells * 8 - kw) / 2 - 1, row * 8, item);
        x = col * 8 + cells * 8 + 2;
        if (label)
            x = c ? draw_text(c, x, row * 8, label) : x + measure(label);
        col = (x + gap + 7) / 8;
    }
    return x;
}
}  // namespace

void hints(Surface* c, const char* spec)
{
    const int row = 23;
    for (int col = 0; col < 32; col++)
        set_cell(c, col, row, T_BAR, K_BAR);
    clear_text_rows(c, row, row + 1);
    // spec: "A:Open|B:Back|L/R:Box"; each key gets a chip, then its label, at least 5 px apart, or
    // 1 px when that doesn't fit the bar.
    hint_layout(c, spec, hint_layout(nullptr, spec, 5) - 1 > kScreen.x1 ? 1 : 5);
}

void tabs(Surface* c, const char* const* names, int n, int active)
{
    for (int col = 0; col < 32; col++)
        set_cell(c, col, 0, T_BAR, K_BAR);
    clear_text_rows(c, 0, 1);
    // n equal segments (taps map by segment); the active tab is a cream chip.
    for (int t = 0; t < n; t++)
    {
        int x0 = t * 256 / n, x1 = (t + 1) * 256 / n;
        int w = draw_text(nullptr, 0, 0, names[t]);
        if (t == active)
        {
            int cells = (w + 9) / 8;
            int col = (x0 + x1) / 16 - cells / 2;
            for (int k = 0; k < cells; k++)
                set_cell(c, col + k, 0, T_CHIP + (k == 0 ? 0 : k == cells - 1 ? 2 : 1), K_CHIP);
            draw_text(c, col * 8 + (cells * 8 - w) / 2 - 1, 0, names[t]);
        }
        else
        {
            char muted[32];
            snprintf(muted, sizeof(muted), "\x01%s", names[t]);
            draw_text(c, (x0 + x1) / 2 - w / 2 - 2, 0, muted);
        }
    }
}

void key_highlight(Surface* c, int col, int row)
{
    for (int k = 0; k < 3; k++)
        set_cell(c, col + k, row, T_KEYSEL + k, K_PANEL);
}

void panel(Surface* c, int col, int row, int w, int h)
{
    if (w < 2 || h < 2)
        return;
    for (int y = row - 1; y <= row + h; y++)
        for (int x = col - 1; x <= col + w; x++)
        {
            int cx = x < col ? 0 : x == col ? 1 : x < col + w - 1 ? 2 : x == col + w - 1 ? 3 : 4;
            int cy = y < row ? 0 : y == row ? 1 : y < row + h - 1 ? 2 : y == row + h - 1 ? 3 : 4;
            bool inner = cx >= 1 && cx <= 3 && cy >= 1 && cy <= 3;
            // Leave bars alone where a panel's ring meets them.
            int s = screen_of(c);
            if (!inner && x >= 0 && x < 32 && y >= 0 && y < 24 && g_kind[s][y * 32 + x] == K_BAR)
                continue;
            uint16_t tile = T_PANEL + cy * 5 + cx;
            // A top ring meeting another panel's bottom ring (or the reverse) shares the row.
            if ((cy == 0 || cy == 4) && x >= 0 && x < 32 && y >= 0 && y < 24)
            {
                uint16_t old = g_fmap[s][y * 32 + x];
                uint16_t other = T_PANEL + (cy == 0 ? 4 : 0) * 5 + cx;
                if (old == other || old == T_BETWEEN + cx)
                    tile = T_BETWEEN + cx;
            }
            set_cell(c, x, y, tile, inner ? K_PANEL : K_BACK);
        }
}

int badge(Surface* c, int col, int row, Badge b, const char* text)
{
    // The word takes its verdict's colour (amber is too light for text: burnt orange).
    static const uint8_t inks[4] = {C_LEGAL, C_SAFE, C_INVALID, C_SAFE};
    int cells = 1 + (draw_text(nullptr, 0, 0, text) + 9) / 8;
    int room = region_cells(c, col, row);  // a long badge stops at its panel's edge (its text is cut)
    cells = cells < room ? cells : room;
    set_cell(c, col, row, T_BADGE + (int)b * 3, K_PANEL);
    for (int k = 1; k < cells; k++)
        set_cell(c, col + k, row, T_PANEL + 2 * 5 + 2, K_PANEL);
    int x = (col + 1) * 8, y = row * 8;
    draw_in(c, x, y, text, -1, region_at(c, x, y), false, inks[(int)b]);
    return col + cells;
}

void slot(Surface* c, int col, int row, int style)
{
    for (int t = 0; t < 20; t++)
        set_cell(c, col + t % 5, row + t / 5, T_SLOT + (style % 3) * 20 + t, K_SLOT);
}

int text_width(const char* s) { return draw_text(nullptr, 0, 0, s); }

void text(Surface* c, int x, int y, const char* fmt, ...)
{
    char buf[128];
    va_list ap;
    va_start(ap, fmt);
    vsnprintf(buf, sizeof(buf), fmt, ap);
    va_end(ap);
    draw_text(c, x, y, buf);
}

void fit(char* text, int width)
{
    if (draw_text(nullptr, 0, 0, text) <= width)
        return;
    int room = width - draw_text(nullptr, 0, 0, "...");
    int n = (int)strlen(text);
    while (n > 0)
    {
        do
            n--;
        while (n > 0 && ((unsigned char)text[n] & 0xC0) == 0x80);
        char saved = text[n];
        text[n] = 0;
        bool ok = draw_text(nullptr, 0, 0, text) <= room;
        text[n] = saved;
        if (ok)
            break;
    }
    strcpy(text + n, "...");
}

int text_right(Surface* c, int x_end, int y, const char* fmt, ...)
{
    char buf[128];
    va_list ap;
    va_start(ap, fmt);
    vsnprintf(buf, sizeof(buf), fmt, ap);
    va_end(ap);
    int x = x_end - measure(buf);
    draw_text(c, x, y, buf);
    return x;
}

void text_in(Surface* c, int x, int x_end, int y, const char* fmt, ...)
{
    char buf[128];
    va_list ap;
    va_start(ap, fmt);
    vsnprintf(buf, sizeof(buf), fmt, ap);
    va_end(ap);
    Clip clip = region_at(c, first_glyph_x(x, buf, -1), y);
    clip.x1 = x_end < clip.x1 ? x_end : clip.x1;
    draw_in(c, x, y, buf, -1, clip, true);
}

int Lines::line()
{
    if (pending_gap)
        y += kBlockGap - kPitch;
    pending_gap = false;
    int at = y;
    y += kPitch;
    return at;
}

int Lines::grid_row()
{
    if (pending_gap)
        y += kBlockGap - kPitch;
    pending_gap = false;
    int row = (y + 7) / 8;
    y = row * 8 + kGridPitch;
    return row;
}

void clear_px(Surface* c, int y0, int y1)
{
    for (int y = y0 < 0 ? 0 : y0; y < y1 && y < 192; y++)
        for (int col = 0; col < 32; col++)
        {
            uint16_t* row = c->tiles + ((y / 8) * 32 + col) * 16 + (y % 8) * 2;
            row[0] = row[1] = 0;
            *c->dirty |= 1u << (y / 8);
        }
}

int wrap_px(Surface* c, int x, int y, int pitch, int width, int max_rows, const char* text, bool muted)
{
    char line[96];
    int rows = 0;
    const char* p = text;
    while (*p && rows < max_rows)
    {
        while (*p == ' ')
            p++;
        int n = 0;  // bytes of p on this line
        for (;;)
        {
            const char* w = p + n;
            while (*w == ' ')
                w++;
            const char* e = w;
            while (*e && *e != ' ')
                e++;
            if (e == w)
                break;
            int len = (int)(e - p);
            if (len >= (int)sizeof(line))
                break;
            memcpy(line, p, len);
            line[len] = 0;
            if (n && text_width(line) > width)
                break;
            n = len;
        }
        if (n == 0)  // one word wider than the line: cut it
            n = (int)strnlen(p, sizeof(line) - 1);
        bool last = rows == max_rows - 1 && p[n] && p[n + strspn(p + n, " ")];
        memcpy(line, p, n);
        line[n] = 0;
        if (last)
        {
            // Make room for "..." by dropping words from the end.
            while (n > 0 && text_width(line) + text_width("...") > width)
            {
                while (n > 0 && line[n - 1] != ' ')
                    n--;
                while (n > 0 && line[n - 1] == ' ')
                    n--;
                line[n] = 0;
            }
            strcat(line, "...");
        }
        ui::text(c, x, y + rows * pitch, muted ? UI_MUTED "%s" : "%s", line);
        rows++;
        p += n;
    }
    return rows;
}

int wrap(Surface* c, int col, int row, int width, int max_rows, const char* text, bool muted)
{
    for (int r = 0; r < max_rows; r++)
        at(c, col, row + r, "%s", "");
    return wrap_px(c, col * 8, row * 8, 8, width, max_rows, text, muted);
}

int text_len(const char* s)
{
    int n = 0;
    for (; *s; s++)
        n += ((unsigned char)*s & 0xC0) != 0x80;
    return n;
}

const char* name(uint32_t category, uint32_t id)
{
    static char buf[4][32];
    static int k = 0;
    char* out = buf[k++ & 3];
    uint16_t w[32];
    if (verity_name(category, id, w, 32) == 0)
        return "?";
    utf16_to_text(w, out, 32);
    return out;
}

const char* location(uint8_t generation, uint16_t id)
{
    static char buf[2][32];
    static int k = 0;
    char* out = buf[k++ & 1];
    uint16_t w[32];
    if (verity_location_name(generation, id, w, 32) == 0)
        snprintf(out, 32, "Loc %u", id);
    else
        utf16_to_text(w, out, 32);
    return out;
}

// ---- sprites -------------------------------------------------------------------------------

namespace {

constexpr int kTypes = 18;
uint16_t* g_icon[kTypes];  // 8bpp 32x32, one per type
uint16_t* g_egg;
uint16_t* g_star;
uint16_t* g_cursor;
// Row highlights, 8, 12 and 16 px tall: [height index][on a panel].
uint16_t* g_bars[3][2];

// Type colours (original palette): {body, shade}.
const uint16_t kTypeColor[kTypes][2] = {
    {RGB15(22, 22, 18), RGB15(14, 14, 11)},  // Normal
    {RGB15(24, 8, 6), RGB15(14, 4, 3)},      // Fighting
    {RGB15(18, 20, 30), RGB15(10, 12, 20)},  // Flying
    {RGB15(20, 10, 22), RGB15(12, 5, 14)},   // Poison
    {RGB15(26, 22, 12), RGB15(17, 13, 6)},   // Ground
    {RGB15(22, 19, 10), RGB15(13, 11, 5)},   // Rock
    {RGB15(18, 24, 6), RGB15(10, 15, 3)},    // Bug
    {RGB15(14, 12, 22), RGB15(7, 6, 13)},    // Ghost
    {RGB15(22, 22, 26), RGB15(13, 13, 17)},  // Steel
    {RGB15(30, 14, 6), RGB15(20, 7, 3)},     // Fire
    {RGB15(10, 16, 30), RGB15(5, 9, 20)},    // Water
    {RGB15(12, 26, 10), RGB15(6, 16, 5)},    // Grass
    {RGB15(31, 26, 6), RGB15(21, 16, 2)},    // Electric
    {RGB15(30, 12, 18), RGB15(20, 6, 10)},   // Psychic
    {RGB15(18, 28, 28), RGB15(10, 18, 19)},  // Ice
    {RGB15(14, 8, 30), RGB15(8, 4, 19)},     // Dragon
    {RGB15(10, 8, 8), RGB15(5, 4, 4)},       // Dark
    {RGB15(20, 20, 20), RGB15(12, 12, 12)},  // unknown
};

}  // namespace

uint16_t type_color(int type) { return kTypeColor[type >= 0 && type < kTypes ? type : kTypes - 1][0]; }

namespace {

// Sprite palette (8bpp, shared): 1 outline, 2 eye, 3 egg, 4 egg spots, 5 star, 6 cursor,
// 7 bar, then 16 + 2*type body/shade.
void palette(uint16_t* pal)
{
    pal[1] = RGB15(2, 3, 5);
    pal[2] = RGB15(31, 31, 31);
    pal[3] = RGB15(30, 29, 24);
    pal[4] = RGB15(12, 24, 12);
    pal[5] = RGB15(31, 28, 4);
    pal[6] = RGB15(30, 17, 5);   // cursor: theme orange
    pal[7] = RGB15(8, 14, 24);   // highlight on navy (scenes without panels)
    pal[8] = RGB15(31, 28, 23);  // highlight on a panel: peach
    pal[9] = RGB15(30, 17, 5);   //   with an orange edge
    for (int t = 0; t < kTypes; t++)
    {
        pal[16 + 2 * t] = kTypeColor[t][0];
        pal[17 + 2 * t] = kTypeColor[t][1];
    }
}

// Writes an 8bpp pixel into a sprite buffer of width w (1D tile mapping).
void put8(uint8_t* buf, int w, int x, int y, uint8_t c)
{
    int tile = (y / 8) * (w / 8) + x / 8;
    buf[tile * 64 + (y % 8) * 8 + (x % 8)] = c;
}

void copy16(uint16_t* dst, const uint8_t* src, size_t bytes)
{
    for (size_t i = 0; i < bytes / 2; i++)
        dst[i] = src[2 * i] | (src[2 * i + 1] << 8);
}

// A round creature: body with a lighter top, outline, two eyes and small ears.
void draw_creature(uint8_t* buf, uint8_t body, uint8_t shade)
{
    memset(buf, 0, 32 * 32);
    for (int y = 0; y < 32; y++)
        for (int x = 0; x < 32; x++)
        {
            int dx = x - 16, dy = y - 18;
            int d = dx * dx + (dy * dy * 5) / 4;
            bool ear = (y >= 4 && y < 10) && ((x >= 7 && x < 11 && x - 7 >= 9 - y) ||
                                              (x >= 21 && x < 25 && 24 - x >= 9 - y));
            if (d < 11 * 11 || ear)
                put8(buf, 32, x, y, (d >= 10 * 10 && !ear) ? 1 : (dy > 3 ? shade : body));
        }
    for (int ex : {12, 19})
        for (int y = 15; y < 18; y++)
            for (int x = ex; x < ex + 2; x++)
                put8(buf, 32, x, y, 1);
    put8(buf, 32, 12, 15, 2);
    put8(buf, 32, 19, 15, 2);
}

void draw_egg(uint8_t* buf)
{
    memset(buf, 0, 32 * 32);
    for (int y = 0; y < 32; y++)
        for (int x = 0; x < 32; x++)
        {
            int dx = x - 16, dy = y - 17;
            int d = dx * dx * 16 / 10 + dy * dy;
            if (d < 12 * 12)
                put8(buf, 32, x, y, d >= 11 * 11 ? 1 : ((x * 7 + y * 3) % 11 == 0 ? 4 : 3));
        }
}

}  // namespace

void sprites_init()
{
    // Consoles also live here so all display setup is in one place.
    videoSetMode(MODE_0_2D);
    videoSetModeSub(MODE_0_2D);
    vramSetPrimaryBanks(VRAM_A_MAIN_BG, VRAM_B_MAIN_SPRITE, VRAM_C_SUB_BG, VRAM_D_SUB_SPRITE);
    measure_font();
    int main_bg = bgInit(0, BgType_Text4bpp, BgSize_T_256x256, 31, 1);
    int sub_bg = bgInitSub(0, BgType_Text4bpp, BgSize_T_256x256, 31, 1);
    int main_frame = bgInit(1, BgType_Text4bpp, BgSize_T_256x256, 30, 3);
    int sub_frame = bgInitSub(1, BgType_Text4bpp, BgSize_T_256x256, 30, 3);
    make_frame_tiles(bgGetGfxPtr(main_frame));
    make_frame_tiles(bgGetGfxPtr(sub_frame));
    bgSetPriority(main_frame, 3);
    bgSetPriority(sub_frame, 3);
    for (int s = 0; s < 2; s++)
    {
        Surface* c = s ? &g_bottom : &g_top;
        // The text layer's map is fixed (one tile per cell), so it is written once here.
        uint16_t* map = bgGetMapPtr(s ? sub_bg : main_bg);
        for (int i = 0; i < 32 * 32; i++)
            map[i] = i < kTiles ? (uint16_t)i : 0;
        display::init(s, bgGetGfxPtr(s ? sub_bg : main_bg), bgGetMapPtr(s ? sub_frame : main_frame));
        c->tiles = display::text_tiles(s);
        c->dirty = display::text_dirty(s);
        c->screen = s;
        g_fmap[s] = display::frame_map(s);
        clear(c);
        frame_clear(c);
        uint16_t* pal = display::bg_palette(s);
        for (int i = 0; i < 16; i++)
            pal[i] = kTheme[i];
    }
    bgSetPriority(main_bg, 0);
    bgSetPriority(sub_bg, 0);
    // Text sits 2 px right of the cell grid, so it clears panel edges.
    bgSetScroll(main_bg, -2, 0);
    bgSetScroll(sub_bg, -2, 0);
    bgUpdate();

    oamInit(&oamMain, SpriteMapping_1D_64, false);
    oamInit(&oamSub, SpriteMapping_1D_64, false);
    palette(display::sprite_palette(0));
    palette(display::sprite_palette(1));

    static uint8_t buf[64 * 32];
    for (int t = 0; t < kTypes; t++)
    {
        g_icon[t] = oamAllocateGfx(&oamSub, SpriteSize_32x32, SpriteColorFormat_256Color);
        draw_creature(buf, 16 + 2 * t, 17 + 2 * t);
        copy16(g_icon[t], buf, 32 * 32);
    }
    g_egg = oamAllocateGfx(&oamSub, SpriteSize_32x32, SpriteColorFormat_256Color);
    draw_egg(buf);
    copy16(g_egg, buf, 32 * 32);

    g_star = oamAllocateGfx(&oamSub, SpriteSize_8x8, SpriteColorFormat_256Color);
    memset(buf, 0, 64);
    const char* star = "...X...."
                       "..XXX..."
                       "XXXXXXX."
                       ".XXXXX.."
                       "..XXX..."
                       ".XX.XX.."
                       ".X...X.."
                       "........";
    for (int i = 0; i < 64; i++)
        buf[i] = star[i] == 'X' ? 5 : 0;
    copy16(g_star, buf, 64);

    g_cursor = oamAllocateGfx(&oamSub, SpriteSize_32x32, SpriteColorFormat_256Color);
    memset(buf, 0, 32 * 32);
    for (int i = 0; i < 32; i++)
        for (int k = 0; k < 2; k++)
        {
            bool corner = i < 8 || i >= 24;
            if (!corner)
                continue;
            put8(buf, 32, i, k, 6);
            put8(buf, 32, i, 31 - k, 6);
            put8(buf, 32, k, i, 6);
            put8(buf, 32, 31 - k, i, 6);
        }
    copy16(g_cursor, buf, 32 * 32);

    // Navy blue off panels; peach with an orange edge on them.
    for (int k = 0; k < 3; k++)
        for (int on = 0; on < 2; on++)
        {
            int h = 8 + 4 * k;
            g_bars[k][on] = oamAllocateGfx(&oamSub, SpriteSize_64x32, SpriteColorFormat_256Color);
            memset(buf, 0, 64 * 32);
            for (int y = 0; y < h; y++)
                for (int x = 0; x < 64; x++)
                    put8(buf, 64, x, y, !on ? 7 : (y == 0 || y == h - 1) ? 9 : 8);
            copy16(g_bars[k][on], buf, 64 * 32);
        }
    // Show the cleared screens (main() may draw a message before the scene loop starts).
    display::present();
}

void sprites_hide_all()
{
    oamClear(&oamSub, 0, 128);
    oamClear(&oamMain, 0, 128);
}

void icon(int id, int x, int y, uint8_t type, bool egg, bool shiny)
{
    uint16_t* gfx = egg ? g_egg : g_icon[type < kTypes ? type : kTypes - 1];
    oamSet(&oamSub, id, x, y, 2, 0, SpriteSize_32x32, SpriteColorFormat_256Color, gfx, -1, false,
           false, false, false, false);
    oamSet(&oamSub, id + 1, x + 23, y + 1, 1, 0, SpriteSize_8x8, SpriteColorFormat_256Color,
           g_star, -1, false, !shiny || egg, false, false, false);
}

namespace {
constexpr int kRomPalBase = 64;  // ROM icon palettes take 64.. of the 256-colour sprite palette
display::SpriteGfx* g_rom_slot[128];  // one 8bpp 32x32 graphic per OAM id that shows a ROM icon

// Sets colours [first, first + 15] of a screen's sprite palette from colours 1..15 of `colors`.
void sprite_colors(int screen, int first, const uint16_t* colors)
{
    uint16_t* pal = display::sprite_palette(screen) + first;
    for (int i = 1; i < 16; i++)
        pal[i] = colors[i];
    display::touch_sprite_palette(screen);
}

// 4 bpp (low nibble first) tiles to 8 bpp on palette colours base + 1.., same 1D tile order.
void to_8bpp(display::SpriteGfx* g, const uint8_t* tiles, int base)
{
    for (int i = 0; i < 512; i++)
    {
        uint8_t lo = tiles[i] & 15, hi = tiles[i] >> 4;
        g->ram[i] = (uint16_t)((lo ? base + lo : 0) | (hi ? base + hi : 0) << 8);
    }
    display::touch(g);
}
}  // namespace

void set_icon_palettes(const uint16_t (*pal)[16], int count)
{
    for (int p = 0; p < count; p++)
        sprite_colors(1, kRomPalBase + 16 * p, pal[p]);
}

void icon_rom(int id, int x, int y, const uint8_t* tiles, uint8_t palette, bool shiny)
{
    if (id < 0 || id >= 128)
        return;
    if (!g_rom_slot[id])
        g_rom_slot[id] = display::sprite_gfx(&oamSub, SpriteSize_32x32, SpriteColorFormat_256Color);
    display::SpriteGfx* g = g_rom_slot[id];
    if (!g)
        return;
    to_8bpp(g, tiles, kRomPalBase + 16 * (palette & 3));
    oamSet(&oamSub, id, x, y, 2, 0, SpriteSize_32x32, SpriteColorFormat_256Color, g->vram, -1, false,
           false, false, false, false);
    oamSet(&oamSub, id + 1, x + 23, y + 1, 1, 0, SpriteSize_8x8, SpriteColorFormat_256Color,
           g_star, -1, false, !shiny, false, false, false);
}

void item_icon(bool top, int x, int y, const uint8_t* tiles, const uint16_t* colors)
{
    // The last OAM entry of each screen and colours 128..143 of its sprite palette.
    constexpr int kId = 127, kBase = 128;
    static display::SpriteGfx* gfx[2];
    OamState* oam = top ? &oamMain : &oamSub;
    if (!tiles)
    {
        oamClear(oam, kId, 1);
        return;
    }
    display::SpriteGfx*& g = gfx[top ? 0 : 1];
    if (!g)
        g = display::sprite_gfx(oam, SpriteSize_32x32, SpriteColorFormat_256Color);
    if (!g)
        return;
    sprite_colors(top ? 0 : 1, kBase, colors);
    to_8bpp(g, tiles, kBase);
    // Priority 0: above the row highlight bar, which crosses the icon on the Item row.
    oamSet(oam, kId, x, y, 0, 0, SpriteSize_32x32, SpriteColorFormat_256Color, g->vram, -1, false,
           false, false, false, false);
}

namespace {
// Top screen OAM: small icons 110.., the Pokémon sprite 120-123, the item icon 127 (behind the
// sprite: lower IDs draw on top). Palette banks: 8 item icon, 9 sprite, 10.. small icons.
constexpr int kSmallId = 110, kSpriteId = 120, kSpriteBank = 9, kSmallBank = 10;

void copy16(uint16_t* dst, const uint8_t* src, int bytes)
{
    for (int i = 0; i < bytes / 2; i++)
        dst[i] = (uint16_t)(src[2 * i] | src[2 * i + 1] << 8);
}
}  // namespace

void pokemon_sprite(int x, int y, const uint8_t* tiles, const uint16_t* colors)
{
    static const SpriteSize size[4] = {SpriteSize_64x64, SpriteSize_32x64, SpriteSize_64x32, SpriteSize_32x32};
    static const int dx[4] = {0, 64, 0, 64}, dy[4] = {0, 0, 64, 64}, bytes[4] = {2048, 1024, 1024, 512};
    static display::SpriteGfx* gfx[4];
    if (!tiles)
    {
        oamClear(&oamMain, kSpriteId, 4);
        return;
    }
    sprite_colors(0, 16 * kSpriteBank, colors);
    int off = 0;
    for (int b = 0; b < 4; b++)
    {
        if (!gfx[b])
            gfx[b] = display::sprite_gfx(&oamMain, size[b], SpriteColorFormat_16Color);
        if (!gfx[b])
            return;
        copy16(gfx[b]->ram, tiles + off, bytes[b]);
        display::touch(gfx[b]);
        off += bytes[b];
        oamSet(&oamMain, kSpriteId + b, x + dx[b], y + dy[b], 2, kSpriteBank, size[b], SpriteColorFormat_16Color,
               gfx[b]->vram, -1, false, false, false, false, false);
    }
}

void logo(int x, int y)
{
    static display::SpriteGfx* gfx;
    if (!gfx)
    {
        gfx = display::sprite_gfx(&oamMain, SpriteSize_64x64, SpriteColorFormat_16Color);
        if (!gfx)
            return;
        copy16(gfx->ram, kLogoTiles, (int)sizeof(kLogoTiles));
        display::touch(gfx);
    }
    sprite_colors(0, 16 * kSpriteBank, kLogoColors);
    // The Pokémon sprite's four blocks share this slot; only the first is the logo.
    oamClear(&oamMain, kSpriteId + 1, 3);
    oamSet(&oamMain, kSpriteId, x, y, 2, kSpriteBank, SpriteSize_64x64, SpriteColorFormat_16Color, gfx->vram,
           -1, false, false, false, false, false);
}

void small_icon(int slot, int x, int y, const uint8_t* tiles, const uint16_t* colors)
{
    static display::SpriteGfx* gfx[kSmallIcons];
    if (slot < 0 || slot >= kSmallIcons)
        return;
    if (!tiles)
    {
        oamClear(&oamMain, kSmallId + slot, 1);
        return;
    }
    if (!gfx[slot])
        gfx[slot] = display::sprite_gfx(&oamMain, SpriteSize_32x16, SpriteColorFormat_16Color);
    if (!gfx[slot])
        return;
    sprite_colors(0, 16 * (kSmallBank + slot), colors);
    copy16(gfx[slot]->ram, tiles, 256);
    display::touch(gfx[slot]);
    oamSet(&oamMain, kSmallId + slot, x, y, 1, kSmallBank + slot, SpriteSize_32x16, SpriteColorFormat_16Color,
           gfx[slot]->vram, -1, false, false, false, false, false);
}

void top_art_hide()
{
    oamClear(&oamMain, kSmallId, kSmallIcons);
    oamClear(&oamMain, kSpriteId, 4);
}

void shape(Surface* c, int x, int y, int kind, int tone)
{
    // 7 rows of 7 bits (bit 6 is the left column).
    static const uint8_t k[9][7] = {
        {0x1C, 0x3E, 0x7F, 0x7F, 0x7F, 0x3E, 0x1C},  // circle
        {0x08, 0x1C, 0x1C, 0x3E, 0x3E, 0x7F, 0x7F},  // triangle
        {0x00, 0x7E, 0x7E, 0x7E, 0x7E, 0x7E, 0x7E},  // square
        {0x36, 0x7F, 0x7F, 0x7F, 0x3E, 0x1C, 0x08},  // heart
        {0x08, 0x08, 0x7F, 0x3E, 0x1C, 0x36, 0x22},  // star
        {0x08, 0x1C, 0x3E, 0x7F, 0x3E, 0x1C, 0x08},  // diamond
        {0x08, 0x08, 0x7F, 0x3E, 0x1C, 0x36, 0x22},  // shiny star
        {0x07, 0x03, 0x3A, 0x44, 0x44, 0x44, 0x38},  // male
        {0x1C, 0x22, 0x22, 0x1C, 0x08, 0x3E, 0x08},  // female
    };
    if (kind < 0 || kind > kFemale)
        return;
    bool muted = g_muted;
    g_muted = tone == 1;
    static const uint8_t fixed[5] = {0, 0, C_FISHY, C_LEGAL, C_INVALID};
    g_ink = tone >= 2 && tone <= 4 ? fixed[tone] : ink_for(c, x, y);
    g_muted = muted;
    for (int r = 0; r < 7; r++)
        for (int col = 0; col < 7; col++)
            if (k[kind][r] & (0x40 >> col))
                plot(c, x + col, y + r);
}

void cursor(int id, int x, int y)
{
    oamSet(&oamSub, id, x, y, 0, 0, SpriteSize_32x32, SpriteColorFormat_256Color, g_cursor, -1,
           false, false, false, false, false);
}

void bar(int first, int y, bool visible, int height)
{
    // Over a panel the highlight is peach with an orange edge (dark text); elsewhere navy blue.
    int row = (y + height / 2) / 8;
    bool on_panel = row >= 0 && row < 24 && g_kind[1][row * 32 + 4] == K_PANEL;
    int k = height >= 16 ? 2 : height >= 12 ? 1 : 0;
    uint16_t* gfx = g_bars[k][on_panel ? 1 : 0];
    for (int i = 0; i < 4; i++)
        oamSet(&oamSub, first + i, i * 64, y, 1, 0, SpriteSize_64x32, SpriteColorFormat_256Color,
               gfx, -1, false, !visible, false, false, false);
}

}  // namespace ui
