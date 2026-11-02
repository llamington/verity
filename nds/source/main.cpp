// SPDX-License-Identifier: GPL-3.0-only
//
// P9.0 UI spike: sprites on both screens, buttons and touch, fopen of a .sav from SD, and a
// scrolling list driven by D-pad and touch. See docs/research/ui-stack.md.

#include <dirent.h>
#include <initializer_list>
#include <stdio.h>
#include <stdlib.h>
#include <string.h>

#include <fat.h>
#include <nds.h>

#include "verity.h"

extern "C" void verity_on_panic(void)
{
    consoleDemoInit();
    BG_PALETTE_SUB[0] = RGB15(16, 0, 0);
    printf("\x1b[2J\x1b[10;8Hverity: core panic\n");
    while (1)
        swiWaitForVBlank();
}

namespace {

constexpr int kMaxFiles = 256;
constexpr int kPathLen = 192;
constexpr int kListTop = 2;    // first list row on the bottom console
constexpr int kListRows = 20;  // visible rows
constexpr int kMaxSave = 1024 * 1024;

char g_files[kMaxFiles][kPathLen];
int g_file_count = 0;

PrintConsole g_top;
PrintConsole g_bottom;

// ---- sprites --------------------------------------------------------------------------------

// Writes pixel (x, y) of a 4bpp sprite of width w (1D mapping) into a RAM buffer.
void put4(uint8_t *buf, int w, int x, int y, uint8_t c)
{
    int tile = (y / 8) * (w / 8) + x / 8;
    int idx = tile * 32 + (y % 8) * 4 + (x % 8) / 2;
    if (x & 1)
        buf[idx] = (buf[idx] & 0x0F) | (c << 4);
    else
        buf[idx] = (buf[idx] & 0xF0) | c;
}

// VRAM takes 16/32-bit writes only.
void copy16(uint16_t *dst, const uint8_t *src, size_t bytes)
{
    for (size_t i = 0; i < bytes / 2; i++)
        dst[i] = src[2 * i] | (src[2 * i + 1] << 8);
}

// A 32x32 rounded badge (original placeholder art: ring + core).
void draw_badge(uint16_t *gfx)
{
    static uint8_t buf[32 * 32 / 2];
    memset(buf, 0, sizeof(buf));
    for (int y = 0; y < 32; y++)
        for (int x = 0; x < 32; x++)
        {
            int dx = x - 16, dy = y - 16, d = dx * dx + dy * dy;
            if (d < 15 * 15)
                put4(buf, 32, x, y, d < 11 * 11 ? (dy < 0 ? 2 : 3) : 1);
        }
    copy16(gfx, buf, sizeof(buf));
}

// A 64x8 highlight bar in a 64x32 sprite (rows below 8 stay transparent).
void draw_bar(uint16_t *gfx)
{
    static uint8_t buf[64 * 32 / 2];
    memset(buf, 0, sizeof(buf));
    for (int y = 0; y < 8; y++)
        for (int x = 0; x < 64; x++)
            put4(buf, 64, x, y, 1);
    copy16(gfx, buf, sizeof(buf));
}

// ---- files ----------------------------------------------------------------------------------

bool is_save_name(const char *name)
{
    const char *dot = strrchr(name, '.');
    return dot && (strcasecmp(dot, ".sav") == 0 || strcasecmp(dot, ".dsv") == 0);
}

void scan(const char *dir, int depth)
{
    DIR *d = opendir(dir);
    if (!d)
        return;
    struct dirent *e;
    while ((e = readdir(d)) != nullptr && g_file_count < kMaxFiles)
    {
        if (e->d_name[0] == '.')
            continue;
        char path[kPathLen];
        int n = snprintf(path, sizeof(path), "%s%s%s", dir,
                         dir[strlen(dir) - 1] == '/' ? "" : "/", e->d_name);
        if (n <= 0 || n >= kPathLen)
            continue;
        if (e->d_type == DT_DIR)
        {
            if (depth > 0)
                scan(path, depth - 1);
        }
        else if (is_save_name(e->d_name))
        {
            strcpy(g_files[g_file_count++], path);
        }
    }
    closedir(d);
}

const char *kind_name(uint8_t kind)
{
    switch (kind)
    {
        case 1: return "Diamond/Pearl";
        case 2: return "Platinum";
        case 3: return "HeartGold/SoulSilver";
        case 4: return "Black/White";
        case 5: return "Black 2/White 2";
        default: return "?";
    }
}

void print_utf16(const uint16_t *s)
{
    for (; *s; s++)
        putchar(*s < 0x80 ? (char)*s : '?');
}

void open_file(const char *path)
{
    consoleSelect(&g_top);
    printf("\x1b[8;0H\x1b[J");
    printf("%.32s\n\n", path);
    FILE *f = fopen(path, "rb");
    if (!f)
    {
        printf("fopen failed\n");
        return;
    }
    uint8_t *buf = (uint8_t *)malloc(kMaxSave);
    if (!buf)
    {
        fclose(f);
        printf("out of memory\n");
        return;
    }
    size_t len = fread(buf, 1, kMaxSave, f);
    fclose(f);
    verity_save_info info;
    verity_err err = verity_save_probe(buf, len, &info);
    free(buf);
    printf("%u bytes\n", (unsigned)len);
    if (err != VERITY_OK)
    {
        printf("not a Gen 4/5 save (err %d)\n", err);
        return;
    }
    printf("Game: %s\n", kind_name(info.kind));
    printf("OT:   ");
    print_utf16(info.ot_name);
    printf("\nID:   %05u / %05u\n", info.tid, info.sid);
    printf("Time: %u:%02u:%02u\n", info.played_hours, info.played_minutes,
           info.played_seconds);
    printf("Party: %u   Checksums: %s\n", info.party_count,
           info.checksums_valid ? "OK" : "BAD");
}

// ---- list -----------------------------------------------------------------------------------

struct List
{
    int selected = 0;
    int scroll = 0;

