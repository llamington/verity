// SPDX-License-Identifier: GPL-3.0-only
//
// verity emu-harness: runs melonDS headless from a JSON script (PLAN §7).
//
//   emu-harness script.json [--out DIR] [--verbose]
//
// Script:
//   {
//     "rom": "nds/verity.nds",            // relative paths resolve against the script's dir
//     "save": "path.sav",                 // optional cartridge save to load
//     "bios_dir": "...",                  // optional bios7.bin, bios9.bin, firmware.bin
//                                         // (default: melonDS FreeBIOS + generated firmware)
//     "sd_dir": "sd", "sd_image": "sd.img",  // optional DLDI SD card synced from a folder
//     "rtc": [2026, 1, 1, 12, 0, 0],      // fixed date/time (default shown)
//     "direct_boot": true,                // skip the DS menu (default; forced with FreeBIOS)
//     "steps": [
//       {"press": ["A"], "frames": 2},          // hold keys, then release for one frame
//       {"hold": ["Down"], "frames": 30},       // hold without the release frame
//       {"touch": [x, y], "frames": 2},         // bottom-screen touch
//       {"drag": [[x, y], [x, y]], "frames": 20},
//       {"wait": 60},
//       {"wait_until": {"screen_hash": "…", "screen": "top"}, "timeout": 600},
//       {"wait_until": {"ram": {"addr": 0x02000000, "value": 1, "mask": 0xFF}}, "timeout": 600},
//       {"assert_ram": {"addr": 0x02000000, "value": 1, "mask": 0xFF}},
//       {"screenshot": "name.png"},             // both screens stacked, 256x384
//       {"hash": "label"},                      // log the screen hashes
//       {"dump_save": "out.sav"}
//     ]
//   }
//
// Two consoles on local wireless: {"instances": [{...}, {...}]}, each object in the format above
// (with bios_dir; "direct_boot": false boots the DS menu, needed to receive Download Play). Each
// instance runs its own steps on its own thread, and keeps running with no input once they end,
// until the other one is done. {"sync": "name", "timeout": 60000} waits for the other instance to
// reach the same name. Output goes to --out/inst0 and inst1. Linked runs are not deterministic:
// melonDS's LocalMP waits on real-time semaphores.
//
// Outputs in --out (default: <script>.out/): screenshots, save dumps, and run.jsonl (one JSON
// object per step with the frame number and screen hashes). Exit status 0 when every step passes.

#include <atomic>
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

#include <png.h>
#include <sys/stat.h>

#include "Args.h"
#include "LocalMP.h"
#include "NDS.h"
#include "NDSCart.h"
#include "Platform.h"
#include "json.h"

using namespace melonDS;

namespace melonDS::Platform {
extern bool g_verbose;
void set_local_mp(LocalMP* mp);
}

