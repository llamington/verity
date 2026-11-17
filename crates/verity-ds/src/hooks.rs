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

// ---- frames and heap -----------------------------------------------------------------------

/// Bytes the heap has handed out and not had back (C and Rust share it).
#[unsafe(no_mangle)]
static mut verity_test_heap_used: u32 = 0;
/// Bytes the heap has taken from RAM, its high-water mark (picolibc never gives them back).
#[unsafe(no_mangle)]
static mut verity_test_heap_arena: u32 = 0;
/// The longest frame's work, in scanlines since its VBlank (263 a frame); the harness zeroes it
/// after each step.
#[unsafe(no_mangle)]
static mut verity_test_frame_lines_max: u32 = 0;
/// Frames whose work ran past the next VBlank.
#[unsafe(no_mangle)]
static mut verity_test_frames_late: u32 = 0;
#[unsafe(no_mangle)]
static mut verity_test_vblanks: u32 = 0;

const LINES: u32 = 263;
const VBLANK_LINE: u32 = 192;
const REG_VCOUNT: *const u16 = 0x0400_0006 as *const u16;

#[repr(C)]
struct MallInfo {
    arena: usize,
    ordblks: usize,
    smblks: usize,
    hblks: usize,
    hblkhd: usize,
    usmblks: usize,
    fsmblks: usize,
    uordblks: usize,
    fordblks: usize,
    keepcost: usize,
}

unsafe extern "C" {
    /// picolibc's heap summary; walks the free list.
    safe fn mallinfo() -> MallInfo;
}

unsafe extern "C" fn on_vblank() {
    // SAFETY: a hook only this handler writes; the main thread only reads it.
    unsafe {
        let n = addr_of_mut!(verity_test_vblanks);
        n.write_volatile(n.read_volatile().wrapping_add(1));
    }
}

pub fn count_vblanks() {
    // SAFETY: installs a handler with libnds's dispatcher, which nothing else uses for VBlank.
    unsafe { blocksds::sys::irqSet(blocksds::sys::IRQ_VBLANK, Some(on_vblank)) };
}

/// The VBlank count and scanline now.
fn now() -> (u32, u32) {
    loop {
        // SAFETY: reads a hook and a read-only display register.
        let (v, line) = unsafe {
            (
                addr_of_mut!(verity_test_vblanks).read_volatile(),
                u32::from(REG_VCOUNT.read_volatile()),
            )
        };
        // SAFETY: as above.
        if unsafe { addr_of_mut!(verity_test_vblanks).read_volatile() } == v {
            return (v, line);
        }
    }
}

/// Marks the start of a frame's work, just after its VBlank.
pub fn frame_start() -> u32 {
    now().0
}

/// Records the frame's work time and the heap, before waiting for the next VBlank.
pub fn frame_end(start: u32) {
    let (v, line) = now();
    let since = (line + LINES - VBLANK_LINE) % LINES;
    let missed = v.wrapping_sub(start);
    let lines = missed * LINES + since;
    let heap = mallinfo();
    // SAFETY: hooks only this module writes (the harness zeroes the max between steps).
    unsafe {
        let max = addr_of_mut!(verity_test_frame_lines_max);
        if lines > max.read_volatile() {
            max.write_volatile(lines);
        }
        if missed > 0 {
            let late = addr_of_mut!(verity_test_frames_late);
            late.write_volatile(late.read_volatile() + 1);
        }
        addr_of_mut!(verity_test_heap_used).write_volatile(heap.uordblks as u32);
        addr_of_mut!(verity_test_heap_arena).write_volatile(heap.arena as u32);
    }
}
