//! Unova Link FFI for the DS front end (P10): Memory Link from a Black or White save, and receiving
//! keys from another Black 2 or White 2 save.

use core::slice;

use verity::memlink::keys::{enable_all, missing, read, receivable, receive, KEY_COUNT};
use verity::memlink::{apply, build, preview, MemLinkError, Seeds};
use verity::save::{detect, SaveKind};

const RAW_SAVE_LEN: usize = 0x80000;

const MODE_NONE: u8 = 0;
const MODE_MEMORY_LINK: u8 = 1;
const MODE_KEYS: u8 = 2;
const MODE_ENABLE_ALL: u8 = 3;

const STATUS_OK: u8 = 0;
const STATUS_OTHER_PLAYER: u8 = 1;
const STATUS_NO_NEW_KEYS: u8 = 2;
const STATUS_WRONG_FILE: u8 = 3;
const STATUS_GIFTS_CORRUPT: u8 = 4;
const STATUS_KEYS_CORRUPT: u8 = 5;

/// What linking would do (`verity_unova_preview`).
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VerityUnovaPreview {
    /// 0 unusable, 1 Memory Link, 2 keys, 3 enable all keys.
    pub mode: u8,
    /// 0 ready; otherwise see `verity_unova_status_text`.
    pub status: u8,
    /// Memory Link: the link unlocks a flashback this save doesn't have.
    pub flashback: u8,
    /// Keys: Easy, Challenge, City, Iron, Iceberg; 1 if this link enables it.
    pub keys: [u8; KEY_COUNT],
}

fn status_of(error: MemLinkError) -> u8 {
    match error {
        MemLinkError::DifferentPlayer => STATUS_OTHER_PLAYER,
        MemLinkError::GiftsCorrupt => STATUS_GIFTS_CORRUPT,
        MemLinkError::KeysCorrupt => STATUS_KEYS_CORRUPT,
        MemLinkError::NotBlackWhite | MemLinkError::NotBlack2White2 => STATUS_WRONG_FILE,
    }
}

fn mode_of(other: &[u8]) -> u8 {
    if other.len() < RAW_SAVE_LEN {
        return MODE_NONE;
    }
    match detect(&other[..RAW_SAVE_LEN]) {
        Some(SaveKind::BW) => MODE_MEMORY_LINK,
        Some(SaveKind::B2W2) => MODE_KEYS,
        _ => MODE_NONE,
    }
}

fn plan(save: &[u8], other: &[u8], replace: bool) -> VerityUnovaPreview {
    let mut out = VerityUnovaPreview {
        mode: mode_of(other),
        ..Default::default()
    };
    match out.mode {
        MODE_MEMORY_LINK => match build(other).and_then(|data| preview(save, &data, replace)) {
            Ok(applied) => out.flashback = applied.unlocked_flashback as u8,
            Err(e) => out.status = status_of(e),
        },
        MODE_KEYS => match read(save).and_then(|s| Ok((s, read(other)?))) {
            Ok((target, partner)) => {
                let new = receivable(&target, &partner);
                out.keys = new.map(u8::from);
                if !new.contains(&true) {
                    out.status = STATUS_NO_NEW_KEYS;
                }
            }
            Err(e) => out.status = status_of(e),
        },
        _ => out.status = STATUS_WRONG_FILE,
    }
    out
}

/// Fills `out` with what linking `other` into the open B2W2 save would do. Writes nothing.
///
/// # Safety
/// `buf` and `other` must be valid for their lengths, and `out` for one write.
#[no_mangle]
pub unsafe extern "C" fn verity_unova_preview(
    buf: *const u8,
    len: usize,
    other: *const u8,
    other_len: usize,
    replace: bool,
    out: *mut VerityUnovaPreview,
) {
    let (save, other) = unsafe {
        (
            slice::from_raw_parts(buf, len),
            slice::from_raw_parts(other, other_len),
        )
    };
    unsafe { *out = plan(save, other, replace) };
}

/// Links `other` into the open B2W2 save. Returns 0 on success, else a status code. `seed` stands in
/// for the game's frame counter. The save is not finalised.
///
/// # Safety
/// `buf` and `other` must be valid for their lengths.
#[no_mangle]
pub unsafe extern "C" fn verity_unova_apply(
    buf: *mut u8,
    len: usize,
    other: *const u8,
    other_len: usize,
    replace: bool,
    seed: u32,
) -> u8 {
    let (save, other) = unsafe {
        (
            slice::from_raw_parts_mut(buf, len),
            slice::from_raw_parts(other, other_len),
        )
    };
    let result = match mode_of(other) {
        MODE_MEMORY_LINK => build(other).and_then(|data| {
            let seeds = Seeds {
                gifts: (seed >> 24) as u8,
                key_info: seed,
            };
            apply(save, &data, seeds, replace).map(|_| ())
        }),
        MODE_KEYS => match receive(save, other) {
            Ok(new) if !new.contains(&true) => return STATUS_NO_NEW_KEYS,
            r => r.map(|_| ()),
        },
        _ => return STATUS_WRONG_FILE,
    };
    result.map_or_else(status_of, |()| STATUS_OK)
}

/// Fills `out` with what `verity_unova_enable_all` would do. Writes nothing to the save.
///
/// # Safety
/// `buf` must be valid for `len` bytes, and `out` for one write.
#[no_mangle]
pub unsafe extern "C" fn verity_unova_enable_preview(
    buf: *const u8,
    len: usize,
    out: *mut VerityUnovaPreview,
) {
    let save = unsafe { slice::from_raw_parts(buf, len) };
    let mut plan = VerityUnovaPreview {
        mode: MODE_ENABLE_ALL,
        ..Default::default()
    };
    match read(save) {
        Ok(keys) => {
            let new = missing(&keys);
            plan.keys = new.map(u8::from);
            if !new.contains(&true) {
                plan.status = STATUS_NO_NEW_KEYS;
            }
        }
        Err(e) => plan.status = status_of(e),
    }
    unsafe { *out = plan };
}

/// Enables every key without a partner save. Returns 0 on success, else a status code. The save is
/// not finalised.
///
/// # Safety
/// `buf` must be valid for `len` bytes.
#[no_mangle]
pub unsafe extern "C" fn verity_unova_enable_all(buf: *mut u8, len: usize) -> u8 {
    let save = unsafe { slice::from_raw_parts_mut(buf, len) };
    match enable_all(save) {
        Ok(new) if !new.contains(&true) => STATUS_NO_NEW_KEYS,
        Ok(_) => STATUS_OK,
        Err(e) => status_of(e),
    }
}

/// Text for a status from `verity_unova_preview` or `verity_unova_apply`.
#[no_mangle]
pub extern "C" fn verity_unova_status_text(status: u8) -> *const u8 {
    let text: &[u8] = match status {
        STATUS_OK => b"Ready\0",
        STATUS_OTHER_PLAYER => b"Replaces another player's link\0",
        STATUS_NO_NEW_KEYS => b"No new keys to enable\0",
        STATUS_GIFTS_CORRUPT => b"This save's gift data is corrupt\0",
        STATUS_KEYS_CORRUPT => b"This save's link data is corrupt\0",
        _ => b"Needs a valid B/W or B2/W2 save\0",
    };
    text.as_ptr()
}
