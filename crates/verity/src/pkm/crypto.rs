//! Pokémon entity encryption, decryption, block shuffling, and checksum verification.
//!
//! Ported directly from PKHeX:
//! `PKHeX.Core/PKM/Util/PokeCrypto.cs`

use crate::rng::lcrng::crypt_array_16;

pub const SIZE_4STORED: usize = 136;
pub const SIZE_4PARTY: usize = 236;
pub const SIZE_5STORED: usize = 136;
pub const SIZE_5PARTY: usize = 220;
pub const SIZE_BLOCK: usize = 32;
pub const BLOCK_COUNT: usize = 4;

/// Block shuffle permutation table (32 entries × 4 blocks).
/// Entries 24..31 duplicate 0..7 to eliminate modulo operations.
#[rustfmt::skip]
pub static BLOCK_POSITION: [u8; 128] = [
    0, 1, 2, 3,  0, 1, 3, 2,  0, 2, 1, 3,  0, 3, 1, 2,
    0, 2, 3, 1,  0, 3, 2, 1,  1, 0, 2, 3,  1, 0, 3, 2,
    2, 0, 1, 3,  3, 0, 1, 2,  2, 0, 3, 1,  3, 0, 2, 1,
    1, 2, 0, 3,  1, 3, 0, 2,  2, 1, 0, 3,  3, 1, 0, 2,
    2, 3, 0, 1,  3, 2, 0, 1,  1, 2, 3, 0,  1, 3, 2, 0,
    2, 1, 3, 0,  3, 1, 2, 0,  2, 3, 1, 0,  3, 2, 1, 0,

    // Duplicates of 0..7
    0, 1, 2, 3,  0, 1, 3, 2,  0, 2, 1, 3,  0, 3, 1, 2,
    0, 2, 3, 1,  0, 3, 2, 1,  1, 0, 2, 3,  1, 0, 3, 2,
];

/// Inverted permutation lookup table for encryption.
#[rustfmt::skip]
pub static BLOCK_POSITION_INVERT: [u8; 32] = [
    0, 1, 2, 4,
    3, 5, 6, 7,
    12, 18, 13, 19,
    8, 10, 14, 20,
    16, 22, 9, 11,
    15, 21, 17, 23,

    // Duplicates of 0..7
    0, 1, 2, 4,
    3, 5, 6, 7,
];

/// Calculates the 16-bit sanity checksum of a Pokémon (sum of u16 words in bytes 0x08..0x88).
#[inline]
pub fn calc_checksum(data: &[u8]) -> u16 {
    debug_assert!(data.len() >= SIZE_4STORED);
    let mut sum: u16 = 0;
    for chunk in data[8..SIZE_4STORED].as_chunks::<2>().0 {
        sum = sum.wrapping_add(u16::from_le_bytes(*chunk));
    }
    sum
}

/// Shuffles or un-shuffles the 4 blocks of 32 bytes in `data` (which must be 128 bytes).
pub fn shuffle_blocks(data: &mut [u8], sv: u32) {
    debug_assert_eq!(data.len(), SIZE_BLOCK * BLOCK_COUNT);
    let sv_idx = (sv as usize & 31) * BLOCK_COUNT;
    let order = &BLOCK_POSITION[sv_idx..sv_idx + BLOCK_COUNT];

    let mut temp = [0u8; SIZE_BLOCK * BLOCK_COUNT];
    temp.copy_from_slice(data);

    for (dst_block, &src_block) in order.iter().enumerate() {
        let src_start = src_block as usize * SIZE_BLOCK;
        let dst_start = dst_block * SIZE_BLOCK;
        data[dst_start..dst_start + SIZE_BLOCK]
            .copy_from_slice(&temp[src_start..src_start + SIZE_BLOCK]);
    }
}

/// Decrypts a Gen 4 or Gen 5 Pokémon byte buffer in-place.
///
/// Ported from `PKHeX.Core.PokeCrypto.Decrypt45`.
pub fn decrypt_45(data: &mut [u8]) {
    assert!(
        data.len() == SIZE_4STORED || data.len() == SIZE_4PARTY || data.len() == SIZE_5PARTY,
        "Invalid PKM buffer length: {}",
        data.len()
    );

    let pv = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    let chk = u16::from_le_bytes([data[6], data[7]]) as u32;
    let sv = (pv >> 13) & 31;

    // 1. Decrypt blocks 8..136 using checksum seed
    crypt_array_16(&mut data[8..SIZE_4STORED], chk);

    // 2. Decrypt party stats if present using PV seed
    if data.len() > SIZE_4STORED {
        crypt_array_16(&mut data[SIZE_4STORED..], pv);
    }

    // 3. Unshuffle blocks
    shuffle_blocks(&mut data[8..SIZE_4STORED], sv);
}

/// Encrypts a decrypted Gen 4 or Gen 5 Pokémon byte buffer in-place.
///
/// Ported from `PKHeX.Core.PokeCrypto.Encrypt45`.
pub fn encrypt_45(data: &mut [u8]) {
    assert!(
        data.len() == SIZE_4STORED || data.len() == SIZE_4PARTY || data.len() == SIZE_5PARTY,
        "Invalid PKM buffer length: {}",
        data.len()
    );

    let pv = u32::from_le_bytes([data[0], data[1], data[2], data[3]]);
    let chk = u16::from_le_bytes([data[6], data[7]]) as u32;
    let sv = (pv >> 13) & 31;
    let sv_inv = BLOCK_POSITION_INVERT[sv as usize] as u32;

    // 1. Shuffle blocks into encrypted order
    shuffle_blocks(&mut data[8..SIZE_4STORED], sv_inv);

    // 2. Encrypt blocks 8..136 using checksum seed
    crypt_array_16(&mut data[8..SIZE_4STORED], chk);

    // 3. Encrypt party stats if present using PV seed
    if data.len() > SIZE_4STORED {
        crypt_array_16(&mut data[SIZE_4STORED..], pv);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_shuffle_invert_table() {
        for sv in 0..24u32 {
            let inv = BLOCK_POSITION_INVERT[sv as usize] as u32;
            let mut original = [0u8; 128];
            for (i, val) in original.iter_mut().enumerate() {
                *val = i as u8;
            }
            let mut shuffled = original;
            shuffle_blocks(&mut shuffled, sv);
            shuffle_blocks(&mut shuffled, inv);
            assert_eq!(original, shuffled, "Failed invert check for sv={sv}");
        }
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let mut pkm = [0u8; SIZE_4STORED];
        // Set PID
        let pid: u32 = 0x5F12A89C;
        pkm[0..4].copy_from_slice(&pid.to_le_bytes());
        // Populate dummy block data
        for (i, byte) in pkm[8..SIZE_4STORED].iter_mut().enumerate() {
            *byte = ((i + 8) * 7) as u8;
        }
        // Compute checksum
        let chk = calc_checksum(&pkm);
        pkm[6..8].copy_from_slice(&chk.to_le_bytes());

        let original = pkm;

        // Encrypt then Decrypt
        encrypt_45(&mut pkm);
        assert_ne!(original, pkm, "Encrypted data must differ from plaintext");

        decrypt_45(&mut pkm);
        assert_eq!(
            original, pkm,
            "Decrypted data must match original plaintext"
        );
    }
}
