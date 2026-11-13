// SPDX-License-Identifier: GPL-3.0-only
// Symbols from a 32-bit little-endian ELF (the homebrew's .elf), so scripts can name variables.
#pragma once

#include <cstdint>
#include <map>
#include <string>

namespace elf {

struct Symbol
{
    uint32_t addr;
    uint32_t size;
};

// Every named object and function symbol in .symtab. Throws if the file isn't a 32-bit ELF or has
// no symbol table (a stripped build).
std::map<std::string, Symbol> load_symbols(const std::string& path);

}  // namespace elf
