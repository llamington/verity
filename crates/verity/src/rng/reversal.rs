//! Seed recovery from LCRNG outputs. Ported from PKHeX
//! `Legality/RNG/Algorithms/LCRNGReversal.cs` and `LCRNGReversalSkip.cs`.

use super::lcrng::{LCRNG_MULT, next_n, next_seed, prev_seed};

const MOD: u32 = 0x67D3;
const PAT: u32 = 0xD3E;
const INC: u32 = 0x4034;
const MOD_2: u32 = 0x3A89;
const PAT_2: u32 = 0x2E4C;
const INC_2: u32 = 0x5831;

pub const MAX_SEEDS_PID: usize = 3;
pub const MAX_SEEDS_IV: usize = 6;
pub const MAX_SEEDS_PID_SKIP: usize = 5;
pub const MAX_SEEDS_IV_SKIP: usize = 10;

fn start(diff: u32, m: u32, inc: u32, pat: u32) -> u32 {
    ((diff.wrapping_mul(m).wrapping_add(inc) >> 16).wrapping_mul(pat)) % m
}

/// Seeds whose next two outputs have the given high halves (`first`/`second` in the upper
/// 16 bits). Returned seeds are the state *before* the first output.
pub fn seeds(out: &mut [u32; MAX_SEEDS_PID], first: u32, second: u32) -> usize {
    let diff = second.wrapping_sub(first.wrapping_mul(LCRNG_MULT)) >> 16;
    let mut low = start(diff, MOD, INC, PAT);
    let mut n = 0;
    loop {
        let seed = first | low;
        if next_seed(seed) & 0xFFFF_0000 == second && n < out.len() {
            out[n] = prev_seed(seed);
            n += 1;
        }
        low += MOD;
        if low >= 0x1_0000 {
            break;
        }
    }
    n
}

/// Seeds whose next two outputs have the given 15-bit IV halves (bit 31 unknown).
pub fn seeds_ivs(out: &mut [u32; MAX_SEEDS_IV], first: u32, second: u32) -> usize {
    let diff = second.wrapping_sub(first.wrapping_mul(LCRNG_MULT)) >> 16;
    let mut n = 0;
    for s in [
        start(diff, MOD, INC, PAT),
        start(diff ^ 0x8000, MOD, INC, PAT),
    ] {
        let mut low = s;
        loop {
            let test = first | low;
            if next_seed(test) & 0x7FFF_0000 == second && n + 2 <= out.len() {
                let seed = prev_seed(test);
                out[n] = seed;
                out[n + 1] = seed ^ 0x8000_0000;
                n += 2;
            }
            low += MOD;
            if low >= 0x1_0000 {
                break;
            }
        }
    }
    n
}

pub fn seeds_from_ivs(out: &mut [u32; MAX_SEEDS_IV], ivs: [u8; 6]) -> usize {
    let [hp, atk, def, spe, spa, spd] = ivs.map(u32::from);
    let first = (hp | atk << 5 | def << 10) << 16;
    let second = (spe | spa << 5 | spd << 10) << 16;
    seeds_ivs(out, first, second)
}

/// Like [`seeds`], with one unknown output between `first` and `third`.
pub fn seeds_skip(out: &mut [u32; MAX_SEEDS_PID_SKIP], first: u32, third: u32) -> usize {
    let diff = third.wrapping_sub(next_n::<2>(first)) >> 16;
    let mut low = start(diff, MOD_2, INC_2, PAT_2);
    let mut n = 0;
    loop {
        let seed = first | low;
        if next_n::<2>(seed) & 0xFFFF_0000 == third && n < out.len() {
            out[n] = prev_seed(seed);
            n += 1;
        }
        low += MOD_2;
        if low >= 0x1_0000 {
            break;
        }
    }
    n
}

