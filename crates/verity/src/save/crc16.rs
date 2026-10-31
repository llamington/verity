//! CRC16-CCITT implementation used by Gen 4 and Gen 5 save structures.
//!
//! Ported directly from PKHeX:
//! `PKHeX.Core/Saves/Util/Checksums.cs` -> `Checksums.CRC16_CCITT`

/// Calculates the CRC16-CCITT checksum over a slice of bytes.
///
/// In Gen 4 and Gen 5 saves, this algorithm verifies the integrity of
/// save blocks and partitions.
#[inline]
pub fn crc16_ccitt(data: &[u8]) -> u16 {
    let mut top: u8 = 0xFF;
    let mut bot: u8 = 0xFF;

    for &b in data {
        let mut x = b ^ top;
        x ^= x >> 4;
        top = bot ^ (x >> 3) ^ (x << 4);
        bot = x ^ (x << 5);
    }

    ((top as u16) << 8) | (bot as u16)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_empty_slice() {
        assert_eq!(crc16_ccitt(&[]), 0xFFFF);
    }

    #[test]
    fn test_single_byte() {
        // top = 0xFF, bot = 0xFF
        // b = 0
        // x = 0 ^ 0xFF = 0xFF
        // x ^= 0x0F = 0xF0
        // top = 0xFF ^ (0xF0 >> 3) ^ (0xF0 << 4) = 0xFF ^ 0x1E ^ 0x00 = 0xE1
        // bot = 0xF0 ^ (0xF0 << 5) = 0xF0 ^ 0x00 = 0xF0
        assert_eq!(crc16_ccitt(&[0x00]), 0xE1F0);
    }

    #[test]
    fn test_known_pattern() {
        // "123456789" ASCII
        let input = b"123456789";
        assert_eq!(crc16_ccitt(input), 0x29B1);
    }
}
