//! The Mystery Gift screen's entry points (listing what a save can receive, delivering one),
//! shared by the DS (`verity-ffi`) and the browser (`verity-wasm`).

use alloc::vec::Vec;

use super::inject4::{self, Refusal};
use super::inject5::{self, Refusal5};
use super::pcd::{Gift4, Gifts4, RANGER_INDEX};
use super::pgf::{Events5, Gifts5, Pgf};
use crate::save::Save;

/// Which event database a card comes from (the DS's `blob` field).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u8)]
pub enum Source {
    /// `gifts4.bin`: Wonder Cards (PCD).
    Gifts4 = 0,
    /// `events4.bin`: the other Gen 4 deliveries (PGT).
    Events4 = 1,
    /// `gifts5.bin` with `events5.bin`: Gen 5 Wonder Cards (PGF).
    Gen5 = 2,
}

impl Source {
    pub fn from_u8(v: u8) -> Option<Self> {
        match v {
            0 => Some(Source::Gifts4),
            1 => Some(Source::Events4),
            2 => Some(Source::Gen5),
            _ => None,
        }
    }
}

/// The parsed event databases; each is optional (a missing one lists nothing).
#[derive(Default)]
pub struct Databases<'a> {
    pub gifts4: Option<Gifts4<'a>>,
    pub events4: Option<Gifts4<'a>>,
    pub gen5: Option<(Gifts5<'a>, Events5<'a>)>,
}

/// One card a save can be offered.
#[derive(Clone, Debug)]
pub struct Card {
    pub source: Source,
    /// Index within its database; with `source`, what [`deliver`] takes.
    pub record: u16,
    /// PKHeX `GiftType4` numbers, also for Gen 5 cards (see [`kind_name`]).
    pub gift_type: u8,
    /// 0 for the Ranger Manaphy egg, which has no card.
    pub card_id: u16,
    /// Pokémon and egg cards (0 otherwise).
    pub species: u16,
    /// Gen 4 region tag (see [`region_name`]); 0 when not regional.
    pub region: u8,
    pub ranger: bool,
    /// UTF-16.
    pub title: Vec<u16>,
    /// Item cards (0 otherwise).
    pub item: u16,
    /// Pokémon cards: the original trainer, which tells apart releases of one event (regional
    /// or shop distributions share a title). Empty otherwise.
    pub ot: Vec<u16>,
    pub tid: u16,
    pub sid: u16,
    /// 0 ready; otherwise why it can't be delivered now (see [`status_text`]).
    pub status: u8,
}

/// Status 0, and the refusal values of [`Refusal`] and [`Refusal5`], NUL-terminated for the C ABI.
const STATUS: [&str; 7] = [
    "Ready\0",
    "Already received\0",
    "Waiting at the deliveryman\0",
    "Deliveryman has 8 gifts waiting\0",
    "3 cards held: discard one in-game\0",
    "Not available for this save\0",
    "12 cards held: discard one in-game\0",
];

/// What a status means, NUL-terminated (unknown values read as not available).
pub fn status_c_text(status: u8) -> &'static str {
    STATUS
        .get(status as usize)
        .copied()
        .unwrap_or(STATUS[Refusal::Unavailable as usize])
}

/// What a status means.
pub fn status_text(status: u8) -> &'static str {
    let t = status_c_text(status);
    &t[..t.len() - 1]
}

/// The kind of gift a card holds, by [`Card::gift_type`].
pub fn kind_name(gift_type: u8) -> &'static str {
    match gift_type {
        1 => "Pokémon",
        3 => "Item",
        5 => "Decoration",
        7 => "Pokémon Egg",
        8 => "Member Card",
        9 => "Oak's Letter",
        11 => "Pokétch app",
        12 => "Secret Key",
        14 => "Pokéwalker course",
        _ => "Gift",
    }
}

