//! The only code that writes VRAM, palettes and OAM after start-up. Everything else draws into the
//! RAM copies here and marks what it changed; `commit` copies the changes during VBlank, so a frame
//! is never shown half drawn (no tearing). See docs/research/ui-stack.md, Display.

use alloc::vec;
use alloc::vec::Vec;
use core::ptr::NonNull;

use blocksds::oam::{self, Oam};
use blocksds::{Engine, dma, sys, video};

use crate::cell::Global;

pub const SCREENS: usize = 2; // 0 top (main engine), 1 bottom (sub engine)
pub const ROWS: usize = 24;
pub const COLS: usize = 32;
pub const TILES: usize = ROWS * COLS;

/// Word aligned for word DMA.
#[repr(C, align(4))]
struct Aligned<const N: usize>([u16; N]);

struct Screen {
    text: Aligned<{ TILES * 16 }>,
    map: Aligned<{ 32 * 32 }>,
    bg_pal: Aligned<256>,
    sprite_pal: Aligned<256>,
    text_vram: *mut u16,
    map_vram: *mut u16,
    dirty_rows: u32, // bit r: text row r changed
    map_dirty: bool,
    bg_pal_dirty: bool,
    sprite_pal_dirty: bool,
}

impl Screen {
    const fn new() -> Self {
        Self {
            text: Aligned([0; TILES * 16]),
            map: Aligned([0; 32 * 32]),
            bg_pal: Aligned([0; 256]),
            sprite_pal: Aligned([0; 256]),
            text_vram: core::ptr::null_mut(),
            map_vram: core::ptr::null_mut(),
            dirty_rows: 0,
            map_dirty: false,
            bg_pal_dirty: false,
            sprite_pal_dirty: false,
        }
    }
}

/// Sprite graphics with a RAM copy: write `ram`, then mark it dirty. `vram` is what OAM takes.
pub struct SpriteGfx {
    pub vram: NonNull<u16>,
    pub ram: Vec<u16>,
    pub dirty: bool,
}

/// Handle to a sprite graphic from `sprite_gfx` (they are never freed).
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Gfx(usize);

struct State {
    screens: [Screen; SCREENS],
    gfx: Vec<SpriteGfx>,
}

static STATE: Global<State> = Global::new(State {
    screens: [Screen::new(), Screen::new()],
    gfx: Vec::new(),
});

/// Sets up a screen's text and frame layers: `text_tiles_vram` and `frame_map_vram` are the
/// VRAM the RAM copies stand in for.
pub fn init(s: usize, text_tiles_vram: *mut u16, frame_map_vram: *mut u16) {
    let mut st = STATE.borrow_mut();
    let sc = &mut st.screens[s];
    sc.text_vram = text_tiles_vram;
    sc.map_vram = frame_map_vram;
    sc.dirty_rows = (1 << ROWS) - 1;
    sc.map_dirty = true;
    sc.bg_pal_dirty = true;
    sc.sprite_pal_dirty = true;
}

/// A screen's text tiles (`TILES` 4 bpp tiles, 16 halfwords each) and its dirty rows (bit r:
/// text row r changed), for drawing.
pub fn with_text<R>(s: usize, f: impl FnOnce(&mut [u16], &mut u32) -> R) -> R {
    let mut st = STATE.borrow_mut();
    let sc = &mut st.screens[s];
    f(&mut sc.text.0, &mut sc.dirty_rows)
}

pub fn touch_text_rows(s: usize, row0: i32, row1: i32) {
    let (row0, row1) = (row0.max(0), row1.min(ROWS as i32));
    let mut st = STATE.borrow_mut();
    for r in row0..row1 {
        st.screens[s].dirty_rows |= 1 << r;
    }
}

/// A screen's frame map (32x32 entries); marks it changed.
pub fn with_frame_map<R>(s: usize, f: impl FnOnce(&mut [u16]) -> R) -> R {
    let mut st = STATE.borrow_mut();
    let sc = &mut st.screens[s];
    sc.map_dirty = true;
    f(&mut sc.map.0)
}

pub fn frame_map_at(s: usize, i: usize) -> u16 {
    STATE.borrow_mut().screens[s].map.0[i]
}

