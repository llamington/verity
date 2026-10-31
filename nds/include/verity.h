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

/**
 * Panic hook callback invoked if Rust core panics in no_std mode.
 * The front-end must implement this to display an error and halt.
 */
void verity_on_panic(void);

#ifdef __cplusplus
}
#endif

#endif /* VERITY_H */
