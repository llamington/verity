#ifndef VERITY_H
#define VERITY_H

#include <stdint.h>
#include <stddef.h>
#include <stdbool.h>

#ifdef __cplusplus
extern "C" {
#endif

/**
 * Calculates the CRC16-CCITT checksum over a buffer.
 * Ported from PKHeX (Checksums.CRC16_CCITT).
 */
uint16_t verity_crc16_ccitt(const uint8_t *data, size_t len);

typedef enum verity_err {
    VERITY_OK = 0,
    VERITY_ERR_NULL_POINTER = 1,
    VERITY_ERR_UNRECOGNIZED = 2,
    VERITY_ERR_BAD_ARGUMENT = 3,
    VERITY_ERR_BAD_DATA = 4,
} verity_err;

/** Data blobs (files in data/blobs). */
typedef enum verity_blob_kind {
    VERITY_BLOB_STRINGS = 1,
    VERITY_BLOB_HGSS = 2,
    VERITY_BLOB_PT = 3,
    VERITY_BLOB_DP = 4,
    VERITY_BLOB_MOVES4 = 5,
    VERITY_BLOB_ENC_DPPT = 6,
    VERITY_BLOB_B2W2 = 7,
    VERITY_BLOB_NAMES4 = 8,
    VERITY_BLOB_BW = 9,
    VERITY_BLOB_MOVES5 = 10,
} verity_blob_kind;

/** Name categories for verity_name (string_table in the strings blob). */
enum {
    VERITY_NAME_SPECIES = 1,
    VERITY_NAME_MOVES = 2,
    VERITY_NAME_ABILITIES = 3,
    VERITY_NAME_NATURES = 4,
    VERITY_NAME_TYPES = 5,
    VERITY_NAME_ITEMS4 = 6,
    VERITY_NAME_ITEMS5 = 7,
};

/** Slot location: box_index 0xFF is the party. */
typedef struct verity_loc {
    uint8_t box_index;
    uint8_t slot;
} verity_loc;

/** One Pokémon as the UI shows it. Strings are UTF-16, 0-terminated. */
typedef struct verity_pkm_view {
    uint16_t species;
    uint16_t held_item;
    uint16_t moves[4];
    uint16_t tid;
    uint16_t sid;
    uint16_t met_location;
    uint32_t pid;
    uint32_t exp;
    uint16_t nickname[13];
    uint16_t ot_name[9];
    uint8_t form;
    uint8_t level; /* 0 when the game's personal blob is not attached */
    uint8_t nature;
    uint8_t ability;
    uint8_t gender;
    uint8_t ball;
    uint8_t met_level;
    uint8_t version;
    uint8_t language;
    uint8_t is_egg;
    uint8_t is_shiny;
    uint8_t checksum_ok;
    uint8_t ivs[6];
    uint8_t evs[6];
    uint8_t types[2]; /* 0xFF when unknown */
} verity_pkm_view;

/**
 * Attaches a data blob (validated). The buffer must stay alive and unchanged until detached
 * (pass NULL to detach).
 */
verity_err verity_data_attach(uint32_t kind, const uint8_t *buf, size_t len);

/** Box count of the save in buf (0 if unrecognised). */
uint32_t verity_box_count(uint8_t *buf, size_t len);

/** Box name, UTF-16, 0-terminated; out holds 21 units. */
verity_err verity_box_name(uint8_t *buf, size_t len, uint32_t box_index, uint16_t *out);

/** The Pokémon at loc (species 0 when empty). */
verity_err verity_slot_view(uint8_t *buf, size_t len, verity_loc loc, verity_pkm_view *out);

/** Legality verdict. status: 0 not checked (see unsupported), 1 legal, 2 illegal. */
typedef struct verity_legality_report {
    uint8_t status;
    uint8_t check_count;
    uint8_t checks[12]; /* failed PKHeX CheckIdentifier values, verity order */
    uint8_t move_ok[4];
    uint16_t enc_location;
    uint8_t enc_slot;
    /* 1 egg, 2 origin game not supported yet, 3 data missing, 4 not Gen 4, 5 hatched,
       6 no wild encounter matched (statics/gifts/trades not checked yet) */
    uint8_t unsupported;
} verity_legality_report;

/** Legality of the Pokémon at loc (D/P/Pt wild origins for now). */
verity_err verity_legality(uint8_t *buf, size_t len, verity_loc loc, verity_legality_report *out);

/** One wild slot. index identifies it for verity_gen_insert. */
typedef struct verity_encounter {
    uint16_t index;
    uint16_t location;
    uint16_t species;
    uint8_t form;
    uint8_t level_min;
    uint8_t level_max;
    uint8_t slot_type; /* PKHeX SlotType4: 0 grass, 1 surf, 2-4 rods, ... */
    uint8_t slot_number;
    uint8_t can_radar;
} verity_encounter;

/** Species with wild slots in version (10 D, 11 P, 12 Pt); returns the count written. */
size_t verity_wild_species(uint8_t version, uint16_t *out, size_t cap);
/** Wild slots of species in version; returns the count written. */
size_t verity_wild_encounters(uint8_t version, uint16_t species, verity_encounter *out, size_t cap);

/** Legal generation request. 0xFF / 0xFFFF mean "any". */
typedef struct verity_gen_request {
    uint8_t version;
    uint8_t form;
    uint16_t species;
    uint16_t encounter_index;
    uint8_t level;
    uint8_t nature;
    uint8_t gender;
    uint8_t shiny;
    uint8_t lead; /* 0 none, 1 Synchronize (lead_param nature), 2 Cute Charm (lead_param gender), 3 radar */
    uint8_t lead_param;
    uint8_t ball;
    uint8_t met_date[3]; /* years since 2000, month, day */
    uint32_t seed;
} verity_gen_request;

/** Generates a legal Pokémon and writes it to loc. Returns a status (0 OK); see
    verity_gen_status_text. The save is not finalised. */
uint8_t verity_gen_insert(uint8_t *buf, size_t len, const verity_gen_request *req, verity_loc loc);
const char *verity_gen_status_text(uint8_t code);

/** Fixes block checksums before writing the save out. */
verity_err verity_save_finalize(uint8_t *buf, size_t len);

/** Game-safe validator verdict. issue: 0 safe, 1 party count, 2 no usable party member,
    3 a Pokémon at (box_index, slot), 4 checksums, 5 validator data missing. */
typedef struct verity_save_check_result {
    uint8_t issue;
    uint8_t box_index;
    uint8_t slot;
} verity_save_check_result;
verity_err verity_save_check(uint8_t *buf, size_t len, verity_save_check_result *out);

/** A name from the attached strings blob; returns its length (0 if unknown). */
size_t verity_name(uint32_t category, uint32_t id, uint16_t *out, size_t cap);

/** Save summary. kind: 1 D/P, 2 Pt, 3 HG/SS, 4 B/W, 5 B2/W2. */
typedef struct verity_save_info {
    uint8_t kind;
    uint8_t generation;
    uint8_t party_count;
    uint8_t checksums_valid;
    uint16_t tid;
    uint16_t sid;
    uint16_t ot_name[13]; /* UTF-16, 0-terminated */
    uint16_t played_hours;
    uint8_t played_minutes;
    uint8_t played_seconds;
} verity_save_info;

/**
 * Detects a save in buf (raw, or DeSmuME .dsv) and summarises it. buf is not modified.
 */
verity_err verity_save_probe(uint8_t *buf, size_t len, verity_save_info *out);

/**
 * Panic hook callback invoked if Rust core panics in no_std mode.
 * The front-end must implement this to display an error and halt.
 */
void verity_on_panic(void);

#ifdef __cplusplus
}
#endif

#endif /* VERITY_H */
