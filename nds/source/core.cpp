// SPDX-License-Identifier: GPL-3.0-only
#include "core.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <sys/stat.h>

#include <filesystem.h>
#include <nds.h>

#include "ui.h"

namespace core {

namespace {

Doc g_doc;
bool g_nitro_tried = false, g_nitro = false;
bool g_loaded[16] = {false};
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
    if (g_doc.info.generation == 4)
    {
        // Everything the Gen 4 legality context reads.
        attach(VERITY_BLOB_HGSS, "hgss.bin");
        attach(VERITY_BLOB_PT, "pt.bin");
        attach(VERITY_BLOB_DP, "dp.bin");
        attach(VERITY_BLOB_B2W2, "b2w2.bin");
        attach(VERITY_BLOB_MOVES4, "moves4.bin");
        attach(VERITY_BLOB_ENC_DPPT, "enc_dppt.bin");
        attach(VERITY_BLOB_NAMES4, "names4.bin");
    }
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
    ui::utf16_to_ascii(w, out, cap);
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

void remember(const char* p)
{
    char dir[64], file[80];
    snprintf(dir, sizeof(dir), "%s", config_path());
    char nds[64];
    snprintf(nds, sizeof(nds), "%.*s", (int)(strrchr(dir, '/') - dir), dir);
    mkdir(nds, 0777);
    mkdir(dir, 0777);
    snprintf(file, sizeof(file), "%s/last.txt", dir);
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
