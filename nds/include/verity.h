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
    VERITY_BLOB_ENC_HGSS = 11,
    VERITY_BLOB_GIFTS4 = 12, /* Gen 4 event database (gifts4.bin); optional */
    VERITY_BLOB_G3_ORIGIN = 14, /* Gen 3 origin data (g3_origin.bin); optional */
    VERITY_BLOB_EVENTS4 = 13, /* Gen 4 non-Pokemon gifts (events4.bin); for the gift screen */
    VERITY_BLOB_ENC_BW = 15,
    VERITY_BLOB_ENC_B2W2 = 16,
    VERITY_BLOB_ENC_DR = 17,
    VERITY_BLOB_NAMES5 = 18,
    VERITY_BLOB_GIFTS5 = 19, /* Gen 5 event database (gifts5.bin); optional */
    VERITY_BLOB_EVENTS5 = 20, /* Gen 5 card availability (events5.bin) */
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
    uint8_t friendship;
    uint8_t is_nicknamed;
    uint8_t met_date[3]; /* years since 2000, month, day */
    uint8_t contest[6];  /* Cool, Beauty, Cute, Smart, Tough, Sheen */
    uint8_t ribbons[12]; /* bytes 0x24..0x28, 0x3C..0x40, 0x60..0x64 */
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
       6 no encounter matched (events/trades/Pokewalker not checked yet) */
    uint8_t unsupported;
} verity_legality_report;

/** Legality of the Pokémon at loc (Gen 4 wild slots and statics for now). */
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

/** One Mystery Gift the open save can take. status: 0 available, else a verity_event_inject code. */
typedef struct verity_event_view {
    uint8_t blob;      /* 0 gifts4.bin, 1 events4.bin */
    uint8_t gift_type; /* PKHeX GiftType4 */
    uint16_t record;   /* position in that blob */
    uint16_t card_id;
    uint16_t species;  /* 0 for items */
    uint8_t region;    /* 0 none, 1 NA, 2 UK, 3 AU, 4 EU */
    uint8_t status;
    uint8_t ranger;    /* 1 for the Ranger Manaphy egg (it has no title) */
    uint8_t reserved;
    uint16_t title[37]; /* UTF-16, 0-terminated */
} verity_event_view;

/** Gifts for this save's game and language; returns how many were written (at most cap). */
size_t verity_events_list(uint8_t *buf, size_t len, verity_event_view *out, size_t cap);

/** Delivers a gift for the player to collect from the deliveryman. days: days since 2000-01-01.
    Returns 0 on success; see verity_event_status_text. The save is not finalised. */
uint8_t verity_event_inject(uint8_t *buf, size_t len, uint8_t blob, uint16_t record, uint16_t days);
const char *verity_event_status_text(uint8_t code);

/** What linking another save into the open B2W2 save would do. mode: 0 unusable, 1 Memory Link
    (Black or White save), 2 keys (Black 2 or White 2 save), 3 enable all keys. status: 0 ready, 1 replaces another
    player's link, else see verity_unova_status_text. keys: Easy, Challenge, City, Iron, Iceberg. */
typedef struct verity_unova_preview_t {
    uint8_t mode;
    uint8_t status;
    uint8_t flashback;
    uint8_t keys[5];
} verity_unova_preview_t;

void verity_unova_preview(const uint8_t *buf, size_t len, const uint8_t *other, size_t other_len,
                          bool replace, verity_unova_preview_t *out);
/** Returns 0 on success, else a status code. The save is not finalised. */
uint8_t verity_unova_apply(uint8_t *buf, size_t len, const uint8_t *other, size_t other_len,
                           bool replace, uint32_t seed);
/** Enabling every key without a partner save; mode is 3. Returns 0 on success, else a status. */
void verity_unova_enable_preview(const uint8_t *buf, size_t len, verity_unova_preview_t *out);
uint8_t verity_unova_enable_all(uint8_t *buf, size_t len);
const char *verity_unova_status_text(uint8_t status);

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

/** Met/egg location name (generation 4 or 5); returns its length (0 if unknown). */
size_t verity_location_name(uint8_t generation, uint16_t id, uint16_t *out, size_t cap);

/** Field edits; mask: 1 held item, 2 moves (PP reset), 4 IVs, 8 EVs, 16 level, 32 friendship,
    64 PID (only with a verity_closest_ivs result), 128 ball, 256 met location/level/date,
    512 contest stats, 1024 ribbons, 2048 nickname, 4096 reset nickname to the species name. */
