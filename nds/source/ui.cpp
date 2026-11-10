// SPDX-License-Identifier: GPL-3.0-only
#include "ui.h"

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

void push(Scene* s) { g_pending = s; }

void run()
{
    Input in;
    while (true)
    {
        if (g_pending)
        {
            if (!g_stack.empty())
                fade(true);
            g_stack.push_back(g_pending);
            g_pending = nullptr;
            sprites_hide_all();
            frame_clear(top());
            frame_clear(bottom());
            g_stack.back()->enter();
            sprites_commit();
            fade(false);
        }
        if (g_stack.empty())
            break;
        swiWaitForVBlank();
        in.poll();
        if (!g_stack.back()->update(in))
        {
            fade(true);
            delete g_stack.back();
            g_stack.pop_back();
            if (g_stack.empty())
                break;
            sprites_hide_all();
            frame_clear(top());
            frame_clear(bottom());
            g_stack.back()->enter();
            sprites_commit();
            fade(false);
            continue;
        }
        sprites_commit();
    }
}

// ---- text ----------------------------------------------------------------------------------

// Proportional 8-pixel-high text. Each screen is a 32x24 grid of its own tiles, so text is drawn
// as pixels. Glyphs come from libnds' console font, trimmed to their inked columns.
namespace {
constexpr int kTiles = 32 * 24;
constexpr int kSpaceWidth = 4;
int g_digit_width = 6;  // digits share one width so numbers line up

const uint8_t* g_font;
int g_font_first, g_font_count;
uint8_t g_left[128], g_width[128];  // inked start column and width per ASCII code

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
}

uint8_t g_ink = 1;  // palette index plot() writes (see the theme below)

