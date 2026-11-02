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
} verity_err;

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
