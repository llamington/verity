//! Bag FFI for the DS front end: pouches, their items, and adding, changing and removing items
//! (`verity::bag`). Results are 0 on success or a `verity::bag::Refusal` code; the save is not
//! finalised.

use core::slice;

use verity::bag::{self, Refusal};
use verity::data::GameData;
use verity::save::Save;

use crate::ui::{open, personal_for};

/// One pouch (`verity_pouch_view`).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct VerityPouchView {
    /// `verity::save::bag::PouchKind` as listed: 0 Items, 1 Key Items, 2 TMs & HMs, 3 Mail,
    /// 4 Medicine, 5 Berries, 6 Poke Balls, 7 Battle Items.
    pub kind: u8,
    pub reserved: u8,
    /// Items in it now.
    pub count: u16,
    pub slots: u16,
    /// The most of one item it holds.
    pub max_count: u16,
}

/// One item in a pouch (`verity_bag_item`).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct VerityBagItem {
    pub item: u16,
    pub count: u16,
    /// 1 for an event item (read-only; it comes from Mystery Gift).
    pub locked: u8,
    pub reserved: u8,
}

/// Runs a bag change with the save's game data.
///
/// # Safety
///
/// `buf` must be null or point to `len` writable bytes.
unsafe fn change(
    buf: *mut u8,
    len: usize,
    f: impl FnOnce(&mut Save, &GameData) -> Result<(), Refusal>,
) -> u8 {
    // SAFETY: `buf` is null or valid for `len` writable bytes, and the `Save` is dropped before
    // this call returns (this function's contract).
    let Some(mut save) = (unsafe { open(buf, len) }) else {
        return Refusal::NoSuchPouch.code();
    };
    let Some(game) = personal_for(save.kind()) else {
        return Refusal::NotInPouch.code();
    };
    match f(&mut save, &game) {
        Ok(()) => 0,
        Err(r) => r.code(),
    }
}

/// C ABI: fills `out` with the save's pouches; returns how many were written.
///
/// # Safety
///
/// `buf` must point to `len` writable bytes and `out` to `cap` writable views.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn verity_bag_pouches(
    buf: *mut u8,
    len: usize,
    out: *mut VerityPouchView,
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
    let defs = save.pouches();
    for (p, (slot, d)) in out.iter_mut().zip(defs).enumerate() {
        *slot = VerityPouchView {
            kind: d.kind as u8,
            reserved: 0,
            count: bag::entries(&save, p).map_or(0, |e| e.count() as u16),
            slots: d.slots,
            max_count: d.max_count,
        };
    }
    defs.len().min(cap)
}

/// C ABI: fills `out` with a pouch's items in slot order; returns how many were written. Their
/// positions are the `index` the edits take.
///
/// # Safety
///
/// `buf` must point to `len` writable bytes and `out` to `cap` writable items.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn verity_bag_items(
    buf: *mut u8,
    len: usize,
    pouch: u8,
    out: *mut VerityBagItem,
    cap: usize,
) -> usize {
    if out.is_null() {
        return 0;
    }
    // SAFETY: as in `verity_bag_pouches`.
    let Some(save) = (unsafe { open(buf, len) }) else {
        return 0;
    };
    // SAFETY: `out` is non-null (checked above) and valid for `cap` writable elements (this
    // function's contract).
    let out = unsafe { slice::from_raw_parts_mut(out, cap) };
    let Ok(entries) = bag::entries(&save, pouch.into()) else {
        return 0;
    };
    let mut n = 0;
    for (slot, e) in out.iter_mut().zip(entries) {
        *slot = VerityBagItem {
            item: e.item,
            count: e.count,
            locked: u8::from(e.locked),
            reserved: 0,
        };
        n += 1;
    }
    n
}

/// C ABI: fills `out` with the items that can be added to a pouch, in the game's order; returns
/// how many were written.
///
/// # Safety
///
/// `buf` must point to `len` writable bytes and `out` to `cap` writable `u16`s.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn verity_bag_addable(
    buf: *mut u8,
    len: usize,
    pouch: u8,
    out: *mut u16,
    cap: usize,
) -> usize {
    if out.is_null() {
        return 0;
    }
    // SAFETY: as in `verity_bag_pouches`.
    let Some(save) = (unsafe { open(buf, len) }) else {
        return 0;
    };
    // SAFETY: `out` is non-null (checked above) and valid for `cap` writable elements (this
    // function's contract).
    let out = unsafe { slice::from_raw_parts_mut(out, cap) };
    let Some(game) = personal_for(save.kind()) else {
        return 0;
    };
    let Ok(items) = bag::addable(&save, &game, pouch.into()) else {
        return 0;
    };
    let mut n = 0;
    for (slot, item) in out.iter_mut().zip(items) {
        *slot = item;
        n += 1;
    }
    n
}

