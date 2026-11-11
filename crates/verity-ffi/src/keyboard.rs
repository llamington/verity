//! Nickname keyboard input: plain keys, the Japanese voicing marks (゛ ゜ change the last kana, as
//! on the games' keyboards) and Hangul composition from compatibility jamo (2-set layout: a
//! consonant then a vowel make a syllable, a following consonant becomes its final, and a vowel
//! after a final moves it to a new syllable). Every result must be storable in the save's
//! generation; otherwise the key does nothing (or, for jamo, is added on its own).

use verity::text::is_encodable_g4;

pub const DAKUTEN: u16 = 0x309B;
pub const HANDAKUTEN: u16 = 0x309C;

/// Initial consonants in syllable order, as compatibility jamo.
const CHO: [u16; 19] = [
    0x3131, 0x3132, 0x3134, 0x3137, 0x3138, 0x3139, 0x3141, 0x3142, 0x3143, 0x3145, 0x3146, 0x3147,
    0x3148, 0x3149, 0x314A, 0x314B, 0x314C, 0x314D, 0x314E,
];
/// Finals in syllable order (index 0 = none), as compatibility jamo.
const JONG: [u16; 28] = [
    0, 0x3131, 0x3132, 0x3133, 0x3134, 0x3135, 0x3136, 0x3137, 0x3139, 0x313A, 0x313B, 0x313C,
    0x313D, 0x313E, 0x313F, 0x3140, 0x3141, 0x3142, 0x3144, 0x3145, 0x3146, 0x3147, 0x3148, 0x314A,
    0x314B, 0x314C, 0x314D, 0x314E,
];
/// (first final, consonant) -> compound final.
const JONG_PAIRS: [(u16, u16, u16); 11] = [
    (0x3131, 0x3145, 0x3133),
    (0x3134, 0x3148, 0x3135),
    (0x3134, 0x314E, 0x3136),
    (0x3139, 0x3131, 0x313A),
    (0x3139, 0x3141, 0x313B),
    (0x3139, 0x3142, 0x313C),
    (0x3139, 0x3145, 0x313D),
    (0x3139, 0x314C, 0x313E),
    (0x3139, 0x314D, 0x313F),
    (0x3139, 0x314E, 0x3140),
    (0x3142, 0x3145, 0x3144),
];
/// (vowel, vowel) -> compound vowel, as jungseong indexes (ㅏ = 0 … ㅣ = 20).
const JUNG_PAIRS: [(u8, u8, u8); 7] = [
    (8, 0, 9),
    (8, 1, 10),
    (8, 20, 11),
    (13, 4, 14),
    (13, 5, 15),
    (13, 20, 16),
    (18, 20, 19),
];

const S_BASE: u16 = 0xAC00;
const VOWEL_FIRST: u16 = 0x314F;
const VOWEL_LAST: u16 = 0x3163;

fn storable(generation: u8, c: u16) -> bool {
    // ゔ has no glyph in the games' fonts.
    c != 0x3094 && (generation != 4 || is_encodable_g4(&[c]))
}

fn cho_index(c: u16) -> Option<u16> {
    CHO.iter().position(|&j| j == c).map(|i| i as u16)
}

fn jong_index(c: u16) -> Option<u16> {
    JONG.iter()
        .skip(1)
        .position(|&j| j == c)
        .map(|i| i as u16 + 1)
}

fn vowel_index(c: u16) -> Option<u16> {
    (VOWEL_FIRST..=VOWEL_LAST)
        .contains(&c)
        .then(|| c - VOWEL_FIRST)
}

/// (initial, vowel, final) of a precomposed syllable.
fn split(c: u16) -> Option<(u16, u16, u16)> {
    let s = c.checked_sub(S_BASE).filter(|&s| s < 11172)?;
    Some((s / (21 * 28), (s / 28) % 21, s % 28))
}

