//! Mystery Gift FFI for the DS front end (P8): the events a save can take, and delivery.

use core::slice;

use verity::gift::inject4::{available, check, inject, Refusal};
use verity::gift::inject5;
use verity::gift::pcd::{Gift4, Gifts4, RANGER_INDEX};
use verity::gift::pgf::{Events5, Gifts5, Pgf};
use verity::save::Save;

use crate::ui::{blob, open, BlobKind};

/// One gift (`verity_event_view`).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct VerityEventView {
    pub blob: u8,
    pub gift_type: u8,
    pub record: u16,
    pub card_id: u16,
    pub species: u16,
    pub region: u8,
    pub status: u8,
    pub ranger: u8,
    pub reserved: u8,
    pub title: [u16; 37],
    /// Item cards: the item (0 otherwise).
    pub item: u16,
    /// Pokémon cards: the original trainer, which tells apart releases of one event (regional
    /// or shop distributions share a title). `ot` is UTF-16, 0-terminated; all 0 otherwise.
    pub tid: u16,
    pub sid: u16,
    pub ot: [u16; 9],
}

fn database(which: u8) -> Option<Gifts4<'static>> {
    let kind = match which {
        0 => BlobKind::Gifts4,
        1 => BlobKind::Events4,
        _ => return None,
    };
    Gifts4::load(blob(kind)?).ok()
}

fn status(result: Result<(), Refusal>) -> u8 {
    match result {
        Ok(()) => 0,
        Err(r) => r as u8,
    }
}

fn view(save: &Save, which: u8, record: usize, g: &Gift4) -> VerityEventView {
    let ranger = g.mgdb_index() == RANGER_INDEX;
    let mut v = VerityEventView {
        blob: which,
        gift_type: g.card_type() as u8,
        record: record as u16,
        card_id: if ranger { 0 } else { g.card_id() },
        species: if g.is_entity() { g.species() } else { 0 },
        region: g.region(),
        status: status(check(save, g)),
        ranger: ranger as u8,
        reserved: 0,
        title: [0; 37],
        item: 0,
        tid: 0,
        sid: 0,
        ot: [0; 9],
    };
    let raw = g.title_raw();
    let n = verity::text::decode4(&raw, &mut v.title[..36]);
    v.title[n] = 0;
    if g.is_entity() && !ranger {
        let pk = g.pk();
        v.tid = pk.tid();
        v.sid = pk.sid();
        let n = verity::text::decode4(&pk.ot_name_raw(), &mut v.ot[..8]);
        v.ot[n] = 0;
    } else if g.card_type() == 3 {
        // PKHeX `PGT.ItemID`.
        let p = g.pgt();
        v.item = u16::from_le_bytes([p[4], p[5]]);
    }
    v
}

/// `blob` value of a Gen 5 card in a [`VerityEventView`].
const WHICH_GEN5: u8 = 2;

fn database5() -> Option<(Gifts5<'static>, Events5<'static>)> {
    Some((
        Gifts5::load(blob(BlobKind::Gifts5)?).ok()?,
        Events5::load(blob(BlobKind::Events5)?).ok()?,
    ))
}

fn status5(result: Result<(), inject5::Refusal5>) -> u8 {
    match result {
        Ok(()) => 0,
        Err(r) => r as u8,
    }
}

fn view5(save: &Save, record: usize, g: &Pgf) -> VerityEventView {
    // The front end names card types by their Gen 4 numbers.
    let gift_type = match (g.card_type(), g.is_egg()) {
        (1, true) => 7,
        (1, false) => 1,
        (2, _) => 3,
        _ => 0,
    };
    let mut v = VerityEventView {
        blob: WHICH_GEN5,
        gift_type,
        record: record as u16,
        card_id: g.card_id(),
        species: if g.card_type() == 1 { g.species() } else { 0 },
        region: 0,
        status: status5(inject5::check(save, g)),
        ranger: 0,
        reserved: 0,
        title: [0; 37],
        item: 0,
        tid: 0,
        sid: 0,
        ot: [0; 9],
    };
    let raw = g.title_raw();
    let n = verity::text::decode5(&raw, &mut v.title[..36]);
    v.title[n] = 0;
    match g.card_type() {
        1 => {
            v.tid = g.tid();
            v.sid = g.sid();
            let n = verity::text::decode5(&g.ot_raw(), &mut v.ot[..8]);
            v.ot[n] = 0;
        }
        // PKHeX `PGF.ItemID` (the same bytes as the Pokémon's ID).
        2 => v.item = g.tid(),
        _ => {}
    }
    v
}