/// A screen's 256 background colours; marks them changed.
pub fn with_bg_palette<R>(s: usize, f: impl FnOnce(&mut [u16]) -> R) -> R {
    let mut st = STATE.borrow_mut();
    let sc = &mut st.screens[s];
    sc.bg_pal_dirty = true;
    f(&mut sc.bg_pal.0)
}

/// A screen's 256 sprite colours; marks them changed.
pub fn with_sprite_palette<R>(s: usize, f: impl FnOnce(&mut [u16]) -> R) -> R {
    let mut st = STATE.borrow_mut();
    let sc = &mut st.screens[s];
    sc.sprite_pal_dirty = true;
    f(&mut sc.sprite_pal.0)
}

/// Allocates VRAM and a RAM copy (never freed, like the VRAM); `None` when VRAM is full.
pub fn sprite_gfx(o: Oam, size: sys::SpriteSize, format: sys::SpriteColorFormat) -> Option<Gfx> {
    let vram = o.alloc_gfx(size, format)?;
    let ram = vec![0u16; oam::gfx_halfwords(size, format)];
    let mut st = STATE.borrow_mut();
    st.gfx.push(SpriteGfx {
        vram,
        ram,
        dirty: false,
    });
    Some(Gfx(st.gfx.len() - 1))
}

pub fn gfx_vram(g: Gfx) -> NonNull<u16> {
    STATE.borrow_mut().gfx[g.0].vram
}

/// Writes a graphic's RAM copy and marks it for the next commit.
pub fn with_gfx<R>(g: Gfx, f: impl FnOnce(&mut [u16]) -> R) -> R {
    let mut st = STATE.borrow_mut();
    let gfx = &mut st.gfx[g.0];
    gfx.dirty = true;
    f(&mut gfx.ram)
}

fn copy(vram: *mut u16, ram: &[u16]) {
    // SAFETY: every `vram` here is mapped VRAM or palette memory at least as long as `ram`
    // (set up by `init`, the palettes, or `oamAllocateGfx` for a graphic of this size), and
    // `ram` is word aligned in main RAM (the `Aligned` buffers and `Vec<u16>` of whole words).
    unsafe { dma::copy_words(3, ram, vram) }
}

/// Copies everything marked since the last commit, OAM first, then text rows top down (so if it
/// ever overran VBlank, the copy would stay ahead of the display). Call right after VBlank starts.
pub fn commit() {
    // OAM and palettes are small and decide what every line shows, so they go first.
    Oam::main().update();
    Oam::sub().update();
    let mut st = STATE.borrow_mut();
    for (s, sc) in st.screens.iter_mut().enumerate() {
        let engine = if s == 0 { Engine::Main } else { Engine::Sub };
        if sc.bg_pal_dirty {
            copy(video::bg_palette(engine).as_mut_ptr(), &sc.bg_pal.0);
        }
        if sc.sprite_pal_dirty {
            copy(video::sprite_palette(engine).as_mut_ptr(), &sc.sprite_pal.0);
        }
        if sc.map_dirty {
            copy(sc.map_vram, &sc.map.0);
        }
        sc.bg_pal_dirty = false;
        sc.sprite_pal_dirty = false;
        sc.map_dirty = false;
    }
    for g in st.gfx.iter_mut().filter(|g| g.dirty) {
        copy(g.vram.as_ptr(), &g.ram);
        g.dirty = false;
    }
    // Text rows top down on both screens together: the display reaches row r of each at once.
    let row_len = COLS * 16;
    for r in 0..ROWS {
        for sc in st.screens.iter() {
            if sc.dirty_rows & (1 << r) == 0 {
                continue;
            }
            // SAFETY: row `r` of the text layer's tiles, inside the VRAM `init` was given.
            let vram = unsafe { sc.text_vram.add(r * row_len) };
            copy(vram, &sc.text.0[r * row_len..(r + 1) * row_len]);
        }
    }
    for sc in st.screens.iter_mut() {
        sc.dirty_rows = 0;
    }
}

/// Waits for VBlank and commits: for showing a message before blocking work.
pub fn present() {
    blocksds::wait_for_vblank();
    commit();
}