typedef struct verity_edit {
    uint32_t mask;
    uint16_t held_item;
    uint16_t moves[4];
    uint8_t ivs[6];
    uint8_t evs[6];
    uint8_t level;
    uint8_t friendship;
    uint32_t pid;
    uint16_t met_location;
    uint8_t ball;
    uint8_t met_level;
    uint8_t met_date[3];
    uint8_t contest[6];
    uint8_t ribbons[12];
    uint16_t nickname[11]; /* UTF-16, 0-terminated */
} verity_edit;

/** Whether a character can be stored in a name in this generation. */
uint8_t verity_char_encodable(uint8_t generation, uint16_t c);

/** Species an egg can be of in this game; returns how many were written. */
size_t verity_egg_species(uint8_t version, uint16_t *out, size_t cap);

/** Request for verity_egg_insert. 0xFF means any. */
typedef struct verity_egg_request {
    uint8_t version;
    uint8_t nature;
    uint16_t species;
    uint8_t gender;
    uint8_t shiny;
    uint8_t hatched;  /* 0 an egg, 1 already hatched */
    uint8_t date[3];  /* years since 2000, month, day */
    uint32_t seed;
} verity_egg_request;

/** Builds a legal bred egg for the save's trainer and writes it to loc. Returns 0 OK, else a code
    for verity_gen_status_text. The save is not finalised. */
uint8_t verity_egg_insert(uint8_t *buf, size_t len, const verity_egg_request *req, verity_loc loc);

/** The legal IV spread closest to the requested one, and the PID that goes with it. */
typedef struct verity_iv_fix {
    uint8_t ivs[6];
    uint16_t distance;
    uint32_t pid;
} verity_iv_fix;

/** Finds the legal IVs closest to target (HP Atk Def Spe SpA SpD) for the Gen 4 wild catch at loc,
    keeping nature, gender, ability and shininess. Writes nothing. Returns 0 OK, else a code for
    verity_gen_status_text. */
uint8_t verity_closest_ivs(uint8_t *buf, size_t len, verity_loc loc, const uint8_t *target,
                           verity_iv_fix *out);

/** Previews an edit: game-safe issue (0 safe) and legality, without writing. */
verity_err verity_edit_preview(uint8_t *buf, size_t len, verity_loc loc, const verity_edit *edit,
                               verity_legality_report *legality, uint8_t *issue);
/** Writes an edit through the validator. mode 0 legal (refuses illegal), 1 game-safe.
    Returns 0 OK, else a code for verity_issue_text. */
uint8_t verity_edit_apply(uint8_t *buf, size_t len, verity_loc loc, const verity_edit *edit, uint8_t mode);
const char *verity_issue_text(uint8_t code);

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
    uint8_t game_version; /* 20 W, 21 B, 22 W2, 23 B2; 0 on Gen 4 */
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

/* Nickname keyboard: types key into text (*len characters, capacity cap, at most max), applying
 * the voicing marks 0x309B/0x309C to the last kana and composing Hangul jamo. 1 if it changed. */
uint8_t verity_keyboard_type(uint16_t *text, size_t *len, size_t cap, size_t max, uint16_t key,
                             uint8_t generation);

/* ---- ROM font (crates/verity-ffi/src/rom.rs, docs/research/rom-font.md) ---- */

/* Reads len bytes at offset of the ROM file into buf; false on failure. */
typedef bool (*verity_read_fn)(void *ctx, uint32_t offset, uint8_t *buf, uint32_t len);

/* How good a ROM's font is (3 any Gen 5, 2 Korean Gen 4, 1 other Gen 4, 0 unsupported), from the
 * 4-letter game code at header offset 0x0C. */
uint8_t verity_rom_font_rank(const uint8_t *code);
/* Builds the font cache file from a ROM; free the result with verity_rom_free. Null on failure. */
uint8_t *verity_rom_build_font_cache(verity_read_fn read, void *ctx, size_t *out_len);
void verity_rom_free(uint8_t *p, size_t len);

/* Bit x of rows[y] is an ink pixel (stroke or shadow); ink starts at column 0 and is width wide.
 * left is where the game would place it after the pen (verity uses 1 px gaps instead). */
typedef struct verity_glyph {
    uint8_t left;
    uint8_t width;
    uint8_t advance;
    uint8_t height;
    uint16_t rows[16];
} verity_glyph;

/* True if the buffer is a usable font cache; call once after reading the file. */
bool verity_font_check(const uint8_t *p, size_t len);
bool verity_font_glyph(const uint8_t *p, size_t len, uint16_t c, verity_glyph *out);

#ifdef __cplusplus
}
#endif

#endif /* VERITY_H */
