//! Gen 3 event gifts: PKHeX `EncounterGift3`, `EncounterGift3NY`, `EncounterGift3JPN`,
//! `EncounterGift3Colo`, `CommonEvent3Checker`, `MystryMew`, `ChannelJirachi` and
//! `PCJPFifthAnniversary`, for a Pokémon in the Gen 4 format.

use super::evo::EvoCriteria;
use crate::data::encounters::{Encounter, EncounterData, EncounterKind, Template, flag};
use crate::pkm::Pkm;
use crate::rng::finder::{PidIv, PidType};
use crate::rng::lcrng::{next_n, next16, prev_seed};
use crate::rng::xdrng;

const UNSPECIFIED_ID: u16 = 0xFFFF;
const BALL_POKE: u8 = 4;
const JAPANESE: u8 = 1;
const ENGLISH: u8 = 2;

// `GiftGender3`.
const RECIPIENT: u8 = 0;
const ONLY_0: u8 = 1;
const ONLY_1: u8 = 2;
const RAND_ALGO: u8 = 3;
const RAND_D3: u8 = 4;
const RAND_S3: u8 = 5;
const RAND_S7: u8 = 6;
const RAND_SG15: u8 = 7;
const RAND_D3_0: u8 = 8;
const RAND_D3_1: u8 = 9;

const MOVE_WISH: u16 = 273;
const JIRACHI: u16 = 385;
const PCJP_NAMES: [&str; 6] = [
    "トウキョー",
    "ヨコハマ",
    "ナゴヤ",
    "オーサカ",
    "フクオカ",
    "サッポロ",
];
const PCJP_TIDS: [u16; 6] = [51126, 51224, 60114, 60227, 60321, 60505];

/// Whether the Pokémon's language lets this event's trainer table apply (`YieldState.EventPCNY`).
pub fn pcny_language(language: u8) -> bool {
    language == ENGLISH
}

/// `YieldState.EventPCJP`.
pub fn pcjp_language(language: u8) -> bool {
    language == JAPANESE
}

fn ot_text(pk: &Pkm, out: &mut [u16; 8]) -> usize {
    if pk.format() == crate::pkm::Format::Pk5 {
        crate::text::decode5(&pk.ot_name_raw(), out)
    } else {
        crate::text::decode4(&pk.ot_name_raw(), out)
    }
}

fn eq_str(a: &[u16], s: &str) -> bool {
    a.iter().copied().eq(s.encode_utf16())
}

/// `EncounterGift3.IsMatchExact`, format 4.
fn gift3_exact(
    ed: &EncounterData,
    e: &Encounter,
    t: &Template,
    pk: &Pkm,
    evo: &EvoCriteria,
) -> bool {
    if t.version_mask() & (1u8 << (pk.version() - 1)) == 0 {
        return false;
    }
    let egg = t.has(flag::EGG);
    if pk.is_egg() && !egg {
        return false;
    }
    if !(egg && !pk.is_egg()) {
        if (t.sid() != UNSPECIFIED_ID && t.sid() != pk.sid())
            || (t.tid() != UNSPECIFIED_ID && t.tid() != pk.tid())
            || !gender_specific(t.ot_gender(), pk.ot_gender())
        {
            return false;
        }
        if let Some((_, name)) = ed.trade_names(e, 0)
            && !name.is_empty()
        {
            let mut buf = [0u16; 8];
            let n = ot_text(pk, &mut buf);
            let same = name.len() == n && name.iter().zip(buf.iter()).all(|(a, b)| a == *b);
            if !same {
                return false;
            }
        }
    }
    if !form_ok(t, evo) {
        return false;
    }
    let lang = t.gift_language();
    if lang != 0 {
        if lang != pk.language() {
            return false;
        }
    } else if !egg && pk.language() == JAPANESE {
        return false;
    }
    pk.ball() == BALL_POKE
        && (t.has(flag::FATEFUL) == pk.fateful() || egg)
        && !pk.is_egg()
        && t.level_min() <= pk.met_level()
        && pk.egg_location() == 0
}

/// Events have form 0; Deoxys can change form later (`FormInfo.IsFormChangeable`).
fn form_ok(t: &Template, evo: &EvoCriteria) -> bool {
    evo.form == 0 || t.species() == 386
}

