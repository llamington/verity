//! Read by the emulator harness by symbol (tools/emu-harness), so scripts wait for a scene instead
//! of counting frames, and fail a step that adds a layout fault.

use core::ptr::addr_of_mut;

/// The top scene's name.
#[unsafe(no_mangle)]
static mut verity_test_scene: [u8; 16] = [0; 16];
/// 1 while scenes change (fades, enter()).
#[unsafe(no_mangle)]
static mut verity_test_busy: u32 = 1;
/// Layout faults: text cut short where its caller didn't plan for it, pixels dropped outside its
/// region, or ink drawn over other text. The text holds the latest fault.
#[unsafe(no_mangle)]
static mut verity_test_layout: u32 = 0;
pub const LAYOUT_TEXT_LEN: usize = 96;
#[unsafe(no_mangle)]
static mut verity_test_layout_text: [u8; LAYOUT_TEXT_LEN] = [0; LAYOUT_TEXT_LEN];

/// Copies `text` (cut to fit, nul-terminated) into a hook with volatile writes, so the harness
/// sees each one.
fn publish(to: *mut u8, cap: usize, text: &[u8]) {
    let n = text.len().min(cap - 1);
    let bytes = text[..n].iter().copied().chain(core::iter::repeat(0));
    for (i, b) in bytes.take(cap).enumerate() {
        // SAFETY: `to` is one of the hook arrays above, `cap` long; only this module writes them.
        unsafe { to.add(i).write_volatile(b) };
    }
}

pub fn set_scene(name: &str) {
    publish(addr_of_mut!(verity_test_scene).cast(), 16, name.as_bytes());
}

pub fn set_busy(busy: bool) {
    // SAFETY: a hook only this module writes.
    unsafe { addr_of_mut!(verity_test_busy).write_volatile(busy.into()) };
}

pub fn layout_fault(text: &[u8]) {
    publish(
        addr_of_mut!(verity_test_layout_text).cast(),
        LAYOUT_TEXT_LEN,
        text,
    );
    // SAFETY: a hook only this module writes.
    unsafe {
        let n = addr_of_mut!(verity_test_layout);
        n.write_volatile(n.read_volatile() + 1);
    }
}

// ---- stack ---------------------------------------------------------------------------------
// The stack is the 15 KB of DTCM between the DTCM data (`__sbss_end`) and `__sp_usr`. Past its
// bottom is a mirror of main RAM: running into it is slow (uncached) and corrupts that RAM, so the
// unused stack is painted at boot and checked every frame.

unsafe extern "C" {
    static __sbss_end: u8;
}

const PAINT: u32 = 0x5AFE_5AFE;
/// Below this much free stack the front end stops with the panic screen.
const STACK_MIN_FREE: u32 = 512;

/// The least free stack seen, in bytes.
#[unsafe(no_mangle)]
static mut verity_test_stack_free: u32 = 0;

fn stack_bottom() -> *mut u32 {
    core::ptr::addr_of!(__sbss_end) as *mut u32
}

/// Paints the stack below the caller's frame.
#[inline(never)]
pub fn paint_stack() {
    let marker = 0u32;
    let top = (core::ptr::addr_of!(marker) as usize).saturating_sub(256) & !3;
    let mut p = stack_bottom();
    while (p as usize) < top {
        // SAFETY: words between the DTCM data and the live stack, which nothing uses yet.
        unsafe {
            p.write_volatile(PAINT);
            p = p.add(1);
        }
    }
}

/// Publishes the free stack (the painted words left at its bottom), and stops if it is nearly
/// gone.
pub fn check_stack() {
    let bottom = stack_bottom();
    let mut p = bottom;
    // SAFETY: reads DTCM from the stack's bottom up to the first word that isn't paint, which is
    // at the latest the live stack.
    while unsafe { p.read_volatile() } == PAINT {
        // SAFETY: as above.
        p = unsafe { p.add(1) };
    }
    let free = (p as usize - bottom as usize) as u32;
    // SAFETY: a hook only this module writes.
    unsafe {
        let hook = addr_of_mut!(verity_test_stack_free);
        if free < hook.read_volatile() || hook.read_volatile() == 0 {
            hook.write_volatile(free);
        }
    }
    assert!(free >= STACK_MIN_FREE, "stack nearly exhausted");
}
