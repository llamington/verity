#![no_std]

#[cfg(feature = "std")]
extern crate std;

#[cfg(any(feature = "alloc", feature = "std"))]
extern crate alloc;

use core::slice;

#[cfg(feature = "alloc")]
pub mod edit;
pub mod events;
pub mod ui;
pub mod unova;

/// The blob registry is global and single-threaded by contract; tests that touch it take this.
#[cfg(test)]
pub(crate) static TEST_LOCK: std::sync::Mutex<()> = std::sync::Mutex::new(());

/// C ABI: Calculates the CRC16-CCITT checksum over a buffer.
///
/// Returns 0 if `data` is null and `len` is not 0.
///
/// # Safety
///
/// If `data` is non-null, it must point to a readable memory region of at least `len` bytes.
#[no_mangle]
pub unsafe extern "C" fn verity_crc16_ccitt(data: *const u8, len: usize) -> u16 {
    if data.is_null() {
        return if len == 0 { 0xFFFF } else { 0 };
    }
    let slice = slice::from_raw_parts(data, len);
    verity::crc16_ccitt(slice)
}

/// FFI error codes (`enum verity_err`).
#[repr(C)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum VerityErr {
    Ok = 0,
    NullPointer = 1,
    Unrecognized = 2,
    BadArgument = 3,
    BadData = 4,
}

/// Save summary for the front end (`verity_save_info`).
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct VeritySaveInfo {
    /// 1 D/P, 2 Pt, 3 HG/SS, 4 B/W, 5 B2/W2.
    pub kind: u8,
    pub generation: u8,
    pub party_count: u8,
    pub checksums_valid: u8,
    pub tid: u16,
    pub sid: u16,
    /// OT name as UTF-16, 0-terminated.
    pub ot_name: [u16; 13],
    pub played_hours: u16,
    pub played_minutes: u8,
    pub played_seconds: u8,
    /// 20 W, 21 B, 22 W2, 23 B2; 0 on Gen 4, whose saves don't record it.
    pub game_version: u8,
}

/// C ABI: detects a save in `buf` (raw or DeSmuME `.dsv`) and summarises it. The buffer is not
/// modified.
///
/// # Safety
///
/// `buf` must point to `len` writable bytes and `out` to a writable `VeritySaveInfo`.
#[no_mangle]
pub unsafe extern "C" fn verity_save_probe(
    buf: *mut u8,
    len: usize,
    out: *mut VeritySaveInfo,
) -> VerityErr {
    if buf.is_null() || out.is_null() {
        return VerityErr::NullPointer;
    }
    let data = slice::from_raw_parts_mut(buf, len);
    match probe(data) {
        Some(info) => {
            *out = info;
            VerityErr::Ok
        }
        None => VerityErr::Unrecognized,
    }
}

fn probe(data: &mut [u8]) -> Option<VeritySaveInfo> {
    use verity::save::{Save, SaveKind};
    let save = Save::open(data).ok()?;
    let t = save.trainer();
    let mut ot_name = [0u16; 13];
    let n = if save.generation() == 4 {
        verity::text::decode4(&t.name_raw, &mut ot_name[..12])
    } else {
        verity::text::decode5(&t.name_raw, &mut ot_name[..12])
    };
    ot_name[n] = 0;
    Some(VeritySaveInfo {
        kind: match save.kind() {
            SaveKind::DP => 1,
            SaveKind::Pt => 2,
            SaveKind::HGSS => 3,
            SaveKind::BW => 4,
            SaveKind::B2W2 => 5,
        },
        generation: save.generation(),
        party_count: save.party_count() as u8,
        checksums_valid: save.checksums_valid() as u8,
        tid: t.tid,
        sid: t.sid,
        ot_name,
        played_hours: t.played_hours,
        played_minutes: t.played_minutes,
        played_seconds: t.played_seconds,
        game_version: save.game_version(),
    })
}

// Global allocator for ARM/picolibc target
#[cfg(all(feature = "picolibc-allocator", not(feature = "std")))]
mod allocator {
    use core::alloc::{GlobalAlloc, Layout};

    extern "C" {
        fn malloc(size: usize) -> *mut u8;
        fn memalign(align: usize, size: usize) -> *mut u8;
        fn free(ptr: *mut u8);
    }

    struct PicolibcAllocator;

    unsafe impl GlobalAlloc for PicolibcAllocator {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            if layout.align() <= 8 {
                malloc(layout.size())
            } else {
                memalign(layout.align(), layout.size())
            }
        }

        unsafe fn dealloc(&self, ptr: *mut u8, _layout: Layout) {
            free(ptr)
        }
    }

    #[global_allocator]
    static ALLOCATOR: PicolibcAllocator = PicolibcAllocator;
}

// Panic handler when building standalone for embedded target
#[cfg(all(not(feature = "std"), not(test)))]
#[panic_handler]
fn panic(_info: &core::panic::PanicInfo) -> ! {
    extern "C" {
        fn verity_on_panic() -> !;
    }
    unsafe {
        verity_on_panic();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ffi_crc16_null() {
        unsafe {
            assert_eq!(verity_crc16_ccitt(core::ptr::null(), 0), 0xFFFF);
            assert_eq!(verity_crc16_ccitt(core::ptr::null(), 10), 0);
        }
    }

    #[test]
    fn test_ffi_probe() {
        let path = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../corpus/real/pkhex-avalonia/gen4_platinum.sav"
        );
        let Ok(mut data) = std::fs::read(path) else {
            return;
        };
        let orig = data.clone();
        let mut info = VeritySaveInfo::default();
        let r = unsafe { verity_save_probe(data.as_mut_ptr(), data.len(), &mut info) };
        assert_eq!(r, VerityErr::Ok);
        assert_eq!((info.kind, info.generation), (2, 4));
        assert!(info.ot_name[0] != 0);
        assert_eq!(data, orig);
        let mut junk = [0u8; 16];
        let r = unsafe { verity_save_probe(junk.as_mut_ptr(), junk.len(), &mut info) };
        assert_eq!(r, VerityErr::Unrecognized);
    }

    #[test]
    fn test_ffi_crc16_valid() {
        let input = b"123456789";
        unsafe {
            let res = verity_crc16_ccitt(input.as_ptr(), input.len());
            assert_eq!(res, verity::crc16_ccitt(input));
        }
    }
}