fn gender_specific(t: u8, value: u8) -> bool {
    match t {
        ONLY_0 | RAND_D3_0 => value == 0,
        ONLY_1 | RAND_D3_1 => value == 1,
        _ => true,
    }
}

fn ny_exact(t: &Template, pk: &Pkm, evo: &EvoCriteria) -> bool {
    let mut buf = [0u16; 8];
    let n = ot_text(pk, &mut buf);
    matches!(pk.version(), 1 | 2)
        && !pk.is_egg()
        && pk.sid() == 0
        && (1..3000).contains(&pk.tid())
        && n > 4
        && ny_name_ok(t.distribution(), &buf[..n])
        && form_ok(t, evo)
        && pk.language() != JAPANESE
        && pk.ball() == BALL_POKE
        && t.level_min() <= pk.met_level()
        && pk.egg_location() == 0
}

/// `Distribution3NY.IsValidTrainerName`.
fn ny_name_ok(d: u8, name: &[u16]) -> bool {
    let (b, c, d_) = (
        eq_str(name, "PCNYb"),
        eq_str(name, "PCNYc"),
        eq_str(name, "PCNYd"),
    );
    match d {
        0 | 2 | 3 | 4 => b || c,
        1 => b || c || d_,
        5..=7 => c || d_,
        _ => d_,
    }
}

fn jpn_exact(t: &Template, pk: &Pkm, evo: &EvoCriteria) -> bool {
    let dist = t.distribution() as usize;
    let mut buf = [0u16; 8];
    let n = ot_text(pk, &mut buf);
    let name_ok = n > 2 && {
        let last = if dist == 5 { 5 } else { 6 };
        PCJP_NAMES[..last].iter().any(|s| eq_str(&buf[..n], s))
    };
    pk.version() == 2
        && !pk.is_egg()
        && pk.sid() == 0
        && dist < PCJP_TIDS.len()
        && pk.tid() == PCJP_TIDS[dist]
        && name_ok
        && form_ok(t, evo)
        && pk.language() == JAPANESE
        && pk.ball() == BALL_POKE
        && t.level_min() <= pk.met_level()
        && pk.egg_location() == 0
}

fn colo_exact(t: &Template, pk: &Pkm, evo: &EvoCriteria) -> bool {
    pk.version() == t.version()
        && pk.egg_location() == 0
        && evo.level_max >= t.level_min()
        && form_ok(t, evo)
}

/// `IsMatchExact` for any of the event kinds.
pub fn is_match_exact(ed: &EncounterData, e: &Encounter, pk: &Pkm, evo: &EvoCriteria) -> bool {
    let Some(t) = e.template() else {
        return false;
    };
    match t.kind() {
        Some(EncounterKind::Gift3) => gift3_exact(ed, e, &t, pk, evo),
        Some(EncounterKind::Gift3NY) => ny_exact(&t, pk, evo),
        Some(EncounterKind::Gift3JPN) => jpn_exact(&t, pk, evo),
        Some(EncounterKind::Gift3Colo) => colo_exact(&t, pk, evo),
        _ => false,
    }
}

/// `EncounterGift3Colo.IsMatchPartial` (bonus-disc gifts).
pub fn colo_partial(t: &Template, pk: &Pkm) -> bool {
    pk.ball() != BALL_POKE || (t.has(flag::JAPANESE_BONUS_DISK) && pk.language() != JAPANESE)
}

fn gender_bit0(r: u32) -> u32 {
    (r / 3) & 1
}

fn gender_bit3(r: u32) -> u32 {
    (r >> 3) & 1
}

fn gender_bit7(r: u32) -> u32 {
    ((r >> 7) & 1) ^ 1
}

fn gender_bit15(r: u32) -> u32 {
    (r >> 15) & 1
}

/// `EncounterGift3.GetGender`.
fn expected_gender(t: u8, seed: u32) -> u32 {
    match t {
        ONLY_0 | RAND_D3_0 => 0,
        ONLY_1 | RAND_D3_1 => 1,
        RAND_D3 => gender_bit0(next_n::<1>(seed) >> 16),
        RAND_S3 => gender_bit3(next_n::<1>(seed) >> 16),
        RAND_S7 => gender_bit7(next_n::<1>(seed) >> 16),
        RAND_SG15 => gender_bit15(next_n::<2>(seed) >> 16),
        _ => 0,
    }
}

