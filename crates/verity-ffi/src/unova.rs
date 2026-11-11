//! Unova Link FFI for the DS front end (P10): Memory Link from a Black or White save, receiving
//! keys from another Black 2 or White 2 save, and enabling every key (`verity::memlink::link`).

use core::slice;

use verity::memlink::keys::KEY_COUNT;
use verity::memlink::link::{self, Plan, Status};

/// What linking would do (`verity_unova_preview`).
#[repr(C)]
#[derive(Clone, Copy, Default)]
pub struct VerityUnovaPreview {
    /// 0 unusable, 1 Memory Link, 2 keys, 3 enable all keys.
    pub mode: u8,
    /// 0 ready, 1 ready but replaces another player's link; otherwise see
    /// `verity_unova_status_text`.
    pub status: u8,
    /// Memory Link: the link unlocks a flashback this save doesn't have.
    pub flashback: u8,
    /// Keys: Easy, Challenge, City, Iron, Iceberg; 1 if this link enables it.
    pub keys: [u8; KEY_COUNT],
}

fn view(p: Plan) -> VerityUnovaPreview {
    VerityUnovaPreview {
        mode: p.mode as u8,
        status: p.status as u8,
        flashback: u8::from(p.flashback),
        keys: p.keys.map(u8::from),
    }
}

fn code(r: Result<(), Status>) -> u8 {
    r.map_or_else(|s| s as u8, |()| Status::Ready as u8)
}

/// Fills `out` with what linking `other` into the open B2W2 save would do. Writes only the save's
/// checksums (recomputed, as saving does).
///
/// # Safety
/// `buf` must be valid for `len` writable bytes, `other` for `other_len` bytes (not overlapping
/// `buf`), and `out` for one write.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn verity_unova_preview(
    buf: *mut u8,
    len: usize,
    other: *const u8,
    other_len: usize,
    out: *mut VerityUnovaPreview,
) {
    // SAFETY: `buf` is valid for `len` writable bytes and `other` for `other_len` bytes, and they
    // don't overlap (this function's contract).
    let (save, other) = unsafe {
        (
            slice::from_raw_parts_mut(buf, len),
            slice::from_raw_parts(other, other_len),
        )
    };
    // SAFETY: `out` is valid for one write (this function's contract).
    unsafe { *out = view(link::plan(save, other)) };
}

/// Links `other` into the open B2W2 save, replacing another player's Memory Link. Returns 0 on
/// success, else a status code. `seed` stands in for the game's frame counter. The save is not
/// finalised.
///
/// # Safety
/// `buf` must be valid for `len` writable bytes and `other` for `other_len` bytes, and the two
/// must not overlap.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn verity_unova_apply(
    buf: *mut u8,
    len: usize,
    other: *const u8,
    other_len: usize,
    seed: u32,
) -> u8 {
    // SAFETY: `buf` is valid for `len` writable bytes and `other` for `other_len` bytes, and they
    // don't overlap (this function's contract).
    let (save, other) = unsafe {
        (
            slice::from_raw_parts_mut(buf, len),
            slice::from_raw_parts(other, other_len),
        )
    };
    code(link::link(save, other, seed))
}

/// Fills `out` with what `verity_unova_enable_all` would do. Writes only the save's checksums.
///
/// # Safety
/// `buf` must be valid for `len` writable bytes, and `out` for one write.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn verity_unova_enable_preview(
    buf: *mut u8,
    len: usize,
    out: *mut VerityUnovaPreview,
) {
    // SAFETY: `buf` is valid for `len` writable bytes (this function's contract).
    let save = unsafe { slice::from_raw_parts_mut(buf, len) };
    // SAFETY: `out` is valid for one write (this function's contract).
    unsafe { *out = view(link::enable_plan(save)) };
}

/// Enables every key without a partner save. Returns 0 on success, else a status code. The save is
/// not finalised.
///
/// # Safety
/// `buf` must be valid for `len` writable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn verity_unova_enable_all(buf: *mut u8, len: usize) -> u8 {
    // SAFETY: `buf` is valid for `len` writable bytes (this function's contract).
    let save = unsafe { slice::from_raw_parts_mut(buf, len) };
    code(link::enable_all(save))
}

/// Text for a status from `verity_unova_preview` or `verity_unova_apply` (static,
/// NUL-terminated ASCII).
#[unsafe(no_mangle)]
pub extern "C" fn verity_unova_status_text(status: u8) -> *const u8 {
    Status::from_code(status).c_text().as_ptr()
}
