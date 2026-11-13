//! Solaceon Ruins Unown forms (D/P/Pt). Port of PKHeX
//! `Legality/RNG/ClassicEra/Gen4/SolaceonRuins4.cs` and `MethodFinder.GetLCRNGMethod1Match`.

use super::lcrng::{advance_seed, next15};
use super::reversal::{MAX_SEEDS_PID, seeds};

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

// ---- HG/SS Ruins of Alph (PKHeX `RuinsOfAlph4.cs`) ------------------------------------------

/// Met location of the Ruins of Alph.
pub const RUINS_OF_ALPH: u16 = 209;

/// PKHeX `RuinsOfAlph4.GetEntranceForm`.
pub fn alph_entrance_form(seed: u32) -> u8 {
    26 + ((seed >> 16) & 1) as u8
}

/// PKHeX `RuinsOfAlph4.IsFormValid(uint seed, byte form)` for a Method 1 origin seed.
pub fn alph_form_valid_for_seed(seed: u32, form: u8) -> bool {
    alph_form_valid_frame(advance_seed(seed, 6), form)
}

/// PKHeX `RuinsOfAlph4.IsFormValid(PKM, byte form)`.
pub fn alph_form_valid(pid: u32, iv32: u32, form: u8) -> bool {
    method1_origin(pid, iv32).is_none_or(|seed| alph_form_valid_for_seed(seed, form))
}

fn alph_form_valid_frame(mut seed: u32, form: u8) -> bool {
    if form >= 26 {
        return form == alph_entrance_form(seed);
    }
    let u16v = seed >> 16;
    let radio = u16v % 100 < 50;
    if radio {
        // The radio gives any not-yet-seen form, which depends on save progress.
        true
    } else {
        if alph_interior(form, u16v) {
            return true;
        }
        let next = super::lcrng::next16(&mut seed) as u32;
        alph_interior(form, next)
    }
}

const ALPH_MAX_DEPTH: usize = 4;

fn alph_interior(form: u8, rand: u32) -> bool {
    let mut forms = [0u8; 26];
    alph_recurse(&mut forms, form, rand, 0, 0)
}

/// Permutes which form sets (A-J, R-V, K-Q, W-Z) the player has unlocked. Mirrors PKHeX,
/// including the in-place reuse of `forms` across branches.
fn alph_recurse(forms: &mut [u8; 26], form: u8, rand: u32, count: usize, depth: usize) -> bool {
    if depth == ALPH_MAX_DEPTH {
        return false;
    }
    let (start, end) = match depth {
        0 => (0u8, 9u8),
        1 => (17, 21),
        2 => (10, 16),
        _ => (22, 25),
    };
    let mut new_count = count;
    for f in start..=end {
        forms[new_count] = f;
        new_count += 1;
    }
    if alph_recurse(forms, form, rand, new_count, depth + 1) {
        return true;
    }
    if forms[(rand % new_count as u32) as usize] == form {
        return true;
    }
    alph_recurse(forms, form, rand, count, depth + 1)
}
