// SPDX-License-Identifier: GPL-3.0-only
#include "elf.h"

#include <cstring>
#include <fstream>
#include <iterator>
#include <stdexcept>
#include <vector>

namespace elf {

namespace {

uint32_t u32(const std::vector<uint8_t>& d, size_t at)
{
    if (at + 4 > d.size())
        throw std::runtime_error("ELF truncated");
    return (uint32_t)d[at] | (uint32_t)d[at + 1] << 8 | (uint32_t)d[at + 2] << 16
           | (uint32_t)d[at + 3] << 24;
}

uint16_t u16(const std::vector<uint8_t>& d, size_t at)
{
    if (at + 2 > d.size())
        throw std::runtime_error("ELF truncated");
    return (uint16_t)(d[at] | d[at + 1] << 8);
}

}  // namespace

std::map<std::string, Symbol> load_symbols(const std::string& path)
{
    std::ifstream f(path, std::ios::binary);
    if (!f)
        throw std::runtime_error("cannot read " + path);
    std::vector<uint8_t> d(std::istreambuf_iterator<char>(f), {});
    // e_ident: magic, ELFCLASS32 (1), ELFDATA2LSB (1).
    if (d.size() < 52 || memcmp(d.data(), "\x7f" "ELF", 4) != 0 || d[4] != 1 || d[5] != 1)
        throw std::runtime_error(path + " is not a 32-bit little-endian ELF");

    uint32_t shoff = u32(d, 0x20);
    uint16_t shentsize = u16(d, 0x2E), shnum = u16(d, 0x30);
    std::map<std::string, Symbol> out;
    bool found = false;
    for (uint16_t i = 0; i < shnum; i++)
    {
        size_t sh = shoff + (size_t)i * shentsize;
        if (u32(d, sh + 4) != 2)  // SHT_SYMTAB
            continue;
        found = true;
        uint32_t off = u32(d, sh + 0x10), size = u32(d, sh + 0x14);
        uint32_t link = u32(d, sh + 0x18), entsize = u32(d, sh + 0x24);
        size_t strsh = shoff + (size_t)link * shentsize;
        uint32_t stroff = u32(d, strsh + 0x10), strsize = u32(d, strsh + 0x14);
        if (entsize < 16 || (size_t)off + size > d.size() || (size_t)stroff + strsize > d.size())
            throw std::runtime_error("bad symbol table in " + path);
        for (uint32_t e = 0; e + entsize <= size; e += entsize)
        {
            size_t s = off + e;
            uint32_t name = u32(d, s), value = u32(d, s + 4), sz = u32(d, s + 8);
            uint8_t type = d[s + 12] & 0xF;
            if (name == 0 || name >= strsize || (type != 1 && type != 2))  // OBJECT, FUNC
                continue;
            const char* n = reinterpret_cast<const char*>(&d[stroff + name]);
            out.emplace(std::string(n, strnlen(n, strsize - name)), Symbol{value, sz});
        }
    }
    if (!found)
        throw std::runtime_error(path + " has no symbol table (stripped?)");
    return out;
}

}  // namespace elf
