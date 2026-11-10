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
    bool dirty = false;  // unsaved changes

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

// Loads the ROM font and box icon caches, building them from the best Gen 4/5 ROM on the SD card
// when they are missing or a better ROM appears. Without a ROM: Latin text, placeholder icons.
void rom_assets_init();
// A Pokémon's box icon from the ROM: 512 bytes of 4 bpp tiles (1D 32x32) and its palette.
bool icon_tiles(uint16_t species, uint8_t form, bool female, bool egg, uint8_t* tiles, uint8_t* palette);
// Type and PP of a move, plus category, power and accuracy when a ROM of the save's generation
// was found (from_rom). False for a move number the generation doesn't have.
bool move_info(uint8_t generation, uint16_t mv, verity_move_stats* out);
// The game code of the ROM the font came from ("" for none).
const char* font_game();
bool last(char* out, int cap);

// Reads a whole save file; the caller frees it. nullptr if it cannot be read.
uint8_t* read_save_file(const char* path, size_t* len);

const char* game_name(uint8_t kind);

// The version whose wild slots the add-Pokémon wizard offers for this save (10/11/12).
uint8_t wizard_version();
// Makes sure the Gen 4 legality/generation data is attached.
bool ensure_gen4_data();
bool ensure_save_data();
// Attaches the Mystery Gift databases (first use only; about 500 KB).
bool ensure_events_data();

// Writes the open save back to its file: finalise, validate (never writes if the validator
// fails), rotate backups (<name>.bak1..3), write a temp file, swap it in, re-read and compare.
bool save_to_disk(const char** message);

}  // namespace core