    void clamp()
    {
        if (g_file_count == 0)
        {
            selected = scroll = 0;
            return;
        }
        if (selected < 0)
            selected = 0;
        if (selected >= g_file_count)
            selected = g_file_count - 1;
        if (selected < scroll)
            scroll = selected;
        if (selected >= scroll + kListRows)
            scroll = selected - kListRows + 1;
        int max_scroll = g_file_count > kListRows ? g_file_count - kListRows : 0;
        if (scroll > max_scroll)
            scroll = max_scroll;
        if (scroll < 0)
            scroll = 0;
    }

    void draw() const
    {
        consoleSelect(&g_bottom);
        printf("\x1b[0;0H %d save(s)  A:open  L/R:page\x1b[K\n", g_file_count);
        for (int r = 0; r < kListRows; r++)
        {
            int i = scroll + r;
            printf("\x1b[%d;0H", kListTop + r);
            if (i < g_file_count)
            {
                const char *name = strrchr(g_files[i], '/');
                printf(" %.30s", name ? name + 1 : g_files[i]);
            }
            printf("\x1b[K");
        }
    }
};

}  // namespace

int main(int argc, char **argv)
{
    (void)argc;
    (void)argv;

    videoSetMode(MODE_0_2D);
    videoSetModeSub(MODE_0_2D);
    vramSetPrimaryBanks(VRAM_A_MAIN_BG, VRAM_B_MAIN_SPRITE, VRAM_C_SUB_BG, VRAM_D_SUB_SPRITE);

    consoleInit(&g_top, 0, BgType_Text4bpp, BgSize_T_256x256, 31, 0, true, true);
    consoleInit(&g_bottom, 0, BgType_Text4bpp, BgSize_T_256x256, 31, 0, false, true);
    BG_PALETTE[0] = RGB15(3, 5, 9);
    BG_PALETTE_SUB[0] = RGB15(3, 5, 9);
    bgSetPriority(g_bottom.bgId, 0);

    oamInit(&oamMain, SpriteMapping_1D_32, false);
    oamInit(&oamSub, SpriteMapping_1D_32, false);
    uint16_t *badge_main = oamAllocateGfx(&oamMain, SpriteSize_32x32, SpriteColorFormat_16Color);
    uint16_t *badge_sub = oamAllocateGfx(&oamSub, SpriteSize_32x32, SpriteColorFormat_16Color);
    uint16_t *bar = oamAllocateGfx(&oamSub, SpriteSize_64x32, SpriteColorFormat_16Color);
    draw_badge(badge_main);
    draw_badge(badge_sub);
    draw_bar(bar);
    for (u16 *pal : {SPRITE_PALETTE, SPRITE_PALETTE_SUB})
    {
        pal[1] = RGB15(8, 14, 24);
        pal[2] = RGB15(28, 10, 8);
        pal[3] = RGB15(30, 30, 30);
    }

    consoleSelect(&g_top);
    printf("verity - P9.0 UI spike\n\n");
    bool fat = fatInitDefault();
    printf("FAT: %s\n", fat ? "ok" : "not available");
    if (fat)
    {
        for (const char *root : {"sd:/", "fat:/"})
            scan(root, 2);
    }
    printf("Found %d save file(s)\n", g_file_count);

    List list;
    list.draw();
    keysSetRepeat(15, 4);

    int bx = 40, by = 100, vx = 1, vy = 1;  // top-screen badge
    int touch_start_y = -1, touch_last_y = -1, touch_scroll = 0;
    bool dragged = false;
    unsigned frame = 0;

    while (1)
    {
        swiWaitForVBlank();
        scanKeys();
        uint32_t down = keysDown();
        uint32_t rep = keysDownRepeat();
        uint32_t held = keysHeld();
        int before_sel = list.selected, before_scroll = list.scroll;

        if (rep & KEY_UP)
            list.selected--;
        if (rep & KEY_DOWN)
            list.selected++;
        if (rep & KEY_L)
            list.selected -= kListRows;
        if (rep & KEY_R)
            list.selected += kListRows;
        list.clamp();
        if ((down & KEY_A) && g_file_count > 0)
            open_file(g_files[list.selected]);

        // Touch: tap selects a row, tapping the selected row opens it, dragging scrolls.
        touchPosition tp;
        if (held & KEY_TOUCH)
        {
            touchRead(&tp);
            if (down & KEY_TOUCH)
            {
                touch_start_y = touch_last_y = tp.py;
                touch_scroll = list.scroll;
                dragged = false;
            }
            else if (touch_start_y >= 0)
            {
                int dy = tp.py - touch_start_y;
                if (abs(dy) > 6)
                    dragged = true;
                if (dragged)
                {
                    list.scroll = touch_scroll - dy / 8;
                    int max_scroll = g_file_count > kListRows ? g_file_count - kListRows : 0;
                    if (list.scroll < 0)
                        list.scroll = 0;
                    if (list.scroll > max_scroll)
                        list.scroll = max_scroll;
                    if (list.selected < list.scroll)
                        list.selected = list.scroll;
                    if (list.selected >= list.scroll + kListRows)
                        list.selected = list.scroll + kListRows - 1;
                }
                touch_last_y = tp.py;
            }
        }
        else if (touch_start_y >= 0)
        {
            if (!dragged)
            {
                int row = touch_last_y / 8 - kListTop;
                int i = list.scroll + row;
                if (row >= 0 && row < kListRows && i < g_file_count)
                {
                    if (i == list.selected)
                        open_file(g_files[i]);
                    list.selected = i;
                }
            }
            touch_start_y = -1;
        }

        if (list.selected != before_sel || list.scroll != before_scroll)
            list.draw();

        // Sprites: a bouncing badge on the top screen, the highlight bar and a badge on the bottom.
        bx += vx;
        by += vy;
        if (bx < 0 || bx > 256 - 32)
            vx = -vx;
        if (by < 64 || by > 192 - 32)
            vy = -vy;
        oamSet(&oamMain, 0, bx, by, 0, 0, SpriteSize_32x32, SpriteColorFormat_16Color,
               badge_main, -1, false, false, false, false, false);
        int bar_y = (kListTop + list.selected - list.scroll) * 8;
        for (int i = 0; i < 4; i++)
            oamSet(&oamSub, i, i * 64, bar_y, 1, 0, SpriteSize_64x32,
                   SpriteColorFormat_16Color, bar, -1, false, g_file_count == 0, false,
                   false, false);
        oamSet(&oamSub, 4, 220, 156, 1, 0, SpriteSize_32x32, SpriteColorFormat_16Color,
               badge_sub, -1, false, false, false, false, (frame / 30) & 1);
        oamUpdate(&oamMain);
        oamUpdate(&oamSub);

        consoleSelect(&g_top);
        printf("\x1b[23;0Hframe %u\x1b[K", frame++);

        if (down & KEY_START)
            break;
    }
    return 0;
}