/// The region a Gen 4 regional release was for, by [`Card::region`].
pub fn region_name(region: u8) -> Option<&'static str> {
    match region {
        1 => Some("North America"),
        2 => Some("United Kingdom"),
        3 => Some("Australia"),
        4 => Some("Europe"),
        _ => None,
    }
}

fn utf16(raw: &[u16], gen5: bool) -> Vec<u16> {
    let mut out = [0u16; 40];
    let n = if gen5 {
        crate::text::decode5(raw, &mut out)
    } else {
        crate::text::decode4(raw, &mut out)
    };
    out[..n].to_vec()
}

fn code<R: Into<u8>>(r: Result<(), R>) -> u8 {
    r.map_or_else(Into::into, |()| 0)
}

impl From<Refusal> for u8 {
    fn from(r: Refusal) -> u8 {
        r as u8
    }
}

impl From<Refusal5> for u8 {
    fn from(r: Refusal5) -> u8 {
        r as u8
    }
}

fn card4(save: &Save, source: Source, record: usize, g: &Gift4) -> Card {
    let ranger = g.mgdb_index() == RANGER_INDEX;
    let mut c = Card {
        source,
        record: record as u16,
        gift_type: g.card_type() as u8,
        card_id: if ranger { 0 } else { g.card_id() },
        species: if g.is_entity() { g.species() } else { 0 },
        region: g.region(),
        ranger,
        title: utf16(&g.title_raw(), false),
        item: 0,
        ot: Vec::new(),
        tid: 0,
        sid: 0,
        status: code(inject4::check(save, g)),
    };
    if g.is_entity() && !ranger {
        let pk = g.pk();
        (c.tid, c.sid) = (pk.tid(), pk.sid());
        c.ot = utf16(&pk.ot_name_raw(), false);
    } else if g.card_type() == 3 {
        // PKHeX `PGT.ItemID`.
        let p = g.pgt();
        c.item = u16::from_le_bytes([p[4], p[5]]);
    }
    c
}

fn card5(save: &Save, record: usize, g: &Pgf) -> Card {
    // Gen 5 card types, named by their Gen 4 numbers.
    let gift_type = match (g.card_type(), g.is_egg()) {
        (1, true) => 7,
        (1, false) => 1,
        (2, _) => 3,
        _ => 0,
    };
    let mut c = Card {
        source: Source::Gen5,
        record: record as u16,
        gift_type,
        card_id: g.card_id(),
        species: if g.card_type() == 1 { g.species() } else { 0 },
        region: 0,
        ranger: false,
        title: utf16(&g.title_raw(), true),
        item: 0,
        ot: Vec::new(),
        tid: 0,
        sid: 0,
        status: code(inject5::check(save, g)),
    };
    match g.card_type() {
        1 => {
            (c.tid, c.sid) = (g.tid(), g.sid());
            c.ot = utf16(&g.ot_raw(), true);
        }
        // PKHeX `PGF.ItemID` (the same bytes as the Pokémon's ID).
        2 => c.item = g.tid(),
        _ => {}
    }
    c
}

/// The cards offered to this save's game and language, in database order.
pub fn list(save: &Save, db: &Databases) -> Vec<Card> {
    let (kind, language) = (save.kind(), save.trainer().language);
    let mut out = Vec::new();
    if save.generation() == 5 {
        if let Some((cards, events)) = &db.gen5 {
            let version = save.game_version();
            for (record, g) in cards.iter().enumerate() {
                if events
                    .get(record)
                    .is_some_and(|e| inject5::available(e, kind, version, language))
                {
                    out.push(card5(save, record, &g));
                }
            }
        }
        return out;
    }
    for (source, gifts) in [(Source::Gifts4, &db.gifts4), (Source::Events4, &db.events4)] {
        let Some(gifts) = gifts else { continue };
        for (record, g) in gifts.iter().enumerate() {
            if inject4::available(&g, kind, language) {
                out.push(card4(save, source, record, &g));
            }
        }
    }
    out
}

