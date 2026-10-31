//! Linear Congruential Random Number Generator (LCRNG).
//!
//! Standard Pokémon 32-bit LCRNG:
//! Forward:  `seed = seed * 0x41C64E6D + 0x6073`
//! Backward: `seed = seed * 0xEEB9EB65 + 0x0A3561A1`
//!
//! Ported directly from PKHeX:
//! `PKHeX.Core/Legality/RNG/LCRNG.cs`

pub const LCRNG_MULT: u32 = 0x41C64E6D;
pub const LCRNG_ADD: u32 = 0x00006073;

pub const LCRNG_REVERSE_MULT: u32 = 0xEEB9EB65;
pub const LCRNG_REVERSE_ADD: u32 = 0x0A3561A1;

/// Advances the LCRNG seed by one step forward.
#[inline(always)]
pub const fn next_seed(seed: u32) -> u32 {
    seed.wrapping_mul(LCRNG_MULT).wrapping_add(LCRNG_ADD)
}

/// Steps the LCRNG seed by one step backward.
#[inline(always)]
pub const fn prev_seed(seed: u32) -> u32 {
    seed.wrapping_mul(LCRNG_REVERSE_MULT)
        .wrapping_add(LCRNG_REVERSE_ADD)
}

/// Advances the seed by `n` steps forward.
pub fn advance_seed(mut seed: u32, n: usize) -> u32 {
    for _ in 0..n {
        seed = next_seed(seed);
    }
    seed
}

/// Encrypts or decrypts a 16-bit word slice in-place using the LCRNG seed cipher.
///
/// Ported from `PKHeX.Core.PokeCrypto.CryptArray`.
pub fn crypt_array_16(data: &mut [u8], mut seed: u32) {
    debug_assert!(data.len().is_multiple_of(2));

    for chunk in data.as_chunks_mut::<2>().0 {
        seed = next_seed(seed);
        let val = u16::from_le_bytes(*chunk) ^ (seed >> 16) as u16;
        *chunk = val.to_le_bytes();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_forward_reverse_identity() {
        let seeds = [0u32, 1, 0x12345678, 0xDEADBEEF, 0xFFFFFFFF];
        for &s in &seeds {
            let next = next_seed(s);
            let prev = prev_seed(next);
            assert_eq!(
                s, prev,
                "prev_seed(next_seed(s)) should restore s for {s:#X}"
            );
        }
    }

    #[test]
    fn test_crypt_symmetry() {
        let buffer = *b"Hello, Pokemon Gen 4/5 Save Editor!";
        let mut slice = buffer;
        let seed = 0xABCD1234;

        // Encrypt
        crypt_array_16(&mut slice[..34], seed);
        assert_ne!(&slice[..34], &buffer[..34]);

        // Decrypt (same operation with same seed)
        crypt_array_16(&mut slice[..34], seed);
        assert_eq!(&slice[..34], &buffer[..34]);
    }
}
