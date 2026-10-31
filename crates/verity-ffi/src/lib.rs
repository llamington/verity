#![no_std]

#[cfg(feature = "std")]
extern crate std;

#[cfg(any(feature = "alloc", feature = "std"))]
extern crate alloc;

use core::slice;

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

// Global allocator for ARM/picolibc target
#[cfg(all(feature = "picolibc-allocator", not(feature = "std")))]
mod allocator {
    use core::alloc::{GlobalAlloc, Layout};

    extern "C" {
        fn malloc(size: usize) -> *mut u8;
        fn free(ptr: *mut u8);
    }

    struct PicolibcAllocator;

    unsafe impl GlobalAlloc for PicolibcAllocator {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            malloc(layout.size())
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
    fn test_ffi_crc16_valid() {
        let input = b"123456789";
        unsafe {
            let res = verity_crc16_ccitt(input.as_ptr(), input.len());
            assert_eq!(res, verity::crc16_ccitt(input));
        }
    }
}