/// The civil date of `days` since 2000-01-01.
fn date_of(days: u16) -> (u16, u8, u8) {
    let (mut year, mut left) = (2000u16, days as u32);
    loop {
        let len =
            if year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400)) {
                366
            } else {
                365
            };
        if left < len {
            break;
        }
        left -= len;
        year += 1;
    }
    let leap = year.is_multiple_of(4) && (!year.is_multiple_of(100) || year.is_multiple_of(400));
    let lengths = [31, 28 + leap as u32, 31, 30, 31, 30, 31, 31, 30, 31, 30, 31];
    let mut month = 0;
    while left >= lengths[month] {
        left -= lengths[month];
        month += 1;
    }
    (year, month as u8 + 1, left as u8 + 1)
}

/// C ABI: fills `out` with the gifts for this save's game and language.
///
/// # Safety
///
/// `buf` must point to `len` writable bytes and `out` to `cap` writable views.
#[no_mangle]
pub unsafe extern "C" fn verity_events_list(
    buf: *mut u8,
    len: usize,
    out: *mut VerityEventView,
    cap: usize,
) -> usize {
    if out.is_null() {
        return 0;
    }
    let Some(save) = open(buf, len) else {
        return 0;
    };
    let out = slice::from_raw_parts_mut(out, cap);
    let (kind, language) = (save.kind(), save.trainer().language);
    let mut n = 0;
    if save.generation() == 5 {
        let Some((cards, events)) = database5() else {
            return 0;
        };
        let version = save.game_version();
        for (record, g) in cards.iter().enumerate() {
            let offered = events
                .get(record)
                .is_some_and(|e| inject5::available(e, kind, version, language));
            if n < out.len() && offered {
                out[n] = view5(&save, record, &g);
                n += 1;
            }
        }
        return n;
    }
    for which in 0..2 {
        let Some(db) = database(which) else {
            continue;
        };
        for (record, g) in db.iter().enumerate() {
            if n < out.len() && available(&g, kind, language) {
                out[n] = view(&save, which, record, &g);
                n += 1;
            }
        }
    }
    n
}

/// C ABI: delivers one gift listed by `verity_events_list`. The save is not finalised.
///
/// # Safety
///
/// `buf` must point to `len` writable bytes.
#[no_mangle]
pub unsafe extern "C" fn verity_event_inject(
    buf: *mut u8,
    len: usize,
    which: u8,
    record: u16,
    days: u16,
) -> u8 {
    const UNAVAILABLE: u8 = Refusal::Unavailable as u8;
    let Some(mut save) = open(buf, len) else {
        return UNAVAILABLE;
    };
    if which == WHICH_GEN5 {
        let Some((cards, events)) = database5() else {
            return UNAVAILABLE;
        };
        let (Some(g), Some(e)) = (cards.get(record as usize), events.get(record as usize)) else {
            return UNAVAILABLE;
        };
        if !inject5::available(e, save.kind(), save.game_version(), save.trainer().language) {
            return UNAVAILABLE;
        }
        return status5(inject5::inject(&mut save, &g, date_of(days)));
    }
    let Some(g) = database(which).and_then(|db| db.get(record as usize)) else {
        return UNAVAILABLE;
    };
    if !available(&g, save.kind(), save.trainer().language) {
        return UNAVAILABLE;
    }
    status(inject(&mut save, &g, days))
}

/// C ABI: human-readable text for a `verity_event_inject` result (static, NUL-terminated ASCII).
#[no_mangle]
pub extern "C" fn verity_event_status_text(code: u8) -> *const u8 {
    let s: &'static [u8] = match code {
        0 => b"Ready\0",
        1 => b"Already received\0",
        2 => b"Waiting at the deliveryman\0",
        3 => b"Deliveryman has 8 gifts waiting\0",
        4 => b"3 cards held: discard one in-game\0",
        6 => b"12 cards held: discard one in-game\0",
        _ => b"Not available for this save\0",
    };
    s.as_ptr()
}
