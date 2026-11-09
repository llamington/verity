//! Mystery Gift FFI for the DS front end (P8): the events a save can take, and delivery.

use core::slice;

use verity::gift::inject4::{available, check, inject, Refusal};
use verity::gift::pcd::{Gift4, Gifts4, RANGER_INDEX};
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
    };
    let raw = g.title_raw();
    let n = verity::text::decode4(&raw, &mut v.title[..36]);
    v.title[n] = 0;
    v
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
        _ => b"Not available for this save\0",
    };
    s.as_ptr()
}