void plot(Surface* c, int x, int y)
{
    if (x < 0 || x >= 256 || y < 0 || y >= 192)
        return;
    int tile = (y / 8) * 32 + x / 8;
    uint16_t* word = c->tiles + tile * 16 + (y % 8) * 2 + (x % 8) / 4;
    int shift = ((x % 8) % 4) * 4;
    *word = (uint16_t)((*word & ~(0xF << shift)) | (g_ink << shift));
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
int glyph(Surface* c, int x, int y, uint32_t cp)
{
    if (cp >= 128)
    {
        verity_glyph g;
        if (cp > 0xFFFF || !rom_glyph((uint16_t)cp, &g))
            cp = '?';
        else
        {
            for (int row = 0; c && row < g.height; row++)
                for (int col = 0; col < g.width; col++)
                    if (g.rows[row] & (1u << col))
                        plot(c, x + col, y + row);
            return x + (g.width ? g.width + 1 : kSpaceWidth);
        }
    }
    unsigned char ch = (unsigned char)cp;
    int ix = ch - g_font_first;
    if (ix < 0 || ix >= g_font_count)
        return x + kSpaceWidth;
    int digit_pad = (ch >= '0' && ch <= '9') ? (g_digit_width - 1 - g_width[ch]) / 2 : 0;
    for (int row = 0; c && row < 8; row++)
    {
        uint8_t bits = g_font[ix * 8 + row];
        for (int col = 0; col < g_width[ch]; col++)
            if (bits & (1 << (col + g_left[ch])))
                plot(c, x + digit_pad + col, y + row);
    }
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
    K_LEGAL,
    K_FISHY,
    K_INVALID,
    K_SAFE,
    K_SLOT,
};
// Tile numbers in the frame layer.
enum : uint16_t
{
    T_BACK = 1,
    T_BAR,
    T_PANEL = 3,      // 25: the 5x5 slices of a canonical panel (ring, corners, edges, middle)
    T_SLOT = 28,      // 3 styles x 16
    T_CHIP = 76,      // left, middle, right, single
    T_BADGE = 80,     // 4 colours x (left, middle, right)
    T_COUNT = 92,
};

uint16_t* g_fmap[2];
uint8_t g_kind[2][kTiles];
bool g_muted = false;

int screen_of(Surface* c) { return c == &g_bottom ? 1 : 0; }

uint8_t ink_for(Surface* c, int x, int y)
{
    int col = x / 8, row = y / 8;
    uint8_t k = (col >= 0 && col < 32 && row >= 0 && row < 24) ? g_kind[screen_of(c)][row * 32 + col]
                                                               : K_BACK;
    bool dark = k == K_PANEL || k == K_CHIP || k == K_FISHY || k == K_SLOT;
    if (g_muted)
        return dark ? C_MUTED : C_MUTED_L;
    if (k == K_LEGAL || k == K_INVALID || k == K_SAFE)
        return C_WHITE;
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
    // Panel: content cells 1..3 of a 5x5 grid; 2 px border just outside them, inset shadow.
    for (int ty = 0; ty < 5; ty++)
        for (int tx = 0; tx < 5; tx++)
        {
            for (int y = 0; y < 8; y++)
                for (int x = 0; x < 8; x++)
                {
                    int X = tx * 8 + x, Y = ty * 8 + y;
                    int v = rounded(X, Y, 6, 6, 34, 34, 5, 2, C_CREAM, C_INK);
                    if (v == C_CREAM && Y >= 30)
                        v = C_SHADOW;
                    px[y * 8 + x] = v < 0 ? backdrop(y) : (uint8_t)v;
                }
            put_tile(tiles, T_PANEL + ty * 5 + tx, px);
        }
    // Slots (32x32): filled, empty, selected.
    const uint8_t fill[3] = {C_CREAM, C_EMPTY, C_PEACH}, edge[3] = {C_INK, C_EMPTY_EDGE, C_ORANGE};
    for (int st = 0; st < 3; st++)
        for (int t = 0; t < 16; t++)
        {
            for (int y = 0; y < 8; y++)
                for (int x = 0; x < 8; x++)
                {
                    int X = (t % 4) * 8 + x, Y = (t / 4) * 8 + y;
                    int v = rounded(X, Y, 1, 1, 31, 31, 5, st == 2 ? 3 : 2, fill[st], edge[st]);
                    px[y * 8 + x] = v < 0 ? backdrop(y) : (uint8_t)v;
                }
            put_tile(tiles, T_SLOT + st * 16 + t, px);
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
    // Badges on a cream panel: pills in the four verdict colours.
    const uint8_t badge[4] = {C_LEGAL, C_FISHY, C_INVALID, C_SAFE};
    for (int b = 0; b < 4; b++)
        for (int t = 0; t < 3; t++)
        {
            for (int y = 0; y < 8; y++)
                for (int x = 0; x < 8; x++)
                {
                    int v = rounded(t * 8 + x, y, 0, 0, 24, 8, 3, 1, badge[b], C_INK);
                    px[y * 8 + x] = v < 0 ? C_CREAM : (uint8_t)v;
                }
            put_tile(tiles, T_BADGE + b * 3 + t, px);
        }
}

void set_cell(Surface* c, int col, int row, uint16_t tile, uint8_t kind)
{
    if (col < 0 || col >= 32 || row < 0 || row >= 24)
        return;
    int s = screen_of(c);
    g_fmap[s][row * 32 + col] = tile;
    g_kind[s][row * 32 + col] = kind;
}

// Draws text at a pixel position without clearing; inks follow the frame. Returns the end x.
int draw_text(Surface* c, int x, int y, const char* text)
{
    for (int i = 0; text[i];)
    {
        if (text[i] == '\x01')
        {
            g_muted = !g_muted;
            i++;
            continue;
        }
        if (text[i] == ' ')
        {
            x += kSpaceWidth;
            i++;
            continue;
        }
        uint32_t cp = next_cp(text, &i);
        if (c)
            g_ink = ink_for(c, x, y);
        x = glyph(c, x, y, cp);
    }
    g_muted = false;
    return x;
}
}  // namespace

Surface* top() { return &g_top; }
Surface* bottom() { return &g_bottom; }

void clear(Surface* c) { memset(c->tiles, 0, kTiles * 32); }

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
    int x = col * 8, y = row * 8;
    int chars = 0;  // characters so far (table gaps line up by character, not byte)
    for (int i = 0; text[i];)
    {
        if (text[i] == '\x01')
        {
            g_muted = !g_muted;
            i++;
            continue;
        }
        if (text[i] != ' ')
        {
            uint32_t cp = next_cp(text, &i);
            g_ink = ink_for(c, x, y);
            x = glyph(c, x, y, cp);
            chars++;
            continue;
        }
        int run = 1;
        while (text[i + run] == ' ')
            run++;
        x += kSpaceWidth;
        chars += run;
        // Two or more spaces mean a table column: line up with the character grid.
        if (run > 1)
        {
            int grid = (col + chars) * 8;
            x = x > grid ? x : grid;
        }
        i += run;
    }
    g_muted = false;
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

void frame_clear(Surface* c)
{
    for (int i = 0; i < kTiles; i++)
        set_cell(c, i % 32, i / 32, T_BACK, K_BACK);
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
    memset(c->tiles, 0, 32 * 32);
    draw_text(c, 4, 0, text);
}

void title_right(Surface* c, const char* text)
{
    int w = draw_text(nullptr, 0, 0, text);
    draw_text(c, 250 - w, 0, text);
}

void hints(Surface* c, const char* spec)
{
    const int row = 23;
    for (int col = 0; col < 32; col++)
        set_cell(c, col, row, T_BAR, K_BAR);
    memset(c->tiles + row * 32 * 16, 0, 32 * 32);
    // spec: "A:Open|B:Back|L/R:Box"; each key gets a chip, then its label.
    int col = 0;
    char buf[96];
    snprintf(buf, sizeof(buf), "%s", spec);
    for (char* item = strtok(buf, "|"); item && col < 31; item = strtok(nullptr, "|"))
    {
        char* label = strchr(item, ':');
        if (label)
            *label++ = 0;
        int kw = draw_text(nullptr, 0, 0, item);
        int cells = kw <= 6 ? 1 : (kw + 9) / 8;
        if (cells == 1)
            set_cell(c, col, row, T_CHIP + 3, K_CHIP);
        else
            for (int k = 0; k < cells; k++)
                set_cell(c, col + k, row, T_CHIP + (k == 0 ? 0 : k == cells - 1 ? 2 : 1), K_CHIP);
        draw_text(c, col * 8 + (cells * 8 - kw) / 2 - 1, row * 8, item);
        int x = col * 8 + cells * 8 + 2;
        if (label)
            x = draw_text(c, x, row * 8, label);
        col = (x + 9) / 8;
    }
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
            set_cell(c, x, y, T_PANEL + cy * 5 + cx, inner ? K_PANEL : K_BACK);
        }
}