fn syllable(cho: u16, jung: u16, jong: u16) -> u16 {
    S_BASE + (cho * 21 + jung) * 28 + jong
}

/// The kana after applying a voicing mark, if `c` takes it.
fn voiced(c: u16, mark: u16) -> Option<u16> {
    let (base, kata) = if (0x30A1..=0x30F6).contains(&c) {
        (c - 0x60, true)
    } else {
        (c, false)
    };
    // か..ち (every other code from か), つ, て, と: the voiced form is the next code.
    let simple = |b: u16| {
        ((0x304B..=0x3061).contains(&b) && (b - 0x304B).is_multiple_of(2))
            || matches!(b, 0x3064 | 0x3066 | 0x3068)
    };
    let out = if let Some(b) = [base, base.wrapping_sub(1)]
        .into_iter()
        .find(|&b| simple(b))
    {
        if mark != DAKUTEN {
            return None;
        }
        if base == b { b + 1 } else { b }
    } else if let Some(b) = [0x306F, 0x3072, 0x3075, 0x3078, 0x307B]
        .into_iter()
        .find(|&b| (b..b + 3).contains(&base))
    {
        let target = if mark == DAKUTEN { b + 1 } else { b + 2 };
        if base == target { b } else { target }
    } else if base == 0x3046 && mark == DAKUTEN {
        0x3094 // ゔ (only ヴ survives `storable`)
    } else if base == 0x3094 && mark == DAKUTEN {
        0x3046
    } else {
        return None;
    };
    Some(if kata { out + 0x60 } else { out })
}

/// Types `key` into `text[..*len]` (at most `max` characters). Returns false if nothing changed.
pub fn type_key(text: &mut [u16], len: &mut usize, max: usize, key: u16, generation: u8) -> bool {
    let max = max.min(text.len());
    let last = (*len > 0).then(|| text[*len - 1]);
    let set_last = |text: &mut [u16], c: u16| {
        if storable(generation, c) {
            text[*len - 1] = c;
            true
        } else {
            false
        }
    };
    if key == DAKUTEN || key == HANDAKUTEN {
        return match last.and_then(|l| voiced(l, key)) {
            Some(c) => set_last(text, c),
            None => false,
        };
    }
    if let (Some(l), true) = (last, (0x3131..=VOWEL_LAST).contains(&key)) {
        if let Some(v) = vowel_index(key) {
            if let Some(cho) = cho_index(l) {
                if set_last(text, syllable(cho, v, 0)) {
                    return true;
                }
            } else if let Some((cho, jung, jong)) = split(l) {
                if jong == 0 {
                    let pair = JUNG_PAIRS
                        .iter()
                        .find(|p| u16::from(p.0) == jung && u16::from(p.1) == v);
                    if let Some(p) = pair
                        && set_last(text, syllable(cho, u16::from(p.2), 0))
                    {
                        return true;
                    }
                } else if *len < max {
                    // The final moves to a new syllable (a compound final splits).
                    let f = JONG[usize::from(jong)];
                    let (keep, moved) = JONG_PAIRS
                        .iter()
                        .find(|p| p.2 == f)
                        .map_or((0, f), |p| (jong_index(p.0).unwrap_or(0), p.1));
                    if let Some(next_cho) = cho_index(moved) {
                        let (a, b) = (syllable(cho, jung, keep), syllable(next_cho, v, 0));
                        if storable(generation, a) && storable(generation, b) {
                            text[*len - 1] = a;
                            text[*len] = b;
                            *len += 1;
                            return true;
                        }
                    }
                }
            }
        } else if let Some((cho, jung, jong)) = split(l) {
            let next = if jong == 0 {
                jong_index(key)
            } else {
                let f = JONG[usize::from(jong)];
                JONG_PAIRS
                    .iter()
                    .find(|p| p.0 == f && p.1 == key)
                    .and_then(|p| jong_index(p.2))
            };
            if let Some(j) = next
                && set_last(text, syllable(cho, jung, j))
            {
                return true;
            }
        }
    }
    if *len < max && storable(generation, key) {
        text[*len] = key;
        *len += 1;
        return true;
    }
    false
}

