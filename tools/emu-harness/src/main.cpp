// SPDX-License-Identifier: GPL-3.0-only
//
// verity emu-harness: runs melonDS headless from JSON scripts (PLAN §7). Each script is a test.
//
//   emu-harness script.json [more.json ...] [--out DIR] [--update] [--junit FILE]
//               [--needles DIR] [--verbose]
//
// --needles uses DIR for every script's needles (screens that differ by setup, such as with ROM
// fonts, keep their own set).
// --update rewrites needles that don't match (and writes missing ones) instead of failing.
// --junit writes a JUnit XML report, one test case per script.
//
// Script:
//   {
//     "rom": "nds/verity.nds",            // relative paths resolve against the script's dir
//     "save": "path.sav",                 // optional cartridge save to load
//     "bios_dir": "...",                  // optional bios7.bin, bios9.bin, firmware.bin
//                                         // (default: melonDS FreeBIOS + generated firmware)
//     "sd_dir": "sd", "sd_image": "sd.img",  // optional DLDI SD card synced from a folder
//     "sd_fresh": true,                   // instead of sd_image: a private copy of sd_dir per run
//     "rtc": [2026, 1, 1, 12, 0, 0],      // fixed date/time (default shown)
//     "direct_boot": true,                // skip the DS menu (default; forced with FreeBIOS)
//     "state": "boot.state",              // optional: start from a save state (same ROM)
//     "elf": "nds/build/verity.elf",      // optional: symbols for "addr" in RAM steps
//     "needles": "needles",               // optional: directory of needles (see below)
//     "steps": [
//       {"press": ["A"], "frames": 2},          // hold keys, then release for one frame
//       {"hold": ["Down"], "frames": 30},       // hold without the release frame
//       {"touch": [x, y], "frames": 2},         // bottom-screen touch
//       {"drag": [[x, y], [x, y]], "frames": 20},
//       {"wait": 60},
//       {"wait_until": {"screen_hash": "…", "screen": "top"}, "timeout": 600},
//       {"wait_until": {"ram": {"addr": 0x02000000, "value": 1, "mask": 0xFF}}, "timeout": 600},
//       {"assert_ram": {"addr": "symbol+4", "value": 1, "size": 1}},  // size 1, 2 or 4 (default)
//       {"assert_ram": {"addr": "symbol", "text": "editor"}},  // a zero-terminated string
//       {"assert_screen": "tag"},               // a needle with this tag matches now
//       {"wait_screen": "tag", "timeout": 600}, // run until a needle with this tag matches
//       {"wait_image": "ok.png", "screen": "bottom", "timeout": 600},  // exact match anywhere
//       {"tap_image": "ok.png", "frames": 2, "timeout": 600},         // wait, then touch its centre
//       {"screenshot": "name.png"},             // both screens stacked, 256x384
//       {"hash": "label"},                      // log the screen hashes
//       {"save_state": "boot.state"},           // written to the output directory
//       {"dump_save": "out.sav"}
//     ]
//   }
//
// Needles (after openQA): <name>.png, both screens stacked (256x384, like screenshots), and
// <name>.json: {"tags": ["editor"], "areas": [{"x": 0, "y": 0, "w": 256, "h": 16}], "match": 1.0}.
// Areas are in the stacked image (the bottom screen starts at y 192) and default to all of it;
// "exclude" lists rectangles inside them to ignore (a timing, a clock);
// "match" is the share of area pixels that must be equal (default 1.0: emulation is
// deterministic, so exact matches hold). Any needle with the tag can satisfy a step. On a
// mismatch the harness writes the actual screens and a diff (differing pixels in red) next to the
// run's outputs; with --update it rewrites the closest needle's image instead. A tag with no needle
// gets one written (whole screen), and the step fails unless --update is given.
//
// Template images (wait_image, tap_image) are small PNGs, relative to the script; pixels with
// alpha 0 match anything. A save state holds the console, the cartridge, the displayed screens and
// the frame number (a run started from it continues exactly where the saving run was), but not the
// SD card image.
//
// Two consoles on local wireless: {"instances": [{...}, {...}]}, each object in the format above
// (with bios_dir; "direct_boot": false boots the DS menu, needed to receive Download Play). Each
// instance runs its own steps on its own thread, and keeps running with no input once they end,
// until the other one is done. {"sync": "name", "timeout": 60000} waits for the other instance to
// reach the same name. Output goes to --out/inst0 and inst1. Linked runs are not deterministic:
// melonDS's LocalMP waits on real-time semaphores.
//
// Outputs in --out (default: <script>.out/; <out>/<script name>/ with several scripts):
// screenshots, save dumps, failure images, and run.jsonl (one JSON object per step with the frame
// number and screen hashes). Exit status 0 when every step of every script passes, 1 when a step
// fails, 2 on an error (bad script, missing file).

#include <algorithm>
#include <atomic>
#include <cctype>
#include <chrono>
#include <cstdio>
#include <filesystem>
#include <cstring>
#include <fstream>
#include <map>
#include <memory>
#include <mutex>
#include <optional>
#include <sstream>
#include <string>
#include <thread>
#include <vector>

