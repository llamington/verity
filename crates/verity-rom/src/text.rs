//! The games' message banks (NARC members of text lines). Research: `docs/research/rom-text.md`.
//!
//! Gen 4 (pokeplatinum `src/message.c`): `u16` count, `u16` seed, then per line an encrypted
//! `u32` offset and `u32` length (in characters), XORed with `(seed * 765 * (i + 1)) & 0xFFFF`
//! repeated in both halves. A line's characters are XORed with a `u16` key that starts at
//! `(i + 1) * 596947` and grows by 18749 per character. Characters are the game's codes; 0xE000 is
//! a line break, 0xFFFE starts a control code, 0xFFFF ends the line. Lines starting 0xF100 are
//! packed 9 bits per character.
//!
//! Gen 5 (pokebw2 `tools/text/msgdata.py`): `u16` sections, `u16` lines, ..., `u32` offset of the
//! first section at 12; a section is `u32` size then per line `u32` offset (from the section),
//! `u16` length, `u16` unused. Characters are UTF-16, XORed from the last one back with a key that
//! starts at `last ^ 0xFFFF` and rotates right by 3 each step. 0xFFFE is a line break, 0xF000 and
//! up are control codes, 0xFFFF ends the line; 0xF100 lines are 9-bit packed as in Gen 4.

use alloc::vec::Vec;

use crate::{Error, Result, u16_at, u32_at};

/// Line break, control code start and end of a line, per generation.
pub struct Codes {
    pub newline: u16,
    pub control: u16,
}

pub const GEN4: Codes = Codes {
    newline: 0xE000,
    control: 0xFFFE,
};
pub const GEN5: Codes = Codes {
    newline: 0xFFFE,
    control: 0xF000,
};
const END: u16 = 0xFFFF;
const COMPRESSED: u16 = 0xF100;

/// Unpacks a 9-bit packed line (characters from the lowest bit up, ended by 0x1FF).
fn unpack(chars: &[u16]) -> Vec<u16> {
    let (mut bits, mut count, mut out) = (0u32, 0u32, Vec::new());
    for &w in chars {
        bits |= u32::from(w) << count;
        count += 16;
        while count >= 9 {
            let c = (bits & 0x1FF) as u16;
            if c == 0x1FF {
                return out;
            }
            out.push(c);
            bits >>= 9;
            count -= 9;
        }
    }
    out
}

/// The line as decoded characters, without the end marker.
fn finish(chars: Vec<u16>) -> Vec<u16> {
    let mut v = if chars.first() == Some(&COMPRESSED) {
        unpack(&chars[1..])
    } else {
        chars
    };
    if let Some(end) = v.iter().position(|&c| c == END) {
        v.truncate(end);
    }
    v
}

/// Every line of a Gen 4 message bank.
pub fn bank4(b: &[u8]) -> Result<Vec<Vec<u16>>> {
    let bad = Error::Format("message bank");
    let (count, seed) = (usize::from(u16_at(b, 0)?), u32::from(u16_at(b, 2)?));
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let k = seed.wrapping_mul(765).wrapping_mul(i as u32 + 1) & 0xFFFF;
        let k = k | k << 16;
        let off = (u32_at(b, 4 + 8 * i)? ^ k) as usize;
        let len = (u32_at(b, 8 + 8 * i)? ^ k) as usize;
        if off.checked_add(2 * len).is_none_or(|e| e > b.len()) {
            return Err(bad);
        }
        let mut key = ((i as u32 + 1).wrapping_mul(596_947)) as u16;
        let mut chars = Vec::with_capacity(len);
        for c in 0..len {
            chars.push(u16_at(b, off + 2 * c)? ^ key);
            key = key.wrapping_add(18_749);
        }
        out.push(finish(chars));
    }
    Ok(out)
}

/// Every line of a Gen 5 message bank (its first language section).
pub fn bank5(b: &[u8]) -> Result<Vec<Vec<u16>>> {
    let bad = Error::Format("message bank");
    let count = usize::from(u16_at(b, 2)?);
    let sec = u32_at(b, 12)? as usize;
    let mut out = Vec::with_capacity(count);
    for i in 0..count {
        let off = sec + u32_at(b, sec + 4 + 8 * i)? as usize;
        let len = usize::from(u16_at(b, sec + 8 + 8 * i)?);
        if off + 2 * len > b.len() {
            return Err(bad);
        }
        let mut chars = Vec::with_capacity(len);
        for c in 0..len {
            chars.push(u16_at(b, off + 2 * c)?);
        }
        if let Some(&last) = chars.last() {
            let mut key = last ^ END;
            for c in chars.iter_mut().rev() {
                *c ^= key;
                key = key.rotate_right(3);
            }
        }
        out.push(finish(chars));
    }
    Ok(out)
}