void badge(Surface* c, int col, int row, int cells, Badge b, const char* text)
{
    if (cells < 2)
        cells = 2;
    static const uint8_t kinds[4] = {K_LEGAL, K_FISHY, K_INVALID, K_SAFE};
    for (int k = 0; k < cells; k++)
        set_cell(c, col + k, row, T_BADGE + (int)b * 3 + (k == 0 ? 0 : k == cells - 1 ? 2 : 1),
                 kinds[(int)b]);
    int w = draw_text(nullptr, 0, 0, text);
    draw_text(c, col * 8 + (cells * 8 - w) / 2 - 1, row * 8, text);
}

void slot(Surface* c, int col, int row, int style)
{
    for (int t = 0; t < 16; t++)
        set_cell(c, col + t % 4, row + t / 4, T_SLOT + (style % 3) * 16 + t, K_SLOT);
}

int text_width(const char* s) { return draw_text(nullptr, 0, 0, s); }

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
uint16_t* g_bar;
uint16_t* g_bar_panel;

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
    g_fmap[0] = bgGetMapPtr(main_frame);
    g_fmap[1] = bgGetMapPtr(sub_frame);
    bgSetPriority(main_frame, 3);
    bgSetPriority(sub_frame, 3);
    for (int s = 0; s < 2; s++)
    {
        Surface* c = s ? &g_bottom : &g_top;
        c->tiles = bgGetGfxPtr(s ? sub_bg : main_bg);
        uint16_t* map = bgGetMapPtr(s ? sub_bg : main_bg);
        for (int i = 0; i < 32 * 32; i++)
            map[i] = i < kTiles ? (uint16_t)i : 0;
        clear(c);
        memset(g_fmap[s], 0, 32 * 32 * 2);
        frame_clear(c);
    }
    for (int i = 0; i < 16; i++)
    {
        BG_PALETTE[i] = kTheme[i];
        BG_PALETTE_SUB[i] = kTheme[i];
    }
    bgSetPriority(main_bg, 0);
    bgSetPriority(sub_bg, 0);
    // Text sits 2 px right of the cell grid, so it clears panel edges.
    bgSetScroll(main_bg, -2, 0);
    bgSetScroll(sub_bg, -2, 0);
    bgUpdate();

    oamInit(&oamMain, SpriteMapping_1D_64, false);
    oamInit(&oamSub, SpriteMapping_1D_64, false);
    palette(SPRITE_PALETTE);
    palette(SPRITE_PALETTE_SUB);

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

    g_bar = oamAllocateGfx(&oamSub, SpriteSize_64x32, SpriteColorFormat_256Color);
    memset(buf, 0, 64 * 32);
    for (int y = 0; y < 8; y++)
        for (int x = 0; x < 64; x++)
            put8(buf, 64, x, y, 7);
    copy16(g_bar, buf, 64 * 32);
    g_bar_panel = oamAllocateGfx(&oamSub, SpriteSize_64x32, SpriteColorFormat_256Color);
    memset(buf, 0, 64 * 32);
    for (int y = 0; y < 8; y++)
        for (int x = 0; x < 64; x++)
            put8(buf, 64, x, y, (y == 0 || y == 7) ? 9 : 8);
    copy16(g_bar_panel, buf, 64 * 32);
}

