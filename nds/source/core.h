// SPDX-License-Identifier: GPL-3.0-only
// The open save and the data blobs, over the Rust core's FFI. The UI never touches save bytes.
#pragma once

#include <stddef.h>
#include <stdint.h>

#include "verity.h"

namespace core {

struct Doc
{
    char path[192] = {0};
    uint8_t* buf = nullptr;
    size_t len = 0;
    verity_save_info info{};
    uint32_t boxes = 0;

    bool loaded() const { return buf != nullptr; }
};

Doc& doc();

// Reads and probes a save file. On failure the previous document is kept and `err` says why.
bool open(const char* path, const char** err);

// Loads (once) the blobs the save's game needs from NitroFS and attaches them.
// Returns false if NitroFS or a blob is missing (the UI then shows IDs instead of names).
bool load_data_for(uint8_t kind);
bool data_available();

verity_pkm_view slot(uint8_t box_index, uint8_t slot);
// Legality of a slot; `ms` gets the time taken. Loads the legality data on first use (Gen 4).
verity_legality_report legality(uint8_t box_index, uint8_t slot, uint32_t* ms);
const char* check_name(uint8_t id);
void box_name(uint32_t box_index, char* out, int cap);

// The last opened file, kept in /_nds/verity/last.txt on the SD card.
void remember(const char* path);
bool last(char* out, int cap);

const char* game_name(uint8_t kind);

}  // namespace core