namespace {

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
// wireless commands, and each unanswered command costs the sender LocalMP's 25 ms timeout.
constexpr u64 kMaxFrameLead = 8;
constexpr int kMaxLeadWaitUs = 2000;

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

void write_png(Ctx& c, const std::string& path)
{
    FILE* fp = fopen(path.c_str(), "wb");
    if (!fp)
        throw std::runtime_error("cannot write " + path);
    png_structp png = png_create_write_struct(PNG_LIBPNG_VER_STRING, nullptr, nullptr, nullptr);
    png_infop info = png_create_info_struct(png);
    if (setjmp(png_jmpbuf(png)))
    {
        fclose(fp);
        throw std::runtime_error("png error");
    }
    png_init_io(png, fp);
    png_set_IHDR(png, info, 256, 384, 8, PNG_COLOR_TYPE_RGB, PNG_INTERLACE_NONE,
                 PNG_COMPRESSION_TYPE_DEFAULT, PNG_FILTER_TYPE_DEFAULT);
    png_write_info(png, info);
    std::vector<u8> row(256 * 3);
    for (int s = 0; s < 2; s++)
    {
        const u32* px = screen(c, s);
        for (int y = 0; y < 192; y++)
        {
            for (int x = 0; x < 256; x++)
            {
                u32 v = px[y * 256 + x];  // 0xAARRGGBB
                row[x * 3 + 0] = (v >> 16) & 0xFF;
                row[x * 3 + 1] = (v >> 8) & 0xFF;
                row[x * 3 + 2] = v & 0xFF;
            }
            png_write_row(png, row.data());
        }
    }
    png_write_end(png, nullptr);
    png_destroy_write_struct(&png, &info);
    fclose(fp);
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

bool ram_matches(Ctx& c, const json::Value& r)
{
    u32 addr = (u32)r["addr"].i();
    u32 value = (u32)r["value"].i();
    u32 mask = r.has("mask") ? (u32)r["mask"].i() : 0xFFFFFFFFu;
    return (c.nds->ARM9Read32(addr) & mask) == (value & mask);
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

void log_step(Ctx& c, size_t i, const std::string& op, const std::string& extra, bool ok)
{
    fprintf(c.log, "{\"step\":%zu,\"op\":\"%s\",\"frame\":%llu,\"top\":\"%s\",\"bottom\":\"%s\",\"ok\":%s%s}\n",
            i, op.c_str(), (unsigned long long)c.frame, hash_screen(c, 0).c_str(),
            hash_screen(c, 1).c_str(), ok ? "true" : "false", extra.c_str());
    fflush(c.log);
    if (!ok)
    {
        c.failed = true;
        fprintf(stderr, "step %zu (%s) failed at frame %llu%s\n", i, op.c_str(),
                (unsigned long long)c.frame, extra.c_str());
    }
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

void run_step(Ctx& c, size_t i, const json::Value& st)
{
    int frames = st.has("frames") ? (int)st["frames"].i() : 1;
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
        long long timeout = st.has("timeout") ? st["timeout"].i() : 600;
        bool ok = false;
        for (long long f = 0; f <= timeout; f++)
        {
            ok = w.has("ram") ? ram_matches(c, w["ram"]) : screen_matches(c, w);
            if (ok)
                break;
            run_frame(c, 0, std::nullopt);
        }
        log_step(c, i, "wait_until", "", ok);
    }
    else if (st.has("assert_ram"))
    {
        bool ok = ram_matches(c, st["assert_ram"]);
        char extra[64];
        snprintf(extra, sizeof(extra), ",\"read\":%u", c.nds->ARM9Read32((u32)st["assert_ram"]["addr"].i()));
        log_step(c, i, "assert_ram", extra, ok);
    }
    else if (st.has("screenshot"))
    {
        std::string path = join(c.out, st["screenshot"].s);
        write_png(c, path);
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
                write_png(c, join(c.out, name));
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
        long long timeout = st.has("timeout") ? st["timeout"].i() : 60000;
        bool ok = false;
        for (long long f = 0; f <= timeout && !ok; f++)
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
    else if (st.has("dump_save"))
    {
        std::string path = join(c.out, st["dump_save"].s);
        const u8* mem = c.nds->GetNDSSave();
        u32 len = c.nds->GetNDSSaveLength();
        FILE* fp = fopen(path.c_str(), "wb");
        bool ok = fp && mem && len;
        if (ok)
            fwrite(mem, 1, len, fp);
        if (fp)
            fclose(fp);
        log_step(c, i, "dump_save", ",\"path\":\"" + esc(path) + "\",\"len\":" + std::to_string(len), ok);
    }
    else
        throw std::runtime_error("unknown step " + std::to_string(i));
}

void setup(Ctx& c, const json::Value& cfg)
{
    std::filesystem::create_directories(c.out);

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
    if (cfg.has("sd_image"))
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

int run_linked(const json::Value& cfg, const std::string& base, const std::string& root)
{
    auto& insts = cfg["instances"].a;
    if (insts.size() != 2)
        throw std::runtime_error("instances must list exactly two consoles");

    LocalMP mp;
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

    bool failed = false;
    for (int i = 0; i < 2; i++)
    {
        if (!errors[i].empty())
            fprintf(stderr, "emu-harness: instance %d: %s\n", i, errors[i].c_str());
        failed |= ctx[i].failed || !errors[i].empty();
        fclose(ctx[i].log);
        printf("instance %d: %zu steps, %llu frames, %s\n", i, insts[i]["steps"].a.size(),
               (unsigned long long)ctx[i].frame, ctx[i].failed ? "FAILED" : "ok");
    }
    return failed ? 1 : 0;
}

}  // namespace

int main(int argc, char** argv)
{
    if (argc < 2)
    {
        fprintf(stderr, "usage: emu-harness script.json [--out DIR] [--verbose]\n");
        return 2;
    }
    std::string script = argv[1], out;
    for (int i = 2; i < argc; i++)
    {
        if (!strcmp(argv[i], "--out") && i + 1 < argc)
            out = argv[++i];
        else if (!strcmp(argv[i], "--verbose"))
            Platform::g_verbose = true;
    }
    try
    {
        std::ifstream sf(script);
        if (!sf)
            throw std::runtime_error("cannot read " + script);
        std::stringstream ss;
        ss << sf.rdbuf();
        json::Value cfg = json::parse(ss.str());

        size_t slash = script.rfind('/');
        std::string base = slash == std::string::npos ? "." : script.substr(0, slash);
        std::string root = out.empty() ? script + ".out" : out;

        if (!cfg.has("instances"))
        {
            Ctx c;
            c.base = base;
            c.out = root;
            setup(c, cfg);
            run_steps(c, cfg);
            fclose(c.log);
            printf("%zu steps, %llu frames, %s\n", cfg["steps"].a.size(),
                   (unsigned long long)c.frame, c.failed ? "FAILED" : "ok");
            return c.failed ? 1 : 0;
        }
        return run_linked(cfg, base, root);
    }
    catch (const std::exception& e)
    {
        fprintf(stderr, "emu-harness: %s\n", e.what());
        return 2;
    }
}