/// Delivers one listed card, dated `days` since 2000-01-01. Returns the status (0 delivered).
/// The save is not finalised.
pub fn deliver(save: &mut Save, db: &Databases, source: Source, record: u16, days: u16) -> u8 {
    const UNAVAILABLE: u8 = Refusal::Unavailable as u8;
    let r = record as usize;
    if source == Source::Gen5 {
        let Some((cards, events)) = &db.gen5 else {
            return UNAVAILABLE;
        };
        let (Some(g), Some(e)) = (cards.get(r), events.get(r)) else {
            return UNAVAILABLE;
        };
        if !inject5::available(e, save.kind(), save.game_version(), save.trainer().language) {
            return UNAVAILABLE;
        }
        return code(inject5::inject(save, &g, date_of(days)));
    }
    let gifts = match source {
        Source::Gifts4 => &db.gifts4,
        _ => &db.events4,
    };
    let Some(g) = gifts.as_ref().and_then(|d| d.get(r)) else {
        return UNAVAILABLE;
    };
    if !inject4::available(&g, save.kind(), save.trainer().language) {
        return UNAVAILABLE;
    }
    code(inject4::inject(save, &g, days))
}

fn leap(year: u16) -> bool {
    year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400))
}

/// The civil date of `days` since 2000-01-01.
pub fn date_of(days: u16) -> (u16, u8, u8) {
    let (mut year, mut left) = (2000u16, u32::from(days));
    loop {
        let len = if leap(year) { 366 } else { 365 };
        if left < len {
            break;
        }
        left -= len;
        year += 1;
    }
    let lengths = [
        31,
        28 + u32::from(leap(year)),
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    let mut month = 0;
    while left >= lengths[month] {
        left -= lengths[month];
        month += 1;
    }
    (year, month as u8 + 1, left as u8 + 1)
}

/// Days since 2000-01-01 of a civil date (the inverse of [`date_of`]); `None` outside
/// 2000-01-01..=2099-12-31 or for a day the month doesn't have.
pub fn days_of(year: u16, month: u8, day: u8) -> Option<u16> {
    if !(2000..=2099).contains(&year) || !(1..=12).contains(&month) || day == 0 {
        return None;
    }
    let lengths = [
        31,
        28 + u8::from(leap(year)),
        31,
        30,
        31,
        30,
        31,
        31,
        30,
        31,
        30,
        31,
    ];
    if day > lengths[month as usize - 1] {
        return None;
    }
    let mut days: u32 = (2000..year).map(|y| if leap(y) { 366 } else { 365 }).sum();
    days += lengths[..month as usize - 1]
        .iter()
        .map(|&d| u32::from(d))
        .sum::<u32>();
    days += u32::from(day) - 1;
    u16::try_from(days).ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn dates_round_trip() {
        assert_eq!(date_of(0), (2000, 1, 1));
        assert_eq!(date_of(59), (2000, 2, 29)); // 2000 is a leap year
        assert_eq!(date_of(366), (2001, 1, 1));
        for days in [0, 59, 60, 365, 366, 9_811, 36_524] {
            let (y, m, d) = date_of(days);
            assert_eq!(days_of(y, m, d), Some(days), "{y}-{m}-{d}");
        }
        assert_eq!(days_of(2026, 11, 11), Some(9_811));
        assert_eq!(days_of(2026, 2, 29), None);
        assert_eq!(days_of(1999, 12, 31), None);
        assert_eq!(days_of(2100, 1, 1), None);
    }

    #[test]
    fn status_texts() {
        assert_eq!(status_text(0), "Ready");
        assert_eq!(
            status_text(Refusal::AlreadyReceived as u8),
            "Already received"
        );
        assert_eq!(
            status_text(Refusal5::AlbumFull as u8),
            "12 cards held: discard one in-game"
        );
        assert_eq!(status_text(200), "Not available for this save");
        assert!(status_c_text(3).ends_with('\0'));
    }
}