pub fn seeds_ivs_skip(out: &mut [u32; MAX_SEEDS_IV_SKIP], first: u32, third: u32) -> usize {
    let diff = third.wrapping_sub(next_n::<2>(first)) >> 16;
    let mut n = 0;
    for s in [
        start(diff, MOD_2, INC_2, PAT_2),
        start(diff ^ 0x8000, MOD_2, INC_2, PAT_2),
    ] {
        let mut low = s;
        loop {
            let test = first | low;
            if next_n::<2>(test) & 0x7FFF_0000 == third && n + 2 <= out.len() {
                let seed = prev_seed(test);
                out[n] = seed;
                out[n + 1] = seed ^ 0x8000_0000;
                n += 2;
            }
            low += MOD_2;
            if low >= 0x1_0000 {
                break;
            }
        }
    }
    n
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Rng(u64);
    impl Rng {
        fn next(&mut self) -> u32 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 7;
            self.0 ^= self.0 << 17;
            self.0 as u32
        }
    }

    fn brute(first: u32, target: u32, mask: u32, skip: bool) -> alloc::vec::Vec<u32> {
        let mut v = alloc::vec::Vec::new();
        for hi_bit in [0u32, 0x8000_0000] {
            if mask == 0xFFFF_0000 && hi_bit != 0 {
                continue;
            }
            for low in 0..0x1_0000u32 {
                let test = (first | hi_bit) | low;
                let out = if skip {
                    next_n::<2>(test)
                } else {
                    next_seed(test)
                };
                if out & mask == target {
                    v.push(prev_seed(test));
                }
            }
        }
        v.sort_unstable();
        v
    }

    fn check(rng: &mut Rng) {
        // Half the inputs come from a real seed, so a solution exists.
        let real = rng.next() & 1 == 0;
        let s0 = rng.next();
        let a = next_seed(s0);
        let (first, second, third) = if real {
            (
                a & 0xFFFF_0000,
                next_seed(a) & 0xFFFF_0000,
                next_n::<2>(a) & 0xFFFF_0000,
            )
        } else {
            (
                rng.next() & 0xFFFF_0000,
                rng.next() & 0xFFFF_0000,
                rng.next() & 0xFFFF_0000,
            )
        };

        let mut o = [0u32; MAX_SEEDS_PID];
        let n = seeds(&mut o, first, second);
        let mut got = o[..n].to_vec();
        got.sort_unstable();
        assert_eq!(got, brute(first, second, 0xFFFF_0000, false));
        if real {
            assert!(got.contains(&s0));
        }

        let mut o = [0u32; MAX_SEEDS_PID_SKIP];
        let n = seeds_skip(&mut o, first, third);
        let mut got = o[..n].to_vec();
        got.sort_unstable();
        assert_eq!(got, brute(first, third, 0xFFFF_0000, true));

        let (f15, s15, t15) = (
            first & 0x7FFF_0000,
            second & 0x7FFF_0000,
            third & 0x7FFF_0000,
        );
        let mut o = [0u32; MAX_SEEDS_IV];
        let n = seeds_ivs(&mut o, f15, s15);
        let mut got = o[..n].to_vec();
        got.sort_unstable();
        assert_eq!(got, brute(f15, s15, 0x7FFF_0000, false));

        let mut o = [0u32; MAX_SEEDS_IV_SKIP];
        let n = seeds_ivs_skip(&mut o, f15, t15);
        let mut got = o[..n].to_vec();
        got.sort_unstable();
        assert_eq!(got, brute(f15, t15, 0x7FFF_0000, true));
    }

    #[test]
    fn reversal_matches_brute_force() {
        let mut rng = Rng(0x5EED_1234_ABCD_0001);
        for _ in 0..300 {
            check(&mut rng);
        }
    }

    /// PLAN P4 acceptance: 10^6 random inputs. Run with `--release -- --ignored`.
    #[test]
    #[ignore]
    fn reversal_matches_brute_force_1e6() {
        extern crate std;
        let threads = 16;
        let handles: alloc::vec::Vec<_> = (0..threads)
            .map(|t| {
                std::thread::spawn(move || {
                    let mut rng = Rng(0xC0FF_EE00_0000_0001 ^ (t as u64) << 32);
                    for _ in 0..1_000_000 / threads {
                        check(&mut rng);
                    }
                })
            })
            .collect();
        for h in handles {
            h.join().unwrap();
        }
    }
}