fn gift3_gender_ok(ot_gender: u8, pk_gender: u8, seed: u32) -> bool {
    let expect = match ot_gender {
        RAND_D3_0 | RAND_D3_1 => gender_bit0(next_n::<6>(seed) >> 16),
        g => expected_gender(g, next_n::<4>(seed)),
    };
    pk_gender as u32 == expect
}

fn is_mystry_mew(seed: u32) -> bool {
    const SEEDS: [u16; 86] = [
        0x0652, 0x0932, 0x0C13, 0x0D43, 0x0EEE, 0x1263, 0x13C9, 0x1614, 0x1C09, 0x1EA5, 0x20BF,
        0x2389, 0x2939, 0x302D, 0x306E, 0x34F3, 0x45F3, 0x46CE, 0x4A0D, 0x4B63, 0x4C79, 0x508E,
        0x50AB, 0x5240, 0x5327, 0x56BA, 0x56CC, 0x5841, 0x5A60, 0x5BC1, 0x5E2B, 0x5EF3, 0x6065,
        0x643F, 0x6457, 0x67A3, 0x6944, 0x6E06, 0x6E62, 0x7667, 0x77EF, 0x78D2, 0x8655, 0x8A92,
        0x8B48, 0x93D0, 0x941D, 0x95A0, 0x967D, 0x9690, 0x9C37, 0x9C40, 0x9D9C, 0x9DE4, 0x9E86,
        0xA153, 0xA443, 0xA8AC, 0xAC08, 0xAFFB, 0xB1F2, 0xB831, 0xBE96, 0xC2D4, 0xC385, 0xC6CE,
        0xC92C, 0xC953, 0xC962, 0xCC43, 0xCD47, 0xCD96, 0xD1E4, 0xDFED, 0xE62C, 0xE6CC, 0xE90A,
        0xE95D, 0xE991, 0xEBB2, 0xEE7F, 0xEE9F, 0xEFC8, 0xF0E4, 0xFE4E, 0xFE9D,
    ];
    const RELEASED: u32 = 0x6065;
    const RELEASED_KEPT_INDEX: usize = 2;
    let mut s = seed;
    let (index, sub) = {
        let mut found = None;
        if s <= 0xFFFF {
            found = Some((SEEDS.binary_search(&(s as u16)), 0));
        } else {
            for i in 1..5usize {
                for _ in 0..5 {
                    s = prev_seed(s);
                }
                if s <= 0xFFFF {
                    found = Some((SEEDS.binary_search(&(s as u16)), i));
                    break;
                }
            }
        }
        match found {
            Some((Ok(i), sub)) => (i, sub),
            _ => return false,
        }
    };
    let effective = if sub == 0 { seed } else { SEEDS[index] as u32 };
    !(effective == RELEASED && sub != RELEASED_KEPT_INDEX)
}

fn pcjp_table_ok(species: u16, wish: bool, shiny: bool, seed16: u32) -> bool {
    let mut s = seed16;
    let a = next16(&mut s) as u32;
    let b = next16(&mut s) as u32;
    let r = a << 16 | b;
    let high = r >> 16;
    let first = ((high << 2) & 0xFFFF) + high;
    let mut second = ((r & 0xFFFF) << 1) + (first >> 16);
    second += high + (second >> 16);
    let result = (1000 * (second & 0xFFFF)) >> 16;
    let eighth = result / 125;
    let index = eighth >> 1;
    let got_wish = eighth & 1 == 1;
    let got_shiny = index == 0 && result % 125 >= 100;
    (((species as u32 >> 2) + 1) & 3) == index && got_wish == wish && got_shiny == shiny
}

fn is_table(origin_seed: u32, species: u16, wish: bool, shiny: bool) -> bool {
    let origin = prev_seed(prev_seed(origin_seed));
    if origin > 0xFFFF {
        return false;
    }
    species == JIRACHI || pcjp_table_ok(species, wish, shiny, origin)
}

/// `ChannelJirachi.GetPossible`: any of the three float patterns.
fn channel_possible(seed: u32) -> bool {
    (0..3).any(|i| {
        let mut origin = seed;
        channel_accept(&mut origin, i) && channel_menu(&mut origin)
    })
}

fn xd_prev16(seed: &mut u32) -> u32 {
    *seed = xdrng::prev(*seed);
    *seed >> 16
}