#include "Args.h"
#include "LocalMP.h"
#include "NDS.h"
#include "NDSCart.h"
#include "Platform.h"
#include "Savestate.h"
#include "elf.h"
#include "image.h"
#include "json.h"

using namespace melonDS;
namespace fs = std::filesystem;

namespace melonDS::Platform {
extern bool g_verbose;
void set_local_mp(LocalMP* mp);
}

namespace {

struct Options
{
    bool update = false;
    std::string needles;  // --needles: overrides every script's "needles"
};
Options g_opts;

// State shared by the two instances of a linked run.
struct Shared
{
    std::atomic<u64> frames[2]{};
    std::atomic<bool> active[2]{};
    std::atomic<bool> done[2]{};
    std::mutex lock;
    std::map<std::string, int> arrivals;
};

// Unthrottled threads would drift apart in emulated time, so an instance that gets this many frames
// ahead pauses briefly. The pause must stay bounded: a paused console can't answer the other's
// wireless commands, and each unanswered command costs the sender a full receive timeout.
constexpr u64 kMaxFrameLead = 8;
constexpr int kMaxLeadWaitUs = 2000;

struct Needle
{
    std::string name;  // file stem in the needles directory
    std::vector<std::string> tags;
    std::vector<image::Rect> areas;
    std::vector<image::Rect> exclude;  // ignored inside areas (timings, clocks)
    double match = 1.0;
    image::Image img;
};

struct Ctx
{
    int inst = 0;  // passed as the core's userdata; the Platform MP hooks read it
    Shared* shared = nullptr;
    std::unique_ptr<NDS> nds;
    std::string base;  // script directory
    std::string out;
    u64 frame = 0;
    FILE* log = nullptr;
    bool failed = false;
    std::vector<std::string> failures;  // one line per failed step, for reports
    std::map<std::string, elf::Symbol> symbols;
    std::string needles_dir;
    std::vector<Needle> needles;
};

std::string join(const std::string& dir, const std::string& p)
{
    if (p.empty() || p[0] == '/')
        return p;
    return dir + "/" + p;
}

std::vector<u8> read_file(const std::string& path)
{
    std::ifstream f(path, std::ios::binary);
    if (!f)
        throw std::runtime_error("cannot read " + path);
    return std::vector<u8>(std::istreambuf_iterator<char>(f), {});
}

void write_file(const std::string& path, const void* data, size_t len)
{
    std::ofstream f(path, std::ios::binary);
    if (!f || !f.write(static_cast<const char*>(data), (std::streamsize)len))
        throw std::runtime_error("cannot write " + path);
}

json::Value read_json(const std::string& path)
{
    std::ifstream f(path);
    if (!f)
        throw std::runtime_error("cannot read " + path);
    std::stringstream ss;
    ss << f.rdbuf();
    return json::parse(ss.str());
}

u32 key_bit(const std::string& k)
{
    static const char* names[] = {"A", "B", "Select", "Start", "Right", "Left",
                                  "Up", "Down", "R", "L", "X", "Y"};
    for (int i = 0; i < 12; i++)
        if (k == names[i])
            return 1u << i;
    throw std::runtime_error("unknown key " + k);
}

u32 key_mask(const json::Value& keys)
{
    u32 m = 0;
    if (keys.kind == json::Value::String)
        return key_bit(keys.s);
    for (auto& k : keys.a)
        m |= key_bit(k.s);
    return m;
}

const u32* screen(Ctx& c, int which)
{
    return c.nds->GPU.Framebuffer[c.nds->GPU.FrontBuffer][which].get();
}

image::Image screens(Ctx& c)
{
    return image::stack_screens(screen(c, 0), screen(c, 1));
}

std::string hash_screen(Ctx& c, int which)
{
    u64 h = 0xcbf29ce484222325ull;
    const u32* px = screen(c, which);
    for (int i = 0; i < 256 * 192; i++)
    {
        u32 v = px[i] & 0x00FFFFFF;
        for (int b = 0; b < 3; b++)
        {
            h ^= (v >> (8 * b)) & 0xFF;
            h *= 0x100000001b3ull;
        }
    }
    char buf[17];
    snprintf(buf, sizeof(buf), "%016llx", (unsigned long long)h);
    return buf;
}

std::string esc(const std::string& s)
{
    std::string o;
    for (char ch : s)
    {
        if (ch == '"' || ch == '\\')
            o += '\\';
        o += ch;
    }
    return o;
}

void run_frame(Ctx& c, u32 keys, std::optional<std::pair<int, int>> touch)
{
    if (c.shared)
    {
        int other = 1 - c.inst;
        for (int us = 0; us < kMaxLeadWaitUs && c.shared->active[other]
                         && c.frame > c.shared->frames[other] + kMaxFrameLead; us += 50)
            std::this_thread::sleep_for(std::chrono::microseconds(50));
    }
    c.nds->SetKeyMask(~keys & 0xFFF);
    if (touch)
        c.nds->TouchScreen(touch->first, touch->second);
    else
        c.nds->ReleaseScreen();
    c.nds->RunFrame();
    c.frame++;
    if (c.shared)
        c.shared->frames[c.inst] = c.frame;
}

// ---- RAM ----------------------------------------------------------------------------------------

// A number, or "symbol" / "symbol+offset" (decimal or 0x hex) looked up in the script's ELF.
u32 address(Ctx& c, const json::Value& v)
{
    if (v.kind != json::Value::String)
        return (u32)v.i();
    std::string name = v.s;
    long long offset = 0;
    size_t plus = name.find('+');
    if (plus != std::string::npos)
    {
        offset = std::stoll(name.substr(plus + 1), nullptr, 0);
        name = name.substr(0, plus);
    }
    if (c.symbols.empty())
        throw std::runtime_error("symbol " + name + " needs \"elf\" in the script");
    auto it = c.symbols.find(name);
    if (it == c.symbols.end())
    {
        std::string near;
        int n = 0;
        for (auto& [k, _] : c.symbols)
            if (k.find(name) != std::string::npos && n++ < 5)
                near += (near.empty() ? "" : ", ") + k;
        throw std::runtime_error("no symbol " + name + (near.empty() ? "" : " (did you mean " + near + "?)"));
    }
    return it->second.addr + (u32)offset;
}

u32 read_ram(Ctx& c, const json::Value& r)
{
    u32 addr = address(c, r["addr"]);
    int size = r.has("size") ? (int)r["size"].i() : 4;
    switch (size)
    {
    case 1: return c.nds->ARM9Read8(addr);
    case 2: return c.nds->ARM9Read16(addr);
    case 4: return c.nds->ARM9Read32(addr);
    default: throw std::runtime_error("size must be 1, 2 or 4");
    }
}

// The zero-terminated string at r["addr"] (at most 64 bytes).
std::string read_text(Ctx& c, const json::Value& r)
{
    u32 addr = address(c, r["addr"]);
    std::string out;
    for (u32 i = 0; i < 64; i++)
    {
        char ch = (char)c.nds->ARM9Read8(addr + i);
        if (!ch)
            break;
        out += ch;
    }
    return out;
}

bool ram_matches(Ctx& c, const json::Value& r)
{
    if (r.has("text"))
        return read_text(c, r) == r["text"].s;
    u32 value = (u32)r["value"].i();
    u32 mask = r.has("mask") ? (u32)r["mask"].i() : 0xFFFFFFFFu;
    return (read_ram(c, r) & mask) == (value & mask);
}

bool screen_matches(Ctx& c, const json::Value& w)
{
    std::string which = w.has("screen") ? w["screen"].s : "top";
    std::string want = w["screen_hash"].s;
    if (which == "top")
        return hash_screen(c, 0) == want;
    if (which == "bottom")
        return hash_screen(c, 1) == want;
    return hash_screen(c, 0) + hash_screen(c, 1) == want;
}

// ---- Needles ------------------------------------------------------------------------------------

image::Rect full_screens() { return {0, 0, 256, 384}; }

void load_needles(Ctx& c)
{
    c.needles.clear();
    if (c.needles_dir.empty() || !fs::exists(c.needles_dir))
        return;
    std::vector<fs::path> files;
    for (auto& e : fs::directory_iterator(c.needles_dir))
        if (e.path().extension() == ".json")
            files.push_back(e.path());
    std::sort(files.begin(), files.end());
    for (auto& p : files)
    {
        json::Value j = read_json(p.string());
        Needle n;
        n.name = p.stem().string();
        for (auto& t : j["tags"].a)
            n.tags.push_back(t.s);
        if (j.has("areas"))
            for (auto& a : j["areas"].a)
                n.areas.push_back({(int)a["x"].i(), (int)a["y"].i(), (int)a["w"].i(), (int)a["h"].i()});
        if (n.areas.empty())
            n.areas.push_back(full_screens());
        if (j.has("exclude"))
            for (auto& a : j["exclude"].a)
                n.exclude.push_back({(int)a["x"].i(), (int)a["y"].i(), (int)a["w"].i(), (int)a["h"].i()});
        if (j.has("match"))
            n.match = j["match"].n;
        n.img = image::read_png(join(c.needles_dir, n.name + ".png"));
        c.needles.push_back(std::move(n));
    }
}

struct Match
{
    const Needle* best = nullptr;  // the closest needle with the tag
    double score = 0;
    bool ok = false;
};

Match match_tag(Ctx& c, const image::Image& now, const std::string& tag)
{
    Match m;
    for (auto& n : c.needles)
    {
        if (std::find(n.tags.begin(), n.tags.end(), tag) == n.tags.end())
            continue;
        double s = image::similarity(now, n.img, n.areas, n.exclude);
        if (!m.best || s > m.score)
        {
            m.best = &n;
            m.score = s;
        }
        if (s >= n.match)
        {
            m.best = &n;
            m.score = s;
            m.ok = true;
            return m;
        }
    }
    return m;
}

std::string needle_file_name(const std::string& tag)
{
    std::string s;
    for (char ch : tag)
        s += (isalnum((unsigned char)ch) || ch == '-' || ch == '_') ? ch : '-';
    return s;
}

// After a failed screen check: write the evidence, and with --update fix the needle. Returns
// whether the step now passes, and adds details to `extra` for the log.
bool settle_needle(Ctx& c, size_t i, const std::string& tag, const image::Image& now, const Match& m,
                   std::string& extra)
{
    char score[32];
    snprintf(score, sizeof(score), "%.4f", m.score);
    if (!m.best)
    {
        if (c.needles_dir.empty())
            throw std::runtime_error("screen checks need \"needles\" in the script");
        fs::create_directories(c.needles_dir);
        std::string stem = needle_file_name(tag);
        image::write_png(now, join(c.needles_dir, stem + ".png"));
        std::ofstream(join(c.needles_dir, stem + ".json"))
            << "{\"tags\": [\"" << esc(tag) << "\"], \"areas\": [{\"x\": 0, \"y\": 0, \"w\": 256, \"h\": 384}]}\n";
        load_needles(c);
        extra += ",\"needle\":\"" + esc(stem) + "\",\"created\":true";
        if (!g_opts.update)
            fprintf(stderr, "step %zu: no needle tagged %s; wrote %s/%s.png (review it, then rerun)\n",
                    i, tag.c_str(), c.needles_dir.c_str(), stem.c_str());
        return g_opts.update;
    }
    extra += ",\"needle\":\"" + esc(m.best->name) + "\",\"similarity\":" + score;
    if (g_opts.update)
    {
        image::write_png(now, join(c.needles_dir, m.best->name + ".png"));
        extra += ",\"updated\":true";
        load_needles(c);
        return true;
    }
    std::string stem = c.out + "/step" + std::to_string(i) + "-" + needle_file_name(tag);
    image::write_png(now, stem + "-actual.png");
    image::write_png(image::diff(now, m.best->img, m.best->areas, m.best->exclude), stem + "-diff.png");
    extra += ",\"actual\":\"" + esc(stem + "-actual.png") + "\"";
    return false;
}

// ---- Templates ----------------------------------------------------------------------------------

image::Rect search_area(const json::Value& st, const char* fallback)
{
    std::string which = st.has("screen") ? st["screen"].s : fallback;
    if (which == "top")
        return {0, 0, 256, 192};
    if (which == "bottom")
        return {0, 192, 256, 192};
    if (which == "both")
        return full_screens();
    throw std::runtime_error("screen must be top, bottom or both");
}

// Runs frames until `tmpl` appears in `area` (at most `timeout`); its position in the stacked image.
std::optional<std::pair<int, int>> wait_for_image(Ctx& c, const image::Image& tmpl, image::Rect area,
                                                  long long timeout)
{
    for (long long f = 0;; f++)
    {
        if (auto at = image::find(screens(c), tmpl, area))
            return at;
        if (f >= timeout)
            return std::nullopt;
        run_frame(c, 0, std::nullopt);
    }
}

// ---- Save states --------------------------------------------------------------------------------

// A harness state file: this header, the frame number, both screens as displayed, then melonDS's
// state. melonDS's state alone doesn't hold the displayed frame, so a script started from it would
// see blank screens (and, a frame later, stale top lines) instead of what the saving run saw.
constexpr char kStateMagic[8] = {'E', 'M', 'U', 'H', 'S', 'T', '0', '1'};
constexpr size_t kScreenWords = 256 * 192;

bool save_state(Ctx& c, const std::string& path)
{
    Savestate state;
    if (state.Error || !c.nds->DoSavestate(&state) || state.Error)
        return false;
    std::ofstream f(path, std::ios::binary);
    f.write(kStateMagic, sizeof(kStateMagic));
    u64 frame = c.frame;
    f.write(reinterpret_cast<const char*>(&frame), sizeof(frame));
    for (int s = 0; s < 2; s++)
        f.write(reinterpret_cast<const char*>(screen(c, s)), kScreenWords * sizeof(u32));
    f.write(static_cast<const char*>(state.Buffer()), state.Length());
    if (!f)
        throw std::runtime_error("cannot write " + path);
    return true;
}

void load_state(Ctx& c, const std::string& path)
{
    auto buf = read_file(path);
    size_t head = sizeof(kStateMagic) + sizeof(u64) + 2 * kScreenWords * sizeof(u32);
    if (buf.size() <= head || memcmp(buf.data(), kStateMagic, sizeof(kStateMagic)) != 0)
        throw std::runtime_error(path + " is not a harness state (written by save_state)");
    Savestate state(buf.data() + head, (u32)(buf.size() - head), false);
    if (state.Error || !c.nds->DoSavestate(&state) || state.Error)
        throw std::runtime_error("cannot load state " + path + " (made with another ROM?)");
    memcpy(&c.frame, buf.data() + sizeof(kStateMagic), sizeof(u64));
    const u8* screens_at = buf.data() + sizeof(kStateMagic) + sizeof(u64);
    for (int s = 0; s < 2; s++)
        memcpy(c.nds->GPU.Framebuffer[c.nds->GPU.FrontBuffer][s].get(),
               screens_at + s * kScreenWords * sizeof(u32), kScreenWords * sizeof(u32));
}

// ---- Steps --------------------------------------------------------------------------------------

void log_step(Ctx& c, size_t i, const std::string& op, const std::string& extra, bool ok)
{
    fprintf(c.log, "{\"step\":%zu,\"op\":\"%s\",\"frame\":%llu,\"top\":\"%s\",\"bottom\":\"%s\",\"ok\":%s%s}\n",
            i, op.c_str(), (unsigned long long)c.frame, hash_screen(c, 0).c_str(),
            hash_screen(c, 1).c_str(), ok ? "true" : "false", extra.c_str());
    fflush(c.log);
    if (!ok)
    {
        c.failed = true;
        char line[512];
        snprintf(line, sizeof(line), "step %zu (%s) failed at frame %llu%s", i, op.c_str(),
                 (unsigned long long)c.frame, extra.c_str());
        c.failures.push_back(line);
        fprintf(stderr, "%s\n", line);
    }
}

void run_step(Ctx& c, size_t i, const json::Value& st)
{
    int frames = st.has("frames") ? (int)st["frames"].i() : 1;
    long long timeout = st.has("timeout") ? st["timeout"].i() : 600;
    if (st.has("press") || st.has("hold"))
    {
        bool press = st.has("press");
        u32 keys = key_mask(press ? st["press"] : st["hold"]);
        for (int f = 0; f < frames; f++)
            run_frame(c, keys, std::nullopt);
        if (press)
            run_frame(c, 0, std::nullopt);
        log_step(c, i, press ? "press" : "hold", "", true);
    }
    else if (st.has("touch"))
    {
        auto& p = st["touch"];
        std::pair<int, int> xy{(int)p[0].i(), (int)p[1].i()};
        for (int f = 0; f < frames; f++)
            run_frame(c, 0, xy);
        run_frame(c, 0, std::nullopt);
        log_step(c, i, "touch", "", true);
    }
    else if (st.has("drag"))
    {
        auto& pts = st["drag"].a;
        int segs = (int)pts.size() - 1;
        for (int f = 0; f <= frames; f++)
        {
            double t = segs * (double)f / frames;
            int s = std::min((int)t, segs - 1);
            double u = t - s;
            int x = (int)(pts[s][0].n + (pts[s + 1][0].n - pts[s][0].n) * u);
            int y = (int)(pts[s][1].n + (pts[s + 1][1].n - pts[s][1].n) * u);
            run_frame(c, 0, std::make_pair(x, y));
        }
        run_frame(c, 0, std::nullopt);
        log_step(c, i, "drag", "", true);
    }
    else if (st.has("wait"))
    {
        for (long long f = 0; f < st["wait"].i(); f++)
            run_frame(c, 0, std::nullopt);
        log_step(c, i, "wait", "", true);
    }
    else if (st.has("wait_until"))
    {
        auto& w = st["wait_until"];
        bool ok = false;
        for (long long f = 0; f <= timeout; f++)
        {
            ok = w.has("ram") ? ram_matches(c, w["ram"]) : screen_matches(c, w);
            if (ok)
                break;
            run_frame(c, 0, std::nullopt);
        }
        std::string extra;
        if (!ok && w.has("ram"))
        {
            // What it read instead, to tell a wrong value from a missing one.
            auto& r = w["ram"];
            extra = ",\"read\":" + (r.has("text") ? "\"" + esc(read_text(c, r)) + "\""
                                                   : std::to_string(read_ram(c, r)));
        }
        log_step(c, i, "wait_until", extra, ok);
    }
    else if (st.has("assert_ram"))
    {
        auto& r = st["assert_ram"];
        bool ok = ram_matches(c, r);
        std::string read = r.has("text") ? "\"" + esc(read_text(c, r)) + "\""
                                         : std::to_string(read_ram(c, r));
        log_step(c, i, "assert_ram", ",\"read\":" + read, ok);
    }
    else if (st.has("assert_screen") || st.has("wait_screen"))
    {
        bool wait = st.has("wait_screen");
        const std::string& tag = wait ? st["wait_screen"].s : st["assert_screen"].s;
        image::Image now = screens(c);
        Match m = match_tag(c, now, tag);
        for (long long f = 0; wait && !m.ok && f < timeout; f++)
        {
            run_frame(c, 0, std::nullopt);
            now = screens(c);
            m = match_tag(c, now, tag);
        }
        std::string extra = ",\"tag\":\"" + esc(tag) + "\"";
        bool ok = m.ok;
        if (ok)
            extra += ",\"needle\":\"" + esc(m.best->name) + "\"";
        else
            ok = settle_needle(c, i, tag, now, m, extra);
        log_step(c, i, wait ? "wait_screen" : "assert_screen", extra, ok);
    }
    else if (st.has("wait_image") || st.has("tap_image"))
    {
        bool tap = st.has("tap_image");
        std::string path = join(c.base, tap ? st["tap_image"].s : st["wait_image"].s);
        image::Image tmpl = image::read_png(path);
        // Only the bottom screen takes touches.
        image::Rect area = tap ? image::Rect{0, 192, 256, 192} : search_area(st, "both");
        auto at = wait_for_image(c, tmpl, area, timeout);
        std::string extra = ",\"image\":\"" + esc(path) + "\"";
        if (at)
        {
            extra += ",\"x\":" + std::to_string(at->first) + ",\"y\":" + std::to_string(at->second);
            if (tap)
            {
                std::pair<int, int> xy{at->first + tmpl.w / 2, at->second - 192 + tmpl.h / 2};
                for (int f = 0; f < frames; f++)
                    run_frame(c, 0, xy);
                run_frame(c, 0, std::nullopt);
            }
        }
        else
        {
            std::string actual = c.out + "/step" + std::to_string(i) + "-actual.png";
            image::write_png(screens(c), actual);
            extra += ",\"actual\":\"" + esc(actual) + "\"";
        }
        log_step(c, i, tap ? "tap_image" : "wait_image", extra, at.has_value());
    }
    else if (st.has("screenshot"))
    {
        std::string path = join(c.out, st["screenshot"].s);
        image::write_png(screens(c), path);
        log_step(c, i, "screenshot", ",\"path\":\"" + esc(path) + "\"", true);
    }
    else if (st.has("every"))
    {
        // {"every": 60, "frames": 1200, "prefix": "boot"}: screenshot every N frames (exploration).
        long long n = st["every"].i(), total = st["frames"].i();
        std::string prefix = st.has("prefix") ? st["prefix"].s : "f";
        for (long long f = 1; f <= total; f++)
        {
            run_frame(c, 0, std::nullopt);
            if (f % n == 0)
            {
                char name[96];
                snprintf(name, sizeof(name), "%s-%06llu.png", prefix.c_str(), (unsigned long long)c.frame);
                image::write_png(screens(c), join(c.out, name));
            }
        }
        log_step(c, i, "every", "", true);
    }
    else if (st.has("sync"))
    {
        if (!c.shared)
            throw std::runtime_error("sync needs two instances");
        const std::string& name = st["sync"].s;
        {
            std::lock_guard<std::mutex> g(c.shared->lock);
            c.shared->arrivals[name]++;
        }
        long long sync_timeout = st.has("timeout") ? st["timeout"].i() : 60000;
        bool ok = false;
        for (long long f = 0; f <= sync_timeout && !ok; f++)
        {
            {
                std::lock_guard<std::mutex> g(c.shared->lock);
                ok = c.shared->arrivals[name] >= 2;
            }
            if (!ok)
            {
                if (!c.shared->active[1 - c.inst] || c.shared->done[1 - c.inst])
                    break;
                run_frame(c, 0, std::nullopt);
            }
        }
        log_step(c, i, "sync", ",\"label\":\"" + esc(name) + "\"", ok);
    }
    else if (st.has("hash"))
    {
        log_step(c, i, "hash", ",\"label\":\"" + esc(st["hash"].s) + "\"", true);
    }
    else if (st.has("save_state"))
    {
        std::string path = join(c.out, st["save_state"].s);
        bool ok = save_state(c, path);
        log_step(c, i, "save_state", ",\"path\":\"" + esc(path) + "\"", ok);
    }
    else if (st.has("dump_save"))
    {
        std::string path = join(c.out, st["dump_save"].s);
        const u8* mem = c.nds->GetNDSSave();
        u32 len = c.nds->GetNDSSaveLength();
        bool ok = mem && len;
        if (ok)
            write_file(path, mem, len);
        log_step(c, i, "dump_save", ",\"path\":\"" + esc(path) + "\",\"len\":" + std::to_string(len), ok);
    }
    else
        throw std::runtime_error("unknown step " + std::to_string(i));
}

void setup(Ctx& c, const json::Value& cfg)
{
    fs::create_directories(c.out);

    NDSArgs args;
    args.JIT = std::nullopt;
    if (cfg.has("bios_dir"))
    {
        std::string d = join(c.base, cfg["bios_dir"].s);
        auto b9 = read_file(d + "/bios9.bin"), b7 = read_file(d + "/bios7.bin");
        auto fw = read_file(d + "/firmware.bin");
        if (b9.size() != ARM9BIOSSize || b7.size() != ARM7BIOSSize)
            throw std::runtime_error("BIOS sizes are wrong");
        args.ARM9BIOS = std::make_unique<ARM9BIOSImage>();
        args.ARM7BIOS = std::make_unique<ARM7BIOSImage>();
        memcpy(args.ARM9BIOS->data(), b9.data(), b9.size());
        memcpy(args.ARM7BIOS->data(), b7.data(), b7.size());
        args.Firmware = Firmware(fw.data(), (u32)fw.size());
    }
    if (c.inst > 0)
    {
        // Consoles on one network need different MAC addresses (as EmuInstance does).
        auto& mac = args.Firmware.GetHeader().MacAddr;
        mac[3] += c.inst;
        mac[4] += c.inst * 0x44;
        mac[5] += c.inst * 0x10;
        args.Firmware.UpdateChecksums();
    }
    c.nds = std::make_unique<NDS>(std::move(args), &c.inst);

    std::string rom_path = join(c.base, cfg["rom"].s);
    auto rom = read_file(rom_path);
    NDSCart::NDSCartArgs cart_args{};
    if (cfg.has("sd_fresh") && cfg["sd_fresh"].b)
    {
        // A private copy of the card per run: scripts can't see each other's writes (settings,
        // saves) and the source folder never changes.
        if (!cfg.has("sd_dir"))
            throw std::runtime_error("sd_fresh needs sd_dir");
        std::string src = join(c.base, cfg["sd_dir"].s), dst = c.out + "/sd";
        fs::remove_all(dst);
        fs::remove(c.out + "/sd.img");
        fs::remove(c.out + "/sd.img.idx");
        fs::copy(src, dst, fs::copy_options::recursive);
        cart_args.SDCard = FATStorageArgs{c.out + "/sd.img", 0, false, dst};
    }
    else if (cfg.has("sd_image"))
        cart_args.SDCard = FATStorageArgs{
            join(c.base, cfg["sd_image"].s), 0, false,
            cfg.has("sd_dir") ? std::optional<std::string>(join(c.base, cfg["sd_dir"].s))
                              : std::nullopt};
    if (cfg.has("save"))
    {
        auto sav = read_file(join(c.base, cfg["save"].s));
        cart_args.SRAM = std::make_unique<u8[]>(sav.size());
        memcpy(cart_args.SRAM.get(), sav.data(), sav.size());
        cart_args.SRAMLength = (u32)sav.size();
    }
    auto romdata = std::make_unique<u8[]>(rom.size());
    memcpy(romdata.get(), rom.data(), rom.size());
    auto cart = NDSCart::ParseROM(std::move(romdata), (u32)rom.size(), &c.inst, std::move(cart_args));
    if (!cart)
        throw std::runtime_error("cannot parse ROM " + rom_path);

    c.nds->Reset();
    c.nds->SetNDSCart(std::move(cart));
    bool direct = !cfg.has("direct_boot") || cfg["direct_boot"].b;
    if (direct || c.nds->NeedsDirectBoot())
    {
        std::string name = rom_path.substr(rom_path.rfind('/') + 1);
        c.nds->SetupDirectBoot(name);
    }
    int rtc[6] = {2026, 1, 1, 12, 0, 0};
    if (cfg.has("rtc"))
        for (int i = 0; i < 6; i++)
            rtc[i] = (int)cfg["rtc"][i].i();
    c.nds->RTC.SetDateTime(rtc[0], rtc[1], rtc[2], rtc[3], rtc[4], rtc[5]);
    c.nds->Start();

    if (cfg.has("state"))
        load_state(c, join(c.base, cfg["state"].s));
    if (cfg.has("elf"))
        c.symbols = elf::load_symbols(join(c.base, cfg["elf"].s));
    if (!g_opts.needles.empty() || cfg.has("needles"))
    {
        c.needles_dir = !g_opts.needles.empty() ? g_opts.needles : join(c.base, cfg["needles"].s);
        load_needles(c);
    }

    c.log = fopen((c.out + "/run.jsonl").c_str(), "w");
    if (!c.log)
        throw std::runtime_error("cannot write run log");
}

void run_steps(Ctx& c, const json::Value& cfg)
{
    auto& steps = cfg["steps"].a;
    for (size_t i = 0; i < steps.size(); i++)
        run_step(c, i, steps[i]);
}

// The outcome of one script, for the summary and the JUnit report.
struct Result
{
    std::string name;
    double seconds = 0;
    std::vector<std::string> failures;
    std::string error;  // the script couldn't run
};

void run_linked(const json::Value& cfg, const std::string& base, const std::string& root, Result& res)
{
    auto& insts = cfg["instances"].a;
    if (insts.size() != 2)
        throw std::runtime_error("instances must list exactly two consoles");

    // melonDS's default 25 ms suits one console per real-time frame; unthrottled, loaded threads
    // miss it and Memory Link drops mid-download.
    LocalMP mp;
    mp.SetRecvTimeout(150);
    Platform::set_local_mp(&mp);
    Shared shared;
    Ctx ctx[2];
    for (int i = 0; i < 2; i++)
    {
        ctx[i].inst = i;
        ctx[i].shared = &shared;
        ctx[i].base = base;
        ctx[i].out = root + "/inst" + std::to_string(i);
        setup(ctx[i], insts[i]);
        shared.active[i] = true;
    }

    std::string errors[2];
    std::thread threads[2];
    for (int i = 0; i < 2; i++)
        threads[i] = std::thread([&, i] {
            Ctx& c = ctx[i];
            try
            {
                run_steps(c, insts[i]);
                shared.done[i] = true;
                while (!shared.done[1 - i] && shared.active[1 - i])
                    run_frame(c, 0, std::nullopt);
            }
            catch (const std::exception& e)
            {
                errors[i] = e.what();
            }
            shared.active[i] = false;
        });
    for (auto& t : threads)
        t.join();
    Platform::set_local_mp(nullptr);

    for (int i = 0; i < 2; i++)
    {
        if (!errors[i].empty())
            res.error += "instance " + std::to_string(i) + ": " + errors[i] + "\n";
        for (auto& f : ctx[i].failures)
            res.failures.push_back("instance " + std::to_string(i) + ": " + f);
        fclose(ctx[i].log);
        printf("instance %d: %zu steps, %llu frames, %s\n", i, insts[i]["steps"].a.size(),
               (unsigned long long)ctx[i].frame, ctx[i].failed ? "FAILED" : "ok");
    }
}

Result run_script(const std::string& script, const std::string& root)
{
    Result res;
    res.name = fs::path(script).stem().string();
    auto start = std::chrono::steady_clock::now();
    try
    {
        json::Value cfg = read_json(script);
        size_t slash = script.rfind('/');
        std::string base = slash == std::string::npos ? "." : script.substr(0, slash);
        if (!cfg.has("instances"))
        {
            Ctx c;
            c.base = base;
            c.out = root;
            setup(c, cfg);
            run_steps(c, cfg);
            fclose(c.log);
            res.failures = c.failures;
            printf("%s: %zu steps, %llu frames, %s\n", res.name.c_str(), cfg["steps"].a.size(),
                   (unsigned long long)c.frame, c.failed ? "FAILED" : "ok");
        }
        else
            run_linked(cfg, base, root, res);
    }
    catch (const std::exception& e)
    {
        res.error = e.what();
        fprintf(stderr, "emu-harness: %s: %s\n", res.name.c_str(), e.what());
    }
    res.seconds = std::chrono::duration<double>(std::chrono::steady_clock::now() - start).count();
    return res;
}

std::string xml(const std::string& s)
{
    std::string o;
    for (char ch : s)
        switch (ch)
        {
        case '<': o += "&lt;"; break;
        case '>': o += "&gt;"; break;
        case '&': o += "&amp;"; break;
        case '"': o += "&quot;"; break;
        default: o += ch;
        }
    return o;
}

void write_junit(const std::string& path, const std::vector<Result>& results)
{
    int failures = 0, errors = 0;
    double total = 0;
    for (auto& r : results)
    {
        failures += !r.failures.empty() && r.error.empty();
        errors += !r.error.empty();
        total += r.seconds;
    }
    std::ofstream f(path);
    if (!f)
        throw std::runtime_error("cannot write " + path);
    f << "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n";
    f << "<testsuite name=\"emu-harness\" tests=\"" << results.size() << "\" failures=\"" << failures
      << "\" errors=\"" << errors << "\" time=\"" << total << "\">\n";
    for (auto& r : results)
    {
        f << "  <testcase classname=\"emu-harness\" name=\"" << xml(r.name) << "\" time=\"" << r.seconds
          << "\"";
        if (r.error.empty() && r.failures.empty())
        {
            f << "/>\n";
            continue;
        }
        f << ">\n";
        if (!r.error.empty())
            f << "    <error message=\"" << xml(r.error) << "\"/>\n";
        else
        {
            std::string all;
            for (auto& line : r.failures)
                all += line + "\n";
            f << "    <failure message=\"" << xml(r.failures.front()) << "\">" << xml(all)
              << "</failure>\n";
        }
        f << "  </testcase>\n";
    }
    f << "</testsuite>\n";
}

}  // namespace

