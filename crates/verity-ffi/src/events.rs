//! Mystery Gift FFI for the DS front end: the gift list and delivery
//! (`verity::gift::catalog`).

use core::slice;

use verity::gift::catalog::{self, Card, Databases, Source};
use verity::gift::inject4::Refusal;
use verity::gift::pcd::Gifts4;
use verity::gift::pgf::{Events5, Gifts5};

use crate::ui::{BlobKind, blob, open};

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

fn databases() -> Databases<'static> {
    let g4 = |k| blob(k).and_then(|b| Gifts4::load(b).ok());
    Databases {
        gifts4: g4(BlobKind::Gifts4),
        events4: g4(BlobKind::Events4),
        gen5: blob(BlobKind::Gifts5)
            .and_then(|b| Gifts5::load(b).ok())
            .zip(blob(BlobKind::Events5).and_then(|b| Events5::load(b).ok())),
    }
}

/// Copies UTF-16 into a 0-terminated array, cut to fit.
fn copy_text(src: &[u16], out: &mut [u16]) {
    let n = src.len().min(out.len() - 1);
    out[..n].copy_from_slice(&src[..n]);
    out[n] = 0;
}

fn view(c: &Card) -> VerityEventView {
    let mut v = VerityEventView {
        blob: c.source as u8,
        gift_type: c.gift_type,
        record: c.record,
        card_id: c.card_id,
        species: c.species,
        region: c.region,
        status: c.status,
        ranger: u8::from(c.ranger),
        reserved: 0,
        title: [0; 37],
        item: c.item,
        tid: c.tid,
        sid: c.sid,
        ot: [0; 9],
    };
    copy_text(&c.title, &mut v.title);
    copy_text(&c.ot, &mut v.ot);
    v
}

/// C ABI: fills `out` with the gifts for this save's game and language.
///
/// # Safety
///
/// `buf` must point to `len` writable bytes and `out` to `cap` writable views.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn verity_events_list(
    buf: *mut u8,
    len: usize,
    out: *mut VerityEventView,
    cap: usize,
) -> usize {
    if out.is_null() {
        return 0;
    }
    // SAFETY: `buf` is null or valid for `len` writable bytes, and the `Save` is dropped before
    // this call returns (this function's contract).
    let Some(save) = (unsafe { open(buf, len) }) else {
        return 0;
    };
    // SAFETY: `out` is non-null (checked above) and valid for `cap` writable elements (this
    // function's contract).
    let out = unsafe { slice::from_raw_parts_mut(out, cap) };
    let cards = catalog::list(&save, &databases());
    for (slot, card) in out.iter_mut().zip(&cards) {
        *slot = view(card);
    }
    cards.len().min(cap)
}

/// C ABI: delivers one gift listed by `verity_events_list`. The save is not finalised.
///
/// # Safety
///
/// `buf` must point to `len` writable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn verity_event_inject(
    buf: *mut u8,
    len: usize,
    which: u8,
    record: u16,
    days: u16,
) -> u8 {
    const UNAVAILABLE: u8 = Refusal::Unavailable as u8;
    // SAFETY: `buf` is null or valid for `len` writable bytes, and the `Save` is dropped before
    // this call returns (this function's contract).
    let (Some(mut save), Some(source)) = ((unsafe { open(buf, len) }), Source::from_u8(which))
    else {
        return UNAVAILABLE;
    };
    catalog::deliver(&mut save, &databases(), source, record, days)
}

/// C ABI: human-readable text for a `verity_event_inject` result (static, NUL-terminated ASCII).
#[unsafe(no_mangle)]
pub extern "C" fn verity_event_status_text(code: u8) -> *const u8 {
    catalog::status_c_text(code).as_ptr()
}
