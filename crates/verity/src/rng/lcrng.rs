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

/// (mult, add) for advancing `n` steps, computed by repeated composition.
pub const fn jump_constants(n: u32) -> (u32, u32) {
    let (mut m, mut a) = (1u32, 0u32);
    let mut i = 0;
    while i < n {
        m = m.wrapping_mul(LCRNG_MULT);
        a = a.wrapping_mul(LCRNG_MULT).wrapping_add(LCRNG_ADD);
        i += 1;
    }
    (m, a)
}

/// (mult, add) for advancing 2^i steps, i in 0..32 (PKHeX `LCRNG.JumpMult`/`JumpAdd`).
pub const JUMP: [(u32, u32); 32] = {
    let mut t = [(0u32, 0u32); 32];
    let (mut m, mut a) = (LCRNG_MULT, LCRNG_ADD);
    let mut i = 0;
    while i < 32 {
        t[i] = (m, a);
        a = a.wrapping_mul(m).wrapping_add(a);
        m = m.wrapping_mul(m);
        i += 1;
    }
    t
};

/// Advances the seed by `n` steps forward in O(log n).
pub fn advance_seed(mut seed: u32, n: usize) -> u32 {
    let mut n = n as u64;
    let mut i = 0;
    while n != 0 && i < 32 {
        if n & 1 != 0 {
            let (m, a) = JUMP[i];
            seed = seed.wrapping_mul(m).wrapping_add(a);
        }
        n >>= 1;
        i += 1;
    }
    seed
}

/// `n` steps backward.
pub fn reverse_seed(seed: u32, n: usize) -> u32 {
    let n = (n as u64 % (1u64 << 32)) as u32;
    advance_seed(seed, n.wrapping_neg() as usize)
}

#[inline(always)]
pub const fn next_n<const N: u32>(seed: u32) -> u32 {
    let (m, a) = jump_constants(N);
    seed.wrapping_mul(m).wrapping_add(a)
}

/// Advances and returns the high 16 bits (PKHeX `Next16`).
#[inline(always)]
pub fn next16(seed: &mut u32) -> u16 {
    *seed = next_seed(*seed);
    (*seed >> 16) as u16
}

/// Advances and returns the high 15 bits (PKHeX `Next15`).
#[inline(always)]
pub fn next15(seed: &mut u32) -> u16 {
    *seed = next_seed(*seed);
    ((*seed >> 16) & 0x7FFF) as u16
}

#[inline(always)]
pub fn prev16(seed: &mut u32) -> u16 {
    *seed = prev_seed(*seed);
    (*seed >> 16) as u16
}

/// Number of steps from `start` to `end` (PKHeX `LCRNG.GetDistance`).
pub fn distance(start: u32, end: u32) -> u32 {
    let mut seed = start;
    let mut dist = 0u32;
    let mut i = 0;
    while seed != end && i < 32 {
        let bit = 1u32 << i;
        if (seed ^ end) & bit != 0 {
            let (m, a) = JUMP[i];
            seed = seed.wrapping_mul(m).wrapping_add(a);
            dist |= bit;
        }
        i += 1;
    }
    dist
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
    fn jump_table_matches_pkhex() {
        // PKHeX LCRNG.JumpMult / JumpAdd, first and last entries.
        assert_eq!(JUMP[0], (0x41C64E6D, 0x00006073));
        assert_eq!(JUMP[1], (0xC2A29A69, 0xE97E7B6A));
        assert_eq!(JUMP[4], (0x5F748241, 0xCBA72510));
        assert_eq!(JUMP[29], (0x80000001, 0x20000000));
        assert_eq!(JUMP[31], (0x00000001, 0x80000000));
        assert_eq!(jump_constants(8), (0xCFDDDF21, 0x67DBB608));
        assert_eq!(jump_constants(9).1, 0xFC3351DB);
        assert_eq!(jump_constants(4), (0xEE067F11, 0x31B0DDE4));
    }

    #[test]
    fn advance_and_distance() {
        let mut seed = 0x1234_5678u32;
        let start = seed;
        for n in 1..3000usize {
            seed = next_seed(seed);
            assert_eq!(advance_seed(start, n), seed);
            assert_eq!(distance(start, seed), n as u32);
            assert_eq!(reverse_seed(seed, n), start);
        }
        assert_eq!(next_n::<5>(start), advance_seed(start, 5));
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
