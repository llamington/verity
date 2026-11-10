// SPDX-License-Identifier: GPL-3.0-only
#include "core.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <dirent.h>
#include <initializer_list>
#include <strings.h>
#include <sys/stat.h>
#include <unistd.h>

#include <filesystem.h>
#include <nds.h>

#include "ui.h"

namespace core {

namespace {

Doc g_doc;
bool g_nitro_tried = false, g_nitro = false;
bool g_loaded[24] = {false};
bool g_data_ok = false;
constexpr size_t kMaxSave = 1024 * 1024;

uint8_t* read_all(const char* path, size_t max, size_t* len)
{
    FILE* f = fopen(path, "rb");
    if (!f)
        return nullptr;
    fseek(f, 0, SEEK_END);
    long n = ftell(f);
    fseek(f, 0, SEEK_SET);
    if (n <= 0 || (size_t)n > max)
    {
        fclose(f);
        return nullptr;
    }
    uint8_t* buf = (uint8_t*)malloc(n);
    if (buf && fread(buf, 1, n, f) != (size_t)n)
    {
        free(buf);
        buf = nullptr;
    }
    fclose(f);
    *len = n;
    return buf;
}

bool attach(uint32_t kind, const char* file)
{
    if (g_loaded[kind])
        return true;
    char path[64];
    snprintf(path, sizeof(path), "nitro:/data/%s", file);
    size_t len = 0;
    uint8_t* buf = read_all(path, 2 * 1024 * 1024, &len);
    if (!buf)
        return false;
    if (verity_data_attach(kind, buf, len) != VERITY_OK)
    {
        free(buf);
        return false;
    }
    g_loaded[kind] = true;  // kept for the program's lifetime
    return true;
}

}  // namespace

Doc& doc() { return g_doc; }

bool open(const char* path, const char** err)
{
    size_t len = 0;
    uint8_t* buf = read_all(path, kMaxSave, &len);
    if (!buf)
    {
        *err = "cannot read file";
        return false;
    }
    verity_save_info info;
    if (verity_save_probe(buf, len, &info) != VERITY_OK)
    {
        free(buf);
        *err = "not a Gen 4/5 save";
        return false;
    }
    free(g_doc.buf);
    g_doc.buf = buf;
    g_doc.len = len;
    g_doc.info = info;
    g_doc.boxes = verity_box_count(buf, len);
    snprintf(g_doc.path, sizeof(g_doc.path), "%s", path);
    return true;
}

bool load_data_for(uint8_t kind)
{
    if (!g_nitro_tried)
    {
        g_nitro_tried = true;
        g_nitro = nitroFSInit(NULL);
    }
    if (!g_nitro)
        return g_data_ok = false;
    bool ok = attach(VERITY_BLOB_STRINGS, "strings_en.bin");
    switch (kind)
    {
        case 1: ok &= attach(VERITY_BLOB_DP, "dp.bin"); break;
        case 2: ok &= attach(VERITY_BLOB_PT, "pt.bin"); break;
        case 3: ok &= attach(VERITY_BLOB_HGSS, "hgss.bin"); break;
        case 4: ok &= attach(VERITY_BLOB_BW, "bw.bin"); break;
        case 5: ok &= attach(VERITY_BLOB_B2W2, "b2w2.bin"); break;
    }
    return g_data_ok = ok;
}

bool data_available() { return g_data_ok; }

verity_pkm_view slot(uint8_t box_index, uint8_t s)
{
    verity_pkm_view v;
    memset(&v, 0, sizeof(v));
    if (!g_doc.loaded() ||
        verity_slot_view(g_doc.buf, g_doc.len, verity_loc{box_index, s}, &v) != VERITY_OK)
        memset(&v, 0, sizeof(v));
    return v;
}

verity_legality_report legality(uint8_t box_index, uint8_t s, uint32_t* ms)
{
    verity_legality_report r;
    memset(&r, 0, sizeof(r));
    if (!g_doc.loaded())
        return r;
    ensure_save_data();
    cpuStartTiming(0);
    if (verity_legality(g_doc.buf, g_doc.len, verity_loc{box_index, s}, &r) != VERITY_OK)
        memset(&r, 0, sizeof(r));
    uint32_t ticks = cpuEndTiming();
    if (ms)
        *ms = timerTicks2msec(ticks);
    return r;
}

const char* check_name(uint8_t id)
{
    // legality::analysis4::CheckId order.
    static const char* names[] = {
        "CurrentMove", "Encounter", "Shiny",  "EC",        "PID",        "Gender",     "EVs",
        "Language",    "Nickname",  "Trainer", "IVs",      "Level",      "Ball",       "Form",
        "Egg",         "Misc",      "Fateful", "Ribbon",   "Ability",    "Evolution",  "Nature",
        "GameOrigin",  "HeldItem",  "Marking", "TrashBytes", "SlotType", "Memory",
    };
    return id < sizeof(names) / sizeof(names[0]) ? names[id] : "?";
}

void box_name(uint32_t b, char* out, int cap)
{
    uint16_t w[21];
    if (!g_doc.loaded() || verity_box_name(g_doc.buf, g_doc.len, b, w) != VERITY_OK)
    {
        snprintf(out, cap, "Box %lu", (unsigned long)b + 1);
        return;
    }
    ui::utf16_to_text(w, out, cap);
}

static const char* config_path()
{
    static char path[48];
    // The SD card on DSi, the flashcart otherwise; fat:/ also works under melonDS DLDI.
    struct stat st;
    const char* root = stat("sd:/", &st) == 0 ? "sd:" : "fat:";
    snprintf(path, sizeof(path), "%s/_nds/verity", root);
    return path;
}

static void ensure_config_dir()
{
    char dir[64], nds[64];
    snprintf(dir, sizeof(dir), "%s", config_path());
    snprintf(nds, sizeof(nds), "%.*s", (int)(strrchr(dir, '/') - dir), dir);
    mkdir(nds, 0777);
    mkdir(dir, 0777);
}

void remember(const char* p)
{
    char file[80];
    ensure_config_dir();
    snprintf(file, sizeof(file), "%s/last.txt", config_path());
    FILE* f = fopen(file, "w");
    if (!f)
        return;
    fprintf(f, "%s\n", p);
    fclose(f);
}

bool last(char* out, int cap)
{
    char file[80];
    snprintf(file, sizeof(file), "%s/last.txt", config_path());
    FILE* f = fopen(file, "r");
    if (!f)
        return false;
    bool ok = fgets(out, cap, f) != nullptr;
    fclose(f);
    if (ok)
        out[strcspn(out, "\r\n")] = 0;
    return ok && out[0];
}

// ---- ROM font ------------------------------------------------------------------------------
// The font comes from the user's own ROM: the first time a Gen 4/5 .nds is on the SD card, its
// font goes into font.vfnt and the ROM isn't needed again. A better ROM (Gen 5 covers kana,
// kanji and Hangul) replaces a cache built from a worse one. See docs/research/rom-font.md.

namespace {

uint8_t* g_font = nullptr;
size_t g_font_len = 0;
char g_font_game[5] = "";

bool read_rom(void* ctx, uint32_t offset, uint8_t* buf, uint32_t len)
{
    FILE* f = (FILE*)ctx;
    return fseek(f, (long)offset, SEEK_SET) == 0 && fread(buf, 1, len, f) == len;
}

int rom_rank(const char* path)
{
    FILE* f = fopen(path, "rb");
    if (!f)
        return 0;
    uint8_t code[4];
    bool ok = fseek(f, 0x0C, SEEK_SET) == 0 && fread(code, 1, 4, f) == 4;
    fclose(f);
    return ok ? verity_rom_font_rank(code) : 0;
}

// The best ROM overall (font and icons) and the best of each generation (move stats, which differ
// between generations).
struct RomScan
{
    char best[192] = "";
    int rank = 0;
    char gen[2][192] = {"", ""};  // Gen 4, Gen 5
    int gen_rank[2] = {0, 0};
};

void scan_roms(const char* dir, int depth, RomScan* scan)
{
    DIR* d = opendir(dir);
    if (!d)
        return;
    struct dirent* e;
    while ((e = readdir(d)) != nullptr && !(scan->rank == 3 && scan->gen_rank[0] > 0))
    {
        if (e->d_name[0] == '.')
            continue;
        char path[192];
        int n = snprintf(path, sizeof(path), "%s%s%s", dir, dir[strlen(dir) - 1] == '/' ? "" : "/",
                         e->d_name);
        if (n <= 0 || n >= (int)sizeof(path))
            continue;
        const char* dot = strrchr(e->d_name, '.');
        if (e->d_type == DT_DIR)
        {
            if (depth > 0)
                scan_roms(path, depth - 1, scan);
        }
        else if (dot && strcasecmp(dot, ".nds") == 0)
        {
            int r = rom_rank(path);
            if (r > scan->rank)
            {
                scan->rank = r;
                snprintf(scan->best, sizeof(scan->best), "%s", path);
            }
            int g = r == 3 ? 1 : 0;
            if (r > 0 && r > scan->gen_rank[g])
            {
                scan->gen_rank[g] = r;
                snprintf(scan->gen[g], sizeof(scan->gen[g]), "%s", path);
            }
        }
    }
    closedir(d);
}

uint8_t* g_moves[2] = {nullptr, nullptr};  // move caches for Gen 4 and Gen 5 saves
size_t g_moves_len[2] = {0, 0};

void load_moves(int g, const char* file)
{
    free(g_moves[g]);
    size_t len = 0;
    uint8_t* b = read_all(file, 96 * 1024, &len);  // stats and descriptions (about 50 KB)
    if (b && verity_moves_generation(b, len) != 4 + g)
    {
        free(b);
        b = nullptr;
    }
    g_moves[g] = b;
    g_moves_len[g] = b ? len : 0;
}

void use_font(uint8_t* buf, size_t len)
{
    free(g_font);
    g_font = buf;
    g_font_len = len;
    memcpy(g_font_game, buf + 12, 4);
    g_font_game[4] = 0;
    ui::set_rom_font(g_font, g_font_len);
}

}  // namespace

namespace {

FILE* g_icon_file = nullptr;  // icons.vicn, kept open: tiles are read per icon
uint8_t* g_icon_head = nullptr;
size_t g_icon_head_len = 0;

int file_rank(const uint8_t* code) { return code ? verity_rom_font_rank(code) : 0; }

// Writes a cache built by verity-ffi and frees it; true if the file was written whole.
bool write_cache(const char* file, uint8_t* built, size_t len)
{
    ensure_config_dir();
    FILE* out = fopen(file, "wb");
    bool ok = out && fwrite(built, 1, len, out) == len;
    if (out)
        ok = fclose(out) == 0 && ok;
    verity_rom_free(built, len);
    if (!ok)
        remove(file);
    return ok;
}

// Opens icons.vicn and keeps its head (palettes and index) in memory; returns its rank.
int open_icons(const char* file)
{
    if (g_icon_file)
        fclose(g_icon_file);
    free(g_icon_head);
    g_icon_file = nullptr;
    g_icon_head = nullptr;
    g_icon_head_len = 0;
    FILE* f = fopen(file, "rb");
    uint8_t h[16];
    if (!f || fread(h, 1, 16, f) != 16)
    {
        if (f)
            fclose(f);
        return 0;
    }
    size_t n = verity_icons_head_len(h, 16);
    uint8_t* head = n >= 16 && n < 64 * 1024 ? (uint8_t*)malloc(n) : nullptr;
    if (!head || fseek(f, 0, SEEK_SET) != 0 || fread(head, 1, n, f) != n ||
        verity_icons_palette_count(head, n) == 0)
    {
        free(head);
        fclose(f);
        return 0;
    }
    g_icon_file = f;
    g_icon_head = head;
    g_icon_head_len = n;
    return file_rank(head + 12);
}

// ribbons.vrib, kept whole in memory (about 28 KB).
uint8_t* g_ribbons = nullptr;
size_t g_ribbons_len = 0;

// Loads ribbons.vrib; returns its rank (0 for none).
int load_ribbons(const char* file)
{
    free(g_ribbons);
    size_t len = 0;
    uint8_t* b = read_all(file, 64 * 1024, &len);
    if (b && !verity_ribbons_check(b, len))
    {
        free(b);
        b = nullptr;
    }
    g_ribbons = b;
    g_ribbons_len = b ? len : 0;
    return b ? file_rank(b + 12) : 0;
}

// Held item icon caches for Gen 4 and Gen 5 saves (items4.vitm, items5.vitm), kept open like the
// box icons: the head (index) is in memory, colours and tiles are read per icon.
FILE* g_item_file[2] = {nullptr, nullptr};
uint8_t* g_item_head[2] = {nullptr, nullptr};
size_t g_item_head_len[2] = {0, 0};

void open_items(int g, const char* file)
{
    if (g_item_file[g])
        fclose(g_item_file[g]);
    free(g_item_head[g]);
    g_item_file[g] = nullptr;
    g_item_head[g] = nullptr;
    g_item_head_len[g] = 0;
    FILE* f = fopen(file, "rb");
    uint8_t h[16];
    size_t n = f && fread(h, 1, 16, f) == 16 ? verity_items_head_len(h, 16) : 0;
    uint8_t* head = n >= 16 && n < 16 * 1024 ? (uint8_t*)malloc(n) : nullptr;
    if (!head || fseek(f, 0, SEEK_SET) != 0 || fread(head, 1, n, f) != n ||
        verity_items_generation(head, n) != 4 + g)
    {
        free(head);
        if (f)
            fclose(f);
        return;
    }
    g_item_file[g] = f;
    g_item_head[g] = head;
    g_item_head_len[g] = n;
}

}  // namespace

void rom_assets_init()
{
    char font_file[80], icon_file[80], ribbon_file[80];
    snprintf(font_file, sizeof(font_file), "%s/font.vfnt", config_path());
    snprintf(icon_file, sizeof(icon_file), "%s/icons.vicn", config_path());
    snprintf(ribbon_file, sizeof(ribbon_file), "%s/ribbons.vrib", config_path());
    size_t len = 0;
    uint8_t* cached = read_all(font_file, 512 * 1024, &len);
    int font_rank = 0;
    if (cached && verity_font_check(cached, len))
        font_rank = file_rank(cached + 12);
    else
    {
        free(cached);
        cached = nullptr;
    }
    int icon_rank = open_icons(icon_file);
    int ribbon_rank = load_ribbons(ribbon_file);
    char move_file[2][80], item_file[2][80];
    for (int g = 0; g < 2; g++)
    {
        snprintf(move_file[g], sizeof(move_file[g]), "%s/moves%d.vmov", config_path(), 4 + g);
        load_moves(g, move_file[g]);
        snprintf(item_file[g], sizeof(item_file[g]), "%s/items%d.vitm", config_path(), 4 + g);
        open_items(g, item_file[g]);
    }
    if (font_rank < 3 || icon_rank < 3 || ribbon_rank < 3 || !g_moves[0] || !g_moves[1] || !g_item_head[0] ||
        !g_item_head[1])
    {
        RomScan scan;
        for (const char* root : {"sd:/", "fat:/"})
            scan_roms(root, 3, &scan);
        for (int g = 0; g < 2; g++)
        {
            if ((g_moves[g] && g_item_head[g]) || !scan.gen_rank[g])
                continue;
            FILE* f = fopen(scan.gen[g], "rb");
            if (!f)
                continue;
            size_t n = 0;
            if (!g_moves[g])
            {
                uint8_t* built = verity_rom_build_moves_cache(read_rom, f, &n);
                if (built && write_cache(move_file[g], built, n))
                    load_moves(g, move_file[g]);
            }
            if (!g_item_head[g])
            {
                uint8_t* built = verity_rom_build_items_cache(read_rom, f, &n);
                if (built && write_cache(item_file[g], built, n))
                    open_items(g, item_file[g]);
            }
            fclose(f);
        }
        const char* best = scan.best;
        int best_rank = scan.rank;
        if (best_rank > font_rank || best_rank > icon_rank || best_rank > ribbon_rank)
        {
            FILE* f = fopen(best, "rb");
            if (f && best_rank > font_rank)
            {
                size_t n = 0;
                uint8_t* built = verity_rom_build_font_cache(read_rom, f, &n);
                if (built && write_cache(font_file, built, n))
                {
                    free(cached);
                    cached = read_all(font_file, 512 * 1024, &len);
                    if (!cached || !verity_font_check(cached, len))
                    {
                        free(cached);
                        cached = nullptr;
                    }
                }
            }
            if (f && best_rank > icon_rank)
            {
                if (g_icon_file)
                {
                    fclose(g_icon_file);
                    g_icon_file = nullptr;
                }
                size_t n = 0;
                uint8_t* built = verity_rom_build_icon_cache(read_rom, f, &n);
                if (built)
                    write_cache(icon_file, built, n);
                open_icons(icon_file);
            }
            if (f && best_rank > ribbon_rank)
            {
                size_t n = 0;
                uint8_t* built = verity_rom_build_ribbon_cache(read_rom, f, &n);
                if (built && write_cache(ribbon_file, built, n))
                    load_ribbons(ribbon_file);
            }
            if (f)
                fclose(f);
        }
    }
    if (cached)
        use_font(cached, len);
    if (g_icon_head)
    {
        uint16_t pal[4][16];
        int count = verity_icons_palette_count(g_icon_head, g_icon_head_len);
        for (int p = 0; p < count && p < 4; p++)
            verity_icons_palette(g_icon_head, g_icon_head_len, (uint8_t)p, pal[p]);
        ui::set_icon_palettes(pal, count < 4 ? count : 4);
    }
}

bool icon_tiles(uint16_t species, uint8_t form, bool female, bool egg, uint8_t* tiles, uint8_t* palette)
{
    uint32_t at = 0;
    if (!g_icon_file ||
        !verity_icons_find(g_icon_head, g_icon_head_len, species, form, female, egg, palette, &at))
        return false;
    return fseek(g_icon_file, (long)at, SEEK_SET) == 0 && fread(tiles, 1, 512, g_icon_file) == 512;
}

bool ribbon_icon(int index, uint8_t* tiles, uint16_t* colors)
{
    return index >= 0 && index < 80 &&
           verity_ribbon_icon(g_ribbons, g_ribbons_len, (uint8_t)index, tiles, colors);
}

bool item_icon(uint8_t generation, uint16_t item, uint8_t* tiles, uint16_t* colors)
{
    int g = generation == 5 ? 1 : 0;
    uint32_t pal_at = 0, at = 0;
    FILE* f = g_item_file[g];
    return f && verity_items_find(g_item_head[g], g_item_head_len[g], item, &pal_at, &at) &&
           fseek(f, (long)pal_at, SEEK_SET) == 0 && fread(colors, 2, 16, f) == 16 &&
           fseek(f, (long)at, SEEK_SET) == 0 && fread(tiles, 1, 512, f) == 512;
}

bool move_info(uint8_t generation, uint16_t mv, verity_move_stats* out)
{
    int g = generation == 5 ? 1 : 0;
    return verity_move_info(generation, mv, g_moves[g], g_moves_len[g], out);
}

bool move_description(uint8_t generation, uint16_t mv, char* out, size_t cap)
{
    int g = generation == 5 ? 1 : 0;
    return verity_move_description(generation, mv, g_moves[g], g_moves_len[g], out, cap);
}

const char* font_game() { return g_font_game; }

bool ensure_gen4_data()
{
    if (!g_nitro_tried)
        load_data_for(g_doc.info.kind);
    // Everything the Gen 4 legality context reads.
    bool ok = attach(VERITY_BLOB_HGSS, "hgss.bin");
    ok &= attach(VERITY_BLOB_PT, "pt.bin");
    ok &= attach(VERITY_BLOB_DP, "dp.bin");
    ok &= attach(VERITY_BLOB_B2W2, "b2w2.bin");
    ok &= attach(VERITY_BLOB_MOVES4, "moves4.bin");
    ok &= attach(VERITY_BLOB_ENC_DPPT, "enc_dppt.bin");
    ok &= attach(VERITY_BLOB_NAMES4, "names4.bin");
    ok &= attach(VERITY_BLOB_ENC_HGSS, "enc_hgss.bin");
    ok &= attach(VERITY_BLOB_GIFTS4, "gifts4.bin");
    attach(VERITY_BLOB_G3_ORIGIN, "g3_origin.bin"); // Optional: Pal Park Pokémon stay unchecked.
    return ok;
}

bool ensure_gen5_data()
{
    if (!g_nitro_tried)
        load_data_for(g_doc.info.kind);
    bool ok = attach(VERITY_BLOB_BW, "bw.bin");
    ok &= attach(VERITY_BLOB_B2W2, "b2w2.bin");
    ok &= attach(VERITY_BLOB_MOVES5, "moves5.bin");
    ok &= attach(VERITY_BLOB_ENC_BW, "enc_bw.bin");
    ok &= attach(VERITY_BLOB_ENC_B2W2, "enc_b2w2.bin");
    ok &= attach(VERITY_BLOB_ENC_DR, "enc_dr.bin");
    ok &= attach(VERITY_BLOB_NAMES5, "names5.bin");
    attach(VERITY_BLOB_GIFTS5, "gifts5.bin"); // Optional: Wonder Card Pokémon stay unchecked.
    // Poké Transfer: Gen 3/4 origins. Optional, they stay unchecked without it.
    if (ok)
        ensure_gen4_data();
    return ok;
}

bool ensure_save_data()
{
    return g_doc.info.generation == 5 ? ensure_gen5_data() : ensure_gen4_data();
}

bool ensure_events_data()
{
    if (!g_nitro_tried)
        load_data_for(g_doc.info.kind);
    if (g_doc.info.generation == 5)
    {
        bool ok5 = attach(VERITY_BLOB_GIFTS5, "gifts5.bin");
        ok5 &= attach(VERITY_BLOB_EVENTS5, "events5.bin");
        return ok5;
    }
    bool ok = attach(VERITY_BLOB_GIFTS4, "gifts4.bin");
    ok &= attach(VERITY_BLOB_EVENTS4, "events4.bin");
    return ok;
}

uint8_t wizard_version()
{
    // D/P saves get Diamond slots, Pt saves Platinum, HG/SS saves HeartGold (the save does not
    // record which of each pair it is; version exclusives differ).
    switch (g_doc.info.kind)
    {
        case 1: return 10;
        case 3: return 7;
        case 4: return g_doc.info.game_version == 21 ? 21 : 20;
        case 5: return g_doc.info.game_version == 23 ? 23 : 22;
        default: return 12;
    }
}

static bool write_file(const char* path, const uint8_t* buf, size_t len)
{
    FILE* f = fopen(path, "wb");
    if (!f)
        return false;
    bool ok = fwrite(buf, 1, len, f) == len;
    ok &= fflush(f) == 0;
    ok &= fsync(fileno(f)) == 0;
    ok &= fclose(f) == 0;
    return ok;
}

bool save_to_disk(const char** message)
{
    if (!g_doc.loaded())
    {
        *message = "no save open";
        return false;
    }
    // The validator reads the game data, which no earlier screen may have needed.
    ensure_save_data();
    verity_save_finalize(g_doc.buf, g_doc.len);
    verity_save_check_result c;
    if (verity_save_check(g_doc.buf, g_doc.len, &c) != VERITY_OK || c.issue != 0)
    {
        static char why[48];
        if (c.issue == 3)
            snprintf(why, sizeof(why), "unsafe Pokemon at %s %u", c.box_index == 0xFF ? "party" : "box",
                     (c.box_index == 0xFF ? c.slot : c.box_index * 30 + c.slot) + 1);
        else
            snprintf(why, sizeof(why), "validator refused (issue %u)", c.issue);
        *message = why;
        return false;
    }
    char tmp[200], bak[3][200];
    snprintf(tmp, sizeof(tmp), "%s.tmp", g_doc.path);
    for (int i = 0; i < 3; i++)
        snprintf(bak[i], sizeof(bak[i]), "%s.bak%d", g_doc.path, i + 1);
    if (!write_file(tmp, g_doc.buf, g_doc.len))
    {
        remove(tmp);
        *message = "could not write temp file";
        return false;
    }
    // Rotate: bak2 -> bak3, bak1 -> bak2, current file -> bak1, temp -> current.
    remove(bak[2]);
    rename(bak[1], bak[2]);
    rename(bak[0], bak[1]);
    if (rename(g_doc.path, bak[0]) != 0 || rename(tmp, g_doc.path) != 0)
    {
        *message = "could not replace the file (temp kept)";
        return false;
    }
    size_t len = 0;
    uint8_t* back = read_all(g_doc.path, kMaxSave, &len);
    bool same = back && len == g_doc.len && memcmp(back, g_doc.buf, len) == 0;
    free(back);
    if (!same)
    {
        *message = "verify failed: restore from .bak1";
        return false;
    }
    g_doc.dirty = false;
    verity_save_probe(g_doc.buf, g_doc.len, &g_doc.info);
    *message = "saved (backup: .bak1)";
    return true;
}

uint8_t* read_save_file(const char* path, size_t* len)
{
    return read_all(path, kMaxSave, len);
}

const char* game_name(uint8_t kind)
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

}  // namespace core