int main(int argc, char** argv)
{
    std::vector<std::string> scripts;
    std::string out, junit;
    for (int i = 1; i < argc; i++)
    {
        if (!strcmp(argv[i], "--out") && i + 1 < argc)
            out = argv[++i];
        else if (!strcmp(argv[i], "--junit") && i + 1 < argc)
            junit = argv[++i];
        else if (!strcmp(argv[i], "--needles") && i + 1 < argc)
            g_opts.needles = argv[++i];
        else if (!strcmp(argv[i], "--update"))
            g_opts.update = true;
        else if (!strcmp(argv[i], "--verbose"))
            Platform::g_verbose = true;
        else if (argv[i][0] == '-')
        {
            fprintf(stderr, "emu-harness: unknown option %s\n", argv[i]);
            return 2;
        }
        else
            scripts.push_back(argv[i]);
    }
    if (scripts.empty())
    {
        fprintf(stderr,
                "usage: emu-harness script.json [more.json ...] [--out DIR] [--update] [--junit FILE] "
                "[--needles DIR] [--verbose]\n");
        return 2;
    }

    std::vector<Result> results;
    for (auto& s : scripts)
    {
        std::string root = out.empty() ? s + ".out"
                           : scripts.size() == 1 ? out
                                                 : out + "/" + fs::path(s).stem().string();
        results.push_back(run_script(s, root));
    }
    if (!junit.empty())
        write_junit(junit, results);

    int failed = 0, errored = 0;
    for (auto& r : results)
    {
        errored += !r.error.empty();
        failed += r.error.empty() && !r.failures.empty();
    }
    if (results.size() > 1)
        printf("%zu scripts: %zu passed, %d failed, %d errors\n", results.size(),
               results.size() - failed - errored, failed, errored);
    return errored ? 2 : failed ? 1 : 0;
}