/// C ABI: adds `count` of `item` to a pouch.
///
/// # Safety
///
/// `buf` must point to `len` writable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn verity_bag_add(
    buf: *mut u8,
    len: usize,
    pouch: u8,
    item: u16,
    count: u16,
) -> u8 {
    // SAFETY: `buf` is valid for `len` writable bytes (this function's contract).
    unsafe { change(buf, len, |s, g| bag::add(s, g, pouch.into(), item, count)) }
}

/// C ABI: sets the count of the `index`th item of a pouch.
///
/// # Safety
///
/// `buf` must point to `len` writable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn verity_bag_set_count(
    buf: *mut u8,
    len: usize,
    pouch: u8,
    index: u16,
    count: u16,
) -> u8 {
    // SAFETY: `buf` is valid for `len` writable bytes (this function's contract).
    unsafe {
        change(buf, len, |s, g| {
            bag::set_count(s, g, pouch.into(), index.into(), count)
        })
    }
}

/// C ABI: removes the `index`th item of a pouch.
///
/// # Safety
///
/// `buf` must point to `len` writable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn verity_bag_remove(buf: *mut u8, len: usize, pouch: u8, index: u16) -> u8 {
    // SAFETY: `buf` is valid for `len` writable bytes (this function's contract).
    unsafe {
        change(buf, len, |s, g| {
            bag::remove(s, g, pouch.into(), index.into())
        })
    }
}

/// C ABI: text for a `verity_bag_*` result (static, NUL-terminated ASCII; "" for 0).
#[unsafe(no_mangle)]
pub extern "C" fn verity_bag_status_text(code: u8) -> *const u8 {
    Refusal::from_code(code)
        .map_or("\0", Refusal::c_text)
        .as_ptr()
}

#[cfg(test)]
// Tests call the C ABI with buffers they own and keep alive for the call.
#[allow(clippy::undocumented_unsafe_blocks)]
mod tests {
    use super::*;
    use crate::ui::{BlobKind, verity_data_attach};
    extern crate std;
    use std::borrow::ToOwned;
    use std::boxed::Box;
    use std::vec::Vec;

    fn file(path: &str) -> Option<Vec<u8>> {
        std::fs::read(concat!(env!("CARGO_MANIFEST_DIR"), "/../../").to_owned() + path).ok()
    }

    #[test]
    fn lists_adds_changes_and_removes() {
        let (Some(mut save), Some(pt)) = (
            file("corpus/real/pkhex-avalonia/gen4_platinum.sav"),
            file("data/blobs/pt.bin"),
        ) else {
            return;
        };
        let pt: &'static [u8] = Box::leak(pt.into_boxed_slice());
        let (b, l) = (save.as_mut_ptr(), save.len());
        unsafe {
            verity_data_attach(BlobKind::Pt as u32, pt.as_ptr(), pt.len());
            let mut pouches = [VerityPouchView {
                kind: 0,
                reserved: 0,
                count: 0,
                slots: 0,
                max_count: 0,
            }; 8];
            assert_eq!(verity_bag_pouches(b, l, pouches.as_mut_ptr(), 8), 8);
            assert_eq!((pouches[1].kind, pouches[1].max_count), (1, 1));
            let before = pouches[0].count as usize;

            let mut add = [0u16; 300];
            let n = verity_bag_addable(b, l, 0, add.as_mut_ptr(), add.len());
            assert!(n > 0);
            assert_eq!(verity_bag_add(b, l, 0, add[0], 0), Refusal::Count.code());
            assert_eq!(verity_bag_add(b, l, 0, add[0], 3), 0);
            // The DS shows code 4 for an event item it was asked to edit (`Bag::kEventItem`).
            assert_eq!(verity_bag_add(b, l, 1, 454, 1), 4);

            let mut items = [VerityBagItem {
                item: 0,
                count: 0,
                locked: 0,
                reserved: 0,
            }; 300];
            let n = verity_bag_items(b, l, 0, items.as_mut_ptr(), items.len());
            assert_eq!(n, before + 1);
            assert_eq!((items[before].item, items[before].count), (add[0], 3));
            assert_eq!(verity_bag_set_count(b, l, 0, before as u16, 50), 0);
            assert_eq!(verity_bag_remove(b, l, 0, before as u16), 0);
            assert_eq!(
                verity_bag_items(b, l, 0, items.as_mut_ptr(), items.len()),
                before
            );
            assert_eq!(verity_bag_remove(b, l, 9, 0), Refusal::NoSuchPouch.code());

            let text = |c| core::ffi::CStr::from_ptr(verity_bag_status_text(c).cast());
            assert_eq!(text(0).to_bytes(), b"");
            assert_eq!(text(Refusal::Full.code()).to_bytes(), b"The pouch is full");
        }
    }
}