fn channel_accept(seed: &mut u32, pivot: u32) -> bool {
    match pivot {
        0 => {
            *seed = xdrng::prev(*seed);
            if xd_prev16(seed) <= 0x547A || xd_prev16(seed) <= 0x4000 {
                return false;
            }
        }
        1 => {
            if xd_prev16(seed) > 0x4000 {
                return false;
            }
        }
        _ => {
            if xd_prev16(seed) > 0x547A || xd_prev16(seed) <= 0x4000 {
                return false;
            }
        }
    }
    *seed = xdrng::prev_n(*seed, 5);
    true
}

fn channel_menu(seed: &mut u32) -> bool {
    let end = *seed >> 30;
    if end == 0 {
        return false;
    }
    let mut seen = 1u32 << end;
    loop {
        *seed = xdrng::prev(*seed);
        let p = *seed >> 30;
        if p == end {
            return false;
        }
        seen |= 1u32 << p;
        if seen >= 0b1110 {
            return true;
        }
    }
}

/// `IsCompatibleReviseReset` / `IsCompatible` for an event, updating the revised type.
pub fn compatible(e: &Encounter, pk: &Pkm, pv: PidIv, mutated: &mut Option<PidType>) -> bool {
    use PidType::*;
    let Some(t) = e.template() else {
        return false;
    };
    let kind = mutated.unwrap_or(pv.kind);
    let seed = pv.seed;
    match t.kind() {
        Some(EncounterKind::Gift3Colo) => matches!(pv.kind, Cxd | CxdAnti),
        Some(EncounterKind::Gift3NY) => kind == BacdAx,
        Some(EncounterKind::Gift3JPN) => {
            let expect = gender_bit7(next_n::<5>(seed) >> 16);
            matches!(kind, Bacd | BacdR) && pk.ot_gender() as u32 == expect
        }
        Some(EncounterKind::Gift3) => {
            let egg = t.has(flag::EGG);
            let method = t.gift_method();
            if matches!(kind, BacdEa | BacdEs) && !egg {
                return false;
            }
            let g = t.ot_gender();
            if !matches!(g, RAND_ALGO | RECIPIENT)
                && (!egg || pk.is_egg())
                && !gift3_gender_ok(g, pk.ot_gender(), seed)
            {
                return false;
            }
            let wish = t.moves().contains(&MOVE_WISH);
            let species = t.species();
            let mut mutate = |to| *mutated = Some(to);
            match method {
                m if m == BacdU as u8 => kind == Bacd,
                m if m == BacdR as u8 => {
                    let ok = kind == Bacd && seed <= 0xFFFF;
                    if ok {
                        mutate(BacdR);
                    }
                    ok
                }
                m if m == BacdRA as u8 => {
                    let ok = matches!(kind, Bacd | BacdA | BacdEa) && seed <= 0xFFFF;
                    if ok {
                        mutate(BacdR);
                    }
                    ok
                }
                m if m == BacdUAx as u8 => {
                    let ok = kind == BacdAx;
                    if ok {
                        mutate(BacdAx);
                    }
                    ok
                }
                m if m == BacdTa as u8 => {
                    let ok = matches!(kind, Bacd | BacdA | BacdEa)
                        && is_table(seed, species, wish, false);
                    if ok {
                        mutate(BacdR);
                    }
                    ok
                }
                m if m == BacdTs as u8 => {
                    let ok = matches!(kind, BacdS | BacdEs) && is_table(seed, species, wish, true);
                    if ok {
                        mutate(BacdR);
                    }
                    ok
                }
                m if m == BacdRbcd as u8 => {
                    let ok = kind == BacdS && (3..=213).contains(&seed);
                    if ok {
                        mutate(BacdR);
                    }
                    ok
                }
                m if m == BacdM as u8 => {
                    let ok = kind == Bacd && is_mystry_mew(seed);
                    if ok {
                        mutate(BacdR);
                    }
                    ok
                }
                m if m == Channel as u8 => {
                    let ok = kind == Channel && channel_possible(seed);
                    if ok {
                        mutate(Channel);
                    }
                    ok
                }
                m if m == Method2 as u8 => matches!(kind, Method1 | Method2 | Method4),
                _ => false,
            }
        }
        _ => false,
    }
}
