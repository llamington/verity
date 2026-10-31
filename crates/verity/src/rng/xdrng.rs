//! Colosseum/XD RNG and ARNG. Ported from PKHeX `Legality/RNG/Algorithms/XDRNG.cs`, `ARNG.cs`.

pub const MULT: u32 = 0x0003_43FD;
pub const ADD: u32 = 0x0026_9EC3;
pub const R_MULT: u32 = 0xB9B3_3155;
pub const R_ADD: u32 = 0xA170_F641;

/// PKHeX's `MaxCountSeedsPID` is 2, but `Analyze` passes a 6-entry buffer; up to 4 can occur.
pub const MAX_SEEDS_PID: usize = 4;
pub const MAX_SEEDS_IV: usize = 6;

#[inline(always)]
pub const fn next(seed: u32) -> u32 {
    seed.wrapping_mul(MULT).wrapping_add(ADD)
}

#[inline(always)]
pub const fn prev(seed: u32) -> u32 {
    seed.wrapping_mul(R_MULT).wrapping_add(R_ADD)
}

pub const fn next_n(mut seed: u32, n: u32) -> u32 {
    let mut i = 0;
    while i < n {
        seed = next(seed);
        i += 1;
    }
    seed
}

pub const fn prev_n(mut seed: u32, n: u32) -> u32 {
    let mut i = 0;
    while i < n {
        seed = prev(seed);
        i += 1;
    }
    seed
}

#[inline(always)]
pub fn next16(seed: &mut u32) -> u32 {
    *seed = next(*seed);
    *seed >> 16
}

#[inline(always)]
pub fn next15(seed: &mut u32) -> u32 {
    *seed = next(*seed);
    (*seed >> 16) & 0x7FFF
}

#[inline(always)]
pub fn next5(seed: &mut u32) -> u32 {
    *seed = next(*seed);
    *seed >> 27
}

pub fn sequential_iv32(mut seed: u32) -> u32 {
    (0..6).fold(0, |acc, i| acc | next5(&mut seed) << (i * 5))
}

const SUB: u32 = ADD.wrapping_sub(0xFFFF);
const BASE: u64 = (MULT as u64 + 1) * 0xFFFF;

/// PKHeX `XDRNG.GetSeeds(first, second)`.
pub fn seeds(out: &mut [u32; MAX_SEEDS_PID], first: u32, second: u32) -> usize {
    let mut t = second
        .wrapping_sub(first.wrapping_mul(MULT))
        .wrapping_sub(SUB) as u64;
    let kmax = BASE.wrapping_sub(t) >> 32;
    let mut n = 0;
    let mut k = 0u64;
    while k <= kmax {
        if t % (MULT as u64) < 0x1_0000 && n < out.len() {
            out[n] = prev(first | (t / (MULT as u64)) as u16 as u32);
            n += 1;
        }
        k += 1;
        t = t.wrapping_add(0x1_0000_0000);
    }
    n
}

/// PKHeX `XDRNG.GetSeedsIVs(first, second)`.
pub fn seeds_ivs(out: &mut [u32; MAX_SEEDS_IV], first: u32, second: u32) -> usize {
    let mut t = (second
        .wrapping_sub(first.wrapping_mul(MULT))
        .wrapping_sub(SUB)
        & 0x7FFF_FFFF) as u64;
    let kmax = BASE.wrapping_sub(t) >> 31;
    let mut n = 0;
    let mut k = 0u64;
    while k <= kmax {
        if t % (MULT as u64) < 0x1_0000 && n + 2 <= out.len() {
            let s = prev(first | (t / (MULT as u64)) as u16 as u32);
            out[n] = s;
            out[n + 1] = s ^ 0x8000_0000;
            n += 2;
        }
        k += 1;
        t = t.wrapping_add(0x8000_0000);
    }
    n
}

pub mod arng {
    pub const MULT: u32 = 0x6C07_8965;
    pub const ADD: u32 = 1;
    pub const R_MULT: u32 = 0x9638_806D;
    pub const R_ADD: u32 = 0x69C7_7F93;

    pub const fn next(seed: u32) -> u32 {
        seed.wrapping_mul(MULT).wrapping_add(ADD)
    }

    pub const fn prev(seed: u32) -> u32 {
        seed.wrapping_mul(R_MULT).wrapping_add(R_ADD)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn inverse() {
        for s in [0u32, 1, 0xDEAD_BEEF, 0xFFFF_FFFF] {
            assert_eq!(prev(next(s)), s);
            assert_eq!(arng::prev(arng::next(s)), s);
        }
    }

    #[test]
    fn seeds_recover_origin() {
        let mut s0 = 0x1357_9BDFu32;
        for _ in 0..2000 {
            s0 = next(s0).rotate_left(7) ^ 0x9E37_79B9;
            let a = next(s0);
            let b = next(a);
            let mut o = [0u32; MAX_SEEDS_PID];
            let n = seeds(&mut o, a & 0xFFFF_0000, b & 0xFFFF_0000);
            assert!(o[..n].contains(&s0));
            let mut o = [0u32; MAX_SEEDS_IV];
            let n = seeds_ivs(&mut o, a & 0x7FFF_0000, b & 0x7FFF_0000);
            assert!(o[..n].contains(&s0));
        }
    }
}
