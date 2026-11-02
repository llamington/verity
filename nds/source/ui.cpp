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
PrintConsole g_top, g_bottom;

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
            g_stack.back()->enter();
            sprites_commit();
            fade(false);
            continue;
        }
        sprites_commit();
    }
}

// ---- text ----------------------------------------------------------------------------------

PrintConsole* top() { return &g_top; }
PrintConsole* bottom() { return &g_bottom; }

void clear(PrintConsole* c)
{
    consoleSelect(c);
    printf("\x1b[2J");
}

void at(PrintConsole* c, int col, int row, const char* fmt, ...)
{
    consoleSelect(c);
    printf("\x1b[%d;%dH", row, col);
    va_list ap;
    va_start(ap, fmt);
    vprintf(fmt, ap);
    va_end(ap);
    printf("\x1b[K");
}

void utf16_to_ascii(const uint16_t* s, char* out, int cap)
{
    int n = 0;
    for (; *s && n + 1 < cap; s++)
    {
        uint16_t c = *s;
        char o;
        if (c >= 0x20 && c < 0x7F)
            o = (char)c;
        else if ((c >= 0xE8 && c <= 0xEB) || (c >= 0xC8 && c <= 0xCB))
            o = 'e';
        else if (c == 0x2642)
            o = 'M';  // male sign (Nidoran)
        else if (c == 0x2640)
            o = 'F';
        else if (c == 0x3000)
            o = ' ';
        else
            o = '?';
        out[n++] = o;
    }
    out[n] = 0;
}

const char* name(uint32_t category, uint32_t id)
{
    static char buf[4][32];
    static int k = 0;
    char* out = buf[k++ & 3];
    uint16_t w[32];
    if (verity_name(category, id, w, 32) == 0)
        return "?";
    utf16_to_ascii(w, out, 32);
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
        utf16_to_ascii(w, out, 32);
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
    pal[6] = RGB15(31, 31, 31);
    pal[7] = RGB15(8, 14, 24);
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
    consoleInit(&g_top, 0, BgType_Text4bpp, BgSize_T_256x256, 31, 0, true, true);
    consoleInit(&g_bottom, 0, BgType_Text4bpp, BgSize_T_256x256, 31, 0, false, true);
    BG_PALETTE[0] = RGB15(3, 5, 9);
    BG_PALETTE_SUB[0] = RGB15(3, 5, 9);
    bgSetPriority(g_bottom.bgId, 0);
    bgSetPriority(g_top.bgId, 0);

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

void cursor(int id, int x, int y)
{
    oamSet(&oamSub, id, x, y, 0, 0, SpriteSize_32x32, SpriteColorFormat_256Color, g_cursor, -1,
           false, false, false, false, false);
}

void bar(int first, int y, bool visible)
{
    for (int i = 0; i < 4; i++)
        oamSet(&oamSub, first + i, i * 64, y, 1, 0, SpriteSize_64x32, SpriteColorFormat_256Color,
               g_bar, -1, false, !visible, false, false, false);
}

}  // namespace ui