void sprites_hide_all()
{
    oamClear(&oamSub, 0, 128);
    oamClear(&oamMain, 0, 128);
}

void sprites_commit()
{
    oamUpdate(&oamMain);
    oamUpdate(&oamSub);
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
uint16_t* g_rom_slot[128];       // one 8bpp 32x32 graphic per OAM id that shows a ROM icon
}  // namespace

void set_icon_palettes(const uint16_t (*pal)[16], int count)
{
    for (int p = 0; p < count; p++)
        for (int i = 1; i < 16; i++)
            SPRITE_PALETTE_SUB[kRomPalBase + 16 * p + i] = pal[p][i];
}

void icon_rom(int id, int x, int y, const uint8_t* tiles, uint8_t palette, bool shiny)
{
    if (id < 0 || id >= 128)
        return;
    if (!g_rom_slot[id])
        g_rom_slot[id] = oamAllocateGfx(&oamSub, SpriteSize_32x32, SpriteColorFormat_256Color);
    // 4 bpp (low nibble first) to 8 bpp, same 1D tile order.
    uint16_t* dst = g_rom_slot[id];
    int base = kRomPalBase + 16 * (palette & 3);
    for (int i = 0; i < 512; i++)
    {
        uint8_t lo = tiles[i] & 15, hi = tiles[i] >> 4;
        dst[i] = (uint16_t)((lo ? base + lo : 0) | (hi ? base + hi : 0) << 8);
    }
    oamSet(&oamSub, id, x, y, 2, 0, SpriteSize_32x32, SpriteColorFormat_256Color, dst, -1, false,
           false, false, false, false);
    oamSet(&oamSub, id + 1, x + 23, y + 1, 1, 0, SpriteSize_8x8, SpriteColorFormat_256Color,
           g_star, -1, false, !shiny, false, false, false);
}

void cursor(int id, int x, int y)
{
    oamSet(&oamSub, id, x, y, 0, 0, SpriteSize_32x32, SpriteColorFormat_256Color, g_cursor, -1,
           false, false, false, false, false);
}

void bar(int first, int y, bool visible)
{
    // Over a panel the highlight is peach with an orange edge (dark text); elsewhere navy blue.
    int row = y / 8;
    bool on_panel = row >= 0 && row < 24 && g_kind[1][row * 32 + 4] == K_PANEL;
    for (int i = 0; i < 4; i++)
        oamSet(&oamSub, first + i, i * 64, y, 1, 0, SpriteSize_64x32, SpriteColorFormat_256Color,
               on_panel ? g_bar_panel : g_bar, -1, false, !visible, false, false, false);
}

}  // namespace ui
