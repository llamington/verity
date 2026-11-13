// SPDX-License-Identifier: GPL-3.0-only
//
// melonDS Platform hooks for the headless harness: stdio files, std threads, no audio/video
// devices. Local multiplayer (MP_*) and networking are stubs until the two-instance work.

#include <chrono>
#include <condition_variable>
#include <cstdarg>
#include <cstdio>
#include <cstring>
#include <mutex>
#include <string>
#include <thread>

#include <sys/stat.h>
#include <unistd.h>

#include "Platform.h"
#include "SPI_Firmware.h"
#include "LocalMP.h"

namespace melonDS::Platform {

bool g_verbose = false;

void SignalStop(StopReason, void*) {}

static std::string mode_string(FileMode mode, bool exists)
{
    std::string m;
    if (mode & FileMode::Append)
        m = (mode & FileMode::Read) ? "a+" : "a";
    else if (mode & FileMode::Write)
    {
        if ((mode & FileMode::Preserve) && exists)
            m = "r+";
        else
            m = (mode & FileMode::Read) ? "w+" : "w";
    }
    else
        m = "r";
    if (!(mode & FileMode::Text))
        m += 'b';
    return m;
}

FileHandle* OpenFile(const std::string& path, FileMode mode)
{
    if ((mode & (FileMode::ReadWrite | FileMode::Append)) == FileMode::None)
        return nullptr;
    bool exists = access(path.c_str(), F_OK) == 0;
    if ((mode & FileMode::NoCreate) && !exists)
        return nullptr;
    return reinterpret_cast<FileHandle*>(fopen(path.c_str(), mode_string(mode, exists).c_str()));
}

std::string GetLocalFilePath(const std::string& filename) { return filename; }
FileHandle* OpenLocalFile(const std::string& path, FileMode mode) { return OpenFile(path, mode); }
bool FileExists(const std::string& name) { return access(name.c_str(), F_OK) == 0; }
bool LocalFileExists(const std::string& name) { return FileExists(name); }
bool CheckFileWritable(const std::string& filepath)
{
    return access(filepath.c_str(), W_OK) == 0 || !FileExists(filepath);
}
bool CheckLocalFileWritable(const std::string& filepath) { return CheckFileWritable(filepath); }

static FILE* f(FileHandle* h) { return reinterpret_cast<FILE*>(h); }
bool CloseFile(FileHandle* file) { return fclose(f(file)) == 0; }
bool IsEndOfFile(FileHandle* file) { return feof(f(file)) != 0; }
bool FileReadLine(char* str, int count, FileHandle* file) { return fgets(str, count, f(file)) != nullptr; }
u64 FilePosition(FileHandle* file) { return (u64)ftello(f(file)); }
bool FileSeek(FileHandle* file, s64 offset, FileSeekOrigin origin)
{
    int w = origin == FileSeekOrigin::Start ? SEEK_SET : origin == FileSeekOrigin::Current ? SEEK_CUR : SEEK_END;
    return fseeko(f(file), offset, w) == 0;
}
void FileRewind(FileHandle* file) { rewind(f(file)); }
u64 FileRead(void* data, u64 size, u64 count, FileHandle* file) { return fread(data, size, count, f(file)); }
bool FileFlush(FileHandle* file) { return fflush(f(file)) == 0; }
u64 FileWrite(const void* data, u64 size, u64 count, FileHandle* file) { return fwrite(data, size, count, f(file)); }
u64 FileWriteFormatted(FileHandle* file, const char* fmt, ...)
{
    va_list ap;
    va_start(ap, fmt);
    int n = vfprintf(f(file), fmt, ap);
    va_end(ap);
    return n < 0 ? 0 : (u64)n;
}
u64 FileLength(FileHandle* file)
{
    struct stat st;
    return fstat(fileno(f(file)), &st) == 0 ? (u64)st.st_size : 0;
}

void Log(LogLevel level, const char* fmt, ...)
{
    if (!g_verbose && level < LogLevel::Warn)
        return;
    // Homebrew isn't a retail cartridge: melonDS warns that it isn't in its ROM list, has an odd
    // size and no encrypted secure area. Expected, so shown only with --verbose.
    static const char* const kHomebrew[] = {"ROM entry not found", "bad ROM size",
                                            "Secure area decryption failed"};
    if (!g_verbose)
        for (const char* h : kHomebrew)
            if (strstr(fmt, h))
                return;
    va_list ap;
    va_start(ap, fmt);
    vfprintf(stderr, fmt, ap);
    va_end(ap);
}

struct Thread { std::thread t; };
Thread* Thread_Create(std::function<void()> func) { return new Thread{std::thread(std::move(func))}; }
void Thread_Free(Thread* thread)
{
    if (thread->t.joinable())
        thread->t.detach();
    delete thread;
}
void Thread_Wait(Thread* thread)
{
    if (thread->t.joinable())
        thread->t.join();
}

struct Semaphore
{
    std::mutex m;
    std::condition_variable cv;
    int count = 0;
};
Semaphore* Semaphore_Create() { return new Semaphore; }
void Semaphore_Free(Semaphore* s) { delete s; }
void Semaphore_Reset(Semaphore* s)
{
    std::lock_guard<std::mutex> g(s->m);
    s->count = 0;
}
void Semaphore_Wait(Semaphore* s)
{
    std::unique_lock<std::mutex> g(s->m);
    s->cv.wait(g, [s] { return s->count > 0; });
    s->count--;
}
bool Semaphore_TryWait(Semaphore* s, int timeout_ms)
{
    std::unique_lock<std::mutex> g(s->m);
    if (!s->cv.wait_for(g, std::chrono::milliseconds(timeout_ms), [s] { return s->count > 0; }))
        return false;
    s->count--;
    return true;
}
void Semaphore_Post(Semaphore* s, int count)
{
    {
        std::lock_guard<std::mutex> g(s->m);
        s->count += count;
    }
    s->cv.notify_all();
}

struct Mutex { std::mutex m; };
Mutex* Mutex_Create() { return new Mutex; }
void Mutex_Free(Mutex* m) { delete m; }
void Mutex_Lock(Mutex* m) { m->m.lock(); }
void Mutex_Unlock(Mutex* m) { m->m.unlock(); }
bool Mutex_TryLock(Mutex* m) { return m->m.try_lock(); }

void Sleep(u64 usecs) { std::this_thread::sleep_for(std::chrono::microseconds(usecs)); }
u64 GetMSCount()
{
    using namespace std::chrono;
    return duration_cast<milliseconds>(steady_clock::now().time_since_epoch()).count();
}
u64 GetUSCount()
{
    using namespace std::chrono;
    return duration_cast<microseconds>(steady_clock::now().time_since_epoch()).count();
}

// Saves are read back explicitly with dump_save; the game's incremental writes are not mirrored.
void WriteNDSSave(const u8*, u32, u32, u32, void*) {}
void WriteGBASave(const u8*, u32, u32, u32, void*) {}
void WriteFirmware(const Firmware&, u32, u32, void*) {}
void WriteDateTime(int, int, int, int, int, int, void*) {}

// Linked runs set the shared LocalMP; single-instance runs have no network. The core's userdata
// is a pointer to the instance number (Ctx::inst).
static LocalMP* g_mp = nullptr;
void set_local_mp(LocalMP* mp) { g_mp = mp; }
static int inst(void* userdata) { return *static_cast<int*>(userdata); }

void MP_Begin(void* u) { if (g_mp) g_mp->Begin(inst(u)); }
void MP_End(void* u) { if (g_mp) g_mp->End(inst(u)); }
int MP_SendPacket(u8* d, int len, u64 ts, void* u) { return g_mp ? g_mp->SendPacket(inst(u), d, len, ts) : 0; }
int MP_RecvPacket(u8* d, u64* ts, void* u) { return g_mp ? g_mp->RecvPacket(inst(u), d, ts) : 0; }
int MP_SendCmd(u8* d, int len, u64 ts, void* u) { return g_mp ? g_mp->SendCmd(inst(u), d, len, ts) : 0; }
int MP_SendReply(u8* d, int len, u64 ts, u16 aid, void* u) { return g_mp ? g_mp->SendReply(inst(u), d, len, ts, aid) : 0; }
int MP_SendAck(u8* d, int len, u64 ts, void* u) { return g_mp ? g_mp->SendAck(inst(u), d, len, ts) : 0; }
int MP_RecvHostPacket(u8* d, u64* ts, void* u) { return g_mp ? g_mp->RecvHostPacket(inst(u), d, ts) : 0; }
u16 MP_RecvReplies(u8* d, u64 ts, u16 mask, void* u) { return g_mp ? g_mp->RecvReplies(inst(u), d, ts, mask) : 0; }

int Net_SendPacket(u8*, int, void*) { return 0; }
int Net_RecvPacket(u8*, void*) { return 0; }

void Camera_Start(int, void*) {}
void Camera_Stop(int, void*) {}
void Camera_CaptureFrame(int, u32*, int, int, bool, void*) {}

void Mic_Start(void*) {}
void Mic_Stop(void*) {}
int Mic_ReadInput(s16*, int, void*) { return 0; }

AACDecoder* AAC_Init() { return nullptr; }
void AAC_DeInit(AACDecoder*) {}
bool AAC_Configure(AACDecoder*, int, int) { return false; }
bool AAC_DecodeFrame(AACDecoder*, const void*, int, void*, int) { return false; }

bool Addon_KeyDown(KeyType, void*) { return false; }
void Addon_RumbleStart(u32, void*) {}
void Addon_RumbleStop(void*) {}
float Addon_MotionQuery(MotionQueryType, void*) { return 0; }

DynamicLibrary* DynamicLibrary_Load(const char*) { return nullptr; }
void DynamicLibrary_Unload(DynamicLibrary*) {}
void* DynamicLibrary_LoadFunction(DynamicLibrary*, const char*) { return nullptr; }

}  // namespace melonDS::Platform