/// C ABI: types `key` into the nickname `text` of `*len` characters (capacity `cap`, at most
/// `max` characters). Returns 1 if the text changed.
///
/// # Safety
///
/// `text` must point to `cap` writable `u16`s and `len` to a writable `size_t` that is <= `cap`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn verity_keyboard_type(
    text: *mut u16,
    len: *mut usize,
    cap: usize,
    max: usize,
    key: u16,
    generation: u8,
) -> u8 {
    // SAFETY: `len` is non-null (checked above) and points to a valid value (this function's
    // contract).
    if text.is_null() || len.is_null() || unsafe { *len } > cap {
        return 0;
    }
    // SAFETY: `text` is non-null (checked above) and valid for `cap` writable elements (this
    // function's contract).
    let text = unsafe { core::slice::from_raw_parts_mut(text, cap) };
    // SAFETY: `len` is non-null (checked above) and points to a valid value (this function's
    // contract).
    type_key(text, unsafe { &mut *len }, max, key, generation) as u8
}

#[cfg(test)]
// Tests call the C ABI with buffers they own and keep alive for the call.
#[allow(clippy::undocumented_unsafe_blocks)]
mod tests {
    use super::*;

    fn typed(keys: &str, generation: u8) -> std::string::String {
        let mut t = [0u16; 10];
        let mut n = 0;
        for k in keys.chars() {
            type_key(&mut t, &mut n, 5, k as u16, generation);
        }
        std::string::String::from_utf16(&t[..n]).unwrap()
    }

    #[test]
    fn hangul_composition() {
        assert_eq!(typed("ㅎㅏㄴㄱㅡㄹ", 5), "한글");
        assert_eq!(typed("ㅍㅣㅋㅏㅊㅠ", 5), "피카츄");
        assert_eq!(typed("ㄷㅏㄹㄱ", 5), "닭"); // compound final
        assert_eq!(typed("ㄷㅏㄹㄱㅣ", 5), "달기"); // split on a vowel
        assert_eq!(typed("ㄱㅗㅏ", 5), "과"); // compound vowel
        assert_eq!(typed("ㅏㄱ", 5), "ㅏㄱ"); // no initial: jamo stay apart
        assert_eq!(typed("ㄲㄲ", 5), "ㄲㄲ");
    }

    #[test]
    fn hangul_respects_gen4_set() {
        // 똠 is outside the Gen 4 Korean table: its final stays a separate jamo there.
        assert_eq!(typed("ㄸㅗㅁ", 5), "똠");
        assert_eq!(typed("ㄸㅗㅁ", 4), "또ㅁ");
    }

    #[test]
    fn voicing_marks() {
        assert_eq!(typed("か゛", 5), "が");
        assert_eq!(typed("か゛゛", 5), "か");
        assert_eq!(typed("は゜", 5), "ぱ");
        assert_eq!(typed("は゛゜", 5), "ぱ");
        assert_eq!(typed("つ゛", 5), "づ");
        assert_eq!(typed("ト゛", 5), "ド");
        assert_eq!(typed("ウ゛", 5), "ヴ");
        assert_eq!(typed("ウ゛", 4), "ウ"); // ヴ can't be stored in Gen 4
        assert_eq!(typed("う゛", 5), "う");
        assert_eq!(typed("あ゛", 5), "あ");
        assert_eq!(typed("゛", 5), "");
        assert_eq!(typed("か゜", 5), "か");
    }

    #[test]
    fn limits() {
        assert_eq!(typed("ABCDEFG", 5), "ABCDE");
        // A split needs room for the new syllable.
        assert_eq!(typed("ㄱㅏㄱㅏㄱㅏㄱㅏㄱㅏㄱㅏ", 5), "가가가가각");
    }
}
