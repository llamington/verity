//! Solaceon Ruins Unown forms (D/P/Pt). Port of PKHeX
//! `Legality/RNG/ClassicEra/Gen4/SolaceonRuins4.cs` and `MethodFinder.GetLCRNGMethod1Match`.

use super::lcrng::{advance_seed, next15};
use super::reversal::{seeds, MAX_SEEDS_PID};

/// Met location of the Solaceon Ruins.
pub const SOLACEON_RUINS: u16 = 53;

/// Unown forms available in every Dead End room.
const UNOWN0: [u8; 20] = [
    0, 1, 2, 6, 7, 9, 10, 11, 12, 14, 15, 16, 18, 19, 20, 21, 22, 23, 24, 25,
];

/// Forms that share a room with others; F, R, I, E, N, D each have a room of their own.
const FORM_RANDOM_UNOWN: u32 = 0b1111111111011101111011000111;

/// The single-form rooms always give that form.
pub fn is_single_form_room(form: u8) -> bool {
    form >= 32 || FORM_RANDOM_UNOWN & (1 << form) == 0
}

fn unown_form(rand: u32, prefer: u8) -> u8 {
    if prefer >= 26 {
        return 26 + (rand & 1) as u8;
    }
    UNOWN0[(rand % UNOWN0.len() as u32) as usize]
}

/// PKHeX `SolaceonRuins4.IsFormValid(uint seed, byte form)`; `seed` is the Method 1 origin
/// (the state before the PID's first call).
pub fn form_valid_for_seed(seed: u32, form: u8) -> bool {
    if is_single_form_room(form) {
        return true;
    }
    // ABCD | E (item) | F (form)
    let f = advance_seed(seed, 6) >> 16;
    unown_form(f, form) == form
}

/// PKHeX `MethodFinder.GetLCRNGMethod1Match`: the first Method 1 origin seed for the spread.
pub fn method1_origin(pid: u32, iv32: u32) -> Option<u32> {
    let (iv1, iv2) = (iv32 & 0x7FFF, (iv32 >> 15) & 0x7FFF);
    let mut buf = [0u32; MAX_SEEDS_PID];
    let n = seeds(&mut buf, pid << 16, pid & 0xFFFF_0000);
    buf[..n].iter().copied().find(|&seed| {
        let mut s = advance_seed(seed, 2);
        next15(&mut s) as u32 == iv1 && next15(&mut s) as u32 == iv2
    })
}

/// PKHeX `SolaceonRuins4.IsFormValid(PKM, byte form)`.
pub fn form_valid(pid: u32, iv32: u32, form: u8) -> bool {
    if is_single_form_room(form) {
        return true;
    }
    method1_origin(pid, iv32).is_none_or(|seed| form_valid_for_seed(seed, form))
}
