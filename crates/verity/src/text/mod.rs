//! Gen 4/5 string encoding. Ported from PKHeX `PKM/Strings/StringConverter4.cs`,
//! `StringConverter4Util.cs`, `StringConverter5.cs` and `StringConverter.GetIsFullWidthString`.
//! Decoded text is UTF-16 code units.

mod gen4_table;

use gen4_table::{KOR_START, REVERSE, TABLE_INT, TABLE_KOR};

pub const TERMINATOR: u16 = 0xFFFF;
const SAVE_INVALID_AS: u16 = 0x1AC;
const APOSTROPHE: u16 = 0x1B3;
const RIGHT_SINGLE_QUOTE: u16 = 0x2019;

/// Half-width gender symbols as stored (Gen 5 values).
const HGM: u16 = 0x246D;
const HGF: u16 = 0x246E;
const FGM: u16 = 0x2642;
const FGF: u16 = 0x2640;

pub const LANGUAGE_KOREAN: u8 = 8;

fn normalize_gender(c: u16) -> u16 {
    match c {
        HGM => FGM,
        HGF => FGF,
        c => c,
    }
}

fn unnormalize_gender(c: u16) -> u16 {
    match c {
        FGM => HGM,
        FGF => HGF,
        c => c,
    }
}

fn is_full_width(s: &[u16]) -> bool {
    s.iter()
        .any(|&c| !matches!(c >> 12, 0 | 0xE) && c != FGF && c != FGM)
}

pub fn value_to_char_g4(v: u16) -> u16 {
    if (v as usize) < TABLE_INT.len() {
        TABLE_INT[v as usize]
    } else if v >= KOR_START && ((v - KOR_START) as usize) < TABLE_KOR.len() {
        TABLE_KOR[(v - KOR_START) as usize]
    } else {
        TERMINATOR
    }
}

pub fn char_to_value_g4(c: u16) -> u16 {
    match REVERSE.binary_search_by_key(&c, |&(u, _)| u) {
        Ok(i) => REVERSE[i].1,
        Err(_) if c == RIGHT_SINGLE_QUOTE => APOSTROPHE,
        Err(_) => SAVE_INVALID_AS,
    }
}

/// Decodes Gen 4 text into `out`, returning the number of code units written.
pub fn decode4(raw: &[u16], out: &mut [u16]) -> usize {
    let mut n = 0;
    for &v in raw {
        if v == TERMINATOR || n == out.len() {
            break;
        }
        out[n] = normalize_gender(value_to_char_g4(v));
        n += 1;
    }
    n
}

/// Decodes Gen 5 text (UTF-16, terminated by 0xFFFF or 0).
pub fn decode5(raw: &[u16], out: &mut [u16]) -> usize {
    let mut n = 0;
    for &v in raw {
        if v == TERMINATOR || v == 0 || n == out.len() {
            break;
        }
        out[n] = normalize_gender(v);
        n += 1;
    }
    n
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Clear {
    /// Leave bytes after the terminator untouched (PKHeX `StringConverterOption.None`).
    None,
    /// Zero the buffer first (PKHeX `ClearZero`).
    Zero,
}

/// Encodes into `dest`, terminating when room remains. Returns the number of code units
/// written including the terminator.
pub fn encode4(
    text: &[u16],
    max_len: usize,
    language: u8,
    dest: &mut [u16],
    clear: Clear,
) -> usize {
    encode(text, max_len, language, dest, clear, char_to_value_g4)
}

pub fn encode5(
    text: &[u16],
    max_len: usize,
    language: u8,
    dest: &mut [u16],
    clear: Clear,
) -> usize {
    encode(text, max_len, language, dest, clear, |c| c)
}

fn encode(
    text: &[u16],
    max_len: usize,
    language: u8,
    dest: &mut [u16],
    clear: Clear,
    map: impl Fn(u16) -> u16,
) -> usize {
    let text = &text[..text.len().min(max_len).min(dest.len())];
    if clear == Clear::Zero {
        dest.fill(0);
    }
    let half = language == LANGUAGE_KOREAN || !is_full_width(text);
    for (d, &c) in dest.iter_mut().zip(text) {
        *d = map(if half { unnormalize_gender(c) } else { c });
    }
    if text.len() == dest.len() {
        return text.len();
    }
    dest[text.len()] = TERMINATOR;
    text.len() + 1
}

/// Whether every code unit can be stored in Gen 4 text.
pub fn is_encodable_g4(text: &[u16]) -> bool {
    text.iter()
        .all(|&c| REVERSE.binary_search_by_key(&c, |&(u, _)| u).is_ok() || c == RIGHT_SINGLE_QUOTE)
}
