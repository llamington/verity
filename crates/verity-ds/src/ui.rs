//! Front-end primitives: input, scene stack with fades, text, frames and sprites.

use alloc::boxed::Box;
use alloc::vec::Vec;
use core::ptr::NonNull;

use blocksds::input::{self, Keys};
use blocksds::oam::{Oam, Sprite};
use blocksds::video::{self, Screens, rgb15};
use blocksds::{Engine, bg, sys};

use crate::cell::{Global, Var};
use crate::display::{self, Gfx};
use crate::ffi;
use crate::hooks;
use crate::t;
use crate::text::Text;

// ---- input ---------------------------------------------------------------------------------

#[derive(Default)]
pub struct Input {
    pub down: u32,
    pub held: u32,
    pub rep: u32,
    pub touching: bool,
    pub tx: i32,
    pub ty: i32,
    /// A completed tap (pen up without dragging), in bottom-screen pixels.
    pub tapped: bool,
    pub tap_x: i32,
    pub tap_y: i32,
    /// Vertical drag since pen down, in pixels (valid while touching).
    pub dragging: bool,
    pub drag_dy: i32,
    start_y: i32,
    last_x: i32,
    last_y: i32,
}

pub const KEY_A: u32 = Keys::A.0;
pub const KEY_B: u32 = Keys::B.0;
pub const KEY_X: u32 = Keys::X.0;
pub const KEY_Y: u32 = Keys::Y.0;
pub const KEY_L: u32 = Keys::L.0;
pub const KEY_R: u32 = Keys::R.0;
pub const KEY_UP: u32 = Keys::UP.0;
pub const KEY_DOWN: u32 = Keys::DOWN.0;
pub const KEY_LEFT: u32 = Keys::LEFT.0;
pub const KEY_RIGHT: u32 = Keys::RIGHT.0;
pub const KEY_START: u32 = Keys::START.0;
pub const KEY_SELECT: u32 = Keys::SELECT.0;
pub const KEY_TOUCH: u32 = Keys::TOUCH.0;

impl Input {
    pub fn new() -> Self {
        Self {
            start_y: -1,
            ..Self::default()
        }
    }

    pub fn poll(&mut self) {
        input::scan();
        self.down = input::down().0;
        self.held = input::held().0;
        self.rep = input::down_repeat().0;
        self.tapped = false;
        self.touching = self.held & KEY_TOUCH != 0;
        if self.touching {
            let t = input::touch().unwrap_or_default();
            self.tx = t.x.into();
            self.ty = t.y.into();
            if self.down & KEY_TOUCH != 0 {
                self.start_y = self.ty;
                self.dragging = false;
            }
            if self.start_y >= 0 && (self.ty - self.start_y).abs() > 6 {
                self.dragging = true;
            }
            self.drag_dy = if self.start_y >= 0 {
                self.ty - self.start_y
            } else {
                0
            };
            self.last_x = self.tx;
            self.last_y = self.ty;
        } else if self.start_y >= 0 {
            if !self.dragging {
                self.tapped = true;
                self.tap_x = self.last_x;
                self.tap_y = self.last_y;
            }
            self.start_y = -1;
            self.dragging = false;
            self.drag_dy = 0;
        }
    }
}

// ---- scenes --------------------------------------------------------------------------------

pub trait Scene {
    /// Called when the scene becomes the top of the stack (first time or after a pop).
    fn enter(&mut self);
    /// One frame. Returns false to pop this scene.
    fn update(&mut self, input: &Input) -> bool;
    /// A short name for tests (the emulator harness reads it as verity_test_scene).
    fn name(&self) -> &'static str;
}

static PENDING: Global<Option<Box<dyn Scene>>> = Global::new(None);

fn fade(out: bool) {
    for i in (0..=16).step_by(2) {
        video::set_brightness(Screens::Both, if out { -i } else { -(16 - i) });
        blocksds::wait_for_vblank();
    }
}

/// Shows a scene's first frame. The screens go black first (the fade in starts there), so the copy
/// can't be seen mid-frame and needs no VBlank wait: the change takes no extra frame.
fn show_entered() {
    video::set_brightness(Screens::Both, -16);
    display::commit();
}

/// The new scene is entered behind a fade.
pub fn push(s: Box<dyn Scene>) {
    *PENDING.borrow_mut() = Some(s);
}

fn enter_top(stack: &mut [Box<dyn Scene>]) {
    sprites_hide_all();
    frame_clear(top());
    frame_clear(bottom());
    if let Some(s) = stack.last_mut() {
        s.enter();
    }
    show_entered();
    fade(false);
    // The fades don't scan keys, so the last scan may still hold the key that started the
    // change; scan now, or a quick press right after the change would be no new press.
    input::scan();
    hooks::set_scene(stack.last().map_or("", |s| s.name()));
    hooks::set_busy(false);
}

/// Runs the stack until it is empty.
pub fn run() {
    let mut stack: Vec<Box<dyn Scene>> = Vec::new();
    let mut input = Input::new();
    loop {
        let pending = PENDING.borrow_mut().take();
        if let Some(s) = pending {
            hooks::set_busy(true);
            if !stack.is_empty() {
                fade(true);
            }
            stack.push(s);
            enter_top(&mut stack);
        }
        let Some(scene) = stack.last_mut() else { break };
        // Show the last frame's drawing while the display is between frames, then draw the next.
        blocksds::wait_for_vblank();
        let start = hooks::frame_start();
        display::commit();
        input.poll();
        let keep = scene.update(&input);
        hooks::check_stack();
        if keep && PENDING.borrow_mut().is_none() {
            hooks::frame_end(start);
        }
        if !keep {
            hooks::set_busy(true);
            fade(true);
            stack.pop();
            if stack.is_empty() {
                break;
            }
            enter_top(&mut stack);
        }
    }
}

pub fn present() {
    display::present();
}

// ---- text ----------------------------------------------------------------------------------
// Proportional 8-pixel-high text. Each screen is a 32x24 grid of its own tiles, so text is drawn
// as pixels. Glyphs come from libnds' console font, trimmed to their inked columns.

const TILES: usize = 32 * 24;
const SPACE_WIDTH: i32 = 4;

struct Font {
    first: i32,
    count: i32,
    digit_width: i32, // digits share one width so numbers line up
    width: [u8; 128],
    /// Glyph rows shifted to their first inked column.
    rows: [[u8; 8]; 128],
}

static FONT: Global<Font> = Global::new(Font {
    first: 0,
    count: 0,
    digit_width: 6,
    width: [0; 128],
    rows: [[0; 8]; 128],
});

fn measure_font() {
    // SAFETY: libnds's default console is a static it owns; its font is `numChars` 8x8 1 bpp
    // glyphs of 8 bytes each.
    let (gfx, first, count) = unsafe {
        let d = &*sys::consoleGetDefault();
        let count = usize::from(d.font.numChars);
        let gfx = core::slice::from_raw_parts(d.font.gfx.cast::<u8>(), count * 8);
        (gfx, i32::from(d.font.asciiOffset), count as i32)
    };
    let mut f = FONT.borrow_mut();
    f.first = first;
    f.count = count;
    let mut left = [0u8; 128];
    for c in 0..128 {
        let ix = c as i32 - first;
        f.width[c] = 0;
        if ix < 0 || ix >= count {
            continue;
        }
        let mask = (0..8).fold(0u8, |m, y| m | gfx[ix as usize * 8 + y]);
        let (mut lo, mut hi) = (8, -1);
        for x in 0..8 {
            if mask & (1 << x) != 0 {
                lo = lo.min(x);
                hi = x;
            }
        }
        if hi >= 0 {
            left[c] = lo as u8;
            f.width[c] = (hi - lo + 1) as u8;
        }
    }
    for c in b'0'..=b'9' {
        let w = i32::from(f.width[usize::from(c)]) + 2;
        if w > f.digit_width {
            f.digit_width = w;
        }
    }
    for c in 0..128 {
        let ix = c as i32 - first;
        for y in 0..8 {
            f.rows[c][y] = if ix < 0 || ix >= count {
                0
            } else {
                ((u32::from(gfx[ix as usize * 8 + y]) >> left[c]) & ((1u32 << f.width[c]) - 1))
                    as u8
            };
        }
    }
}

static INK: Var<u8> = Var::new(1); // palette index plot() writes (see the theme below)

/// Where plot() may draw, in text-layer pixels (the layer is scrolled 2 px right, so its last 2
/// columns would wrap to the left edge). While a text draw tracks, plot() notes pixels it had to
/// drop and ink drawn over ink.
#[derive(Clone, Copy)]
struct Clip {
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
}
const SCREEN: Clip = Clip {
    x0: 0,
    y0: 0,
    x1: 254,
    y1: 192,
};
static CLIP: Var<Clip> = Var::new(SCREEN);
static TRACK: Var<bool> = Var::new(false);
static CLIPPED: Var<bool> = Var::new(false);
static OVERLAP: Var<bool> = Var::new(false);

fn plot(c: Surface, x: i32, y: i32) {
    let clip = CLIP.get();
    if x < clip.x0 || x >= clip.x1 || y < clip.y0 || y >= clip.y1 {
        CLIPPED.set(true);
        return;
    }
    let ink = u16::from(INK.get());
    display::with_text(c.0, |tiles, dirty| {
        let tile = (y / 8 * 32 + x / 8) as usize;
        *dirty |= 1 << (y / 8);
        let word = &mut tiles[tile * 16 + (y % 8) as usize * 2 + ((x % 8) / 4) as usize];
        let shift = ((x % 8) % 4) * 4;
        if TRACK.get() && (*word >> shift) & 0xF != 0 {
            OVERLAP.set(true);
        }
        *word = (*word & !(0xF << shift)) | (ink << shift);
    });
}

/// Draws one row of a glyph: bit i of `bits` is the pixel at x + i. The same as plot() for each set
/// bit, but each tile's row (8 pixels, one 32-bit word) is read and written once.
fn blit_row(c: Surface, x: i32, y: i32, mut bits: u32) {
    let clip = CLIP.get();
    if y < clip.y0 || y >= clip.y1 {
        CLIPPED.set(true);
        return;
    }
    // Drop the pixels left and right of the clip.
    if x < clip.x0 {
        let n = clip.x0 - x;
        let keep = if n >= 32 { 0 } else { !((1u32 << n) - 1) };
        if bits & !keep != 0 {
            CLIPPED.set(true);
        }
        bits &= keep;
    }
    if clip.x1 - x < 32 {
        let n = clip.x1 - x; // pixels that fit
        let keep = if n <= 0 { 0 } else { (1u32 << n) - 1 };
        if bits & !keep != 0 {
            CLIPPED.set(true);
        }
        bits &= keep;
    }
    if bits == 0 {
        return;
    }
    let ink = u32::from(INK.get() & 0xF);
    let overlap = display::with_text(c.0, |tiles, dirty| {
        *dirty |= 1 << (y / 8);
        let row = (y / 8) as usize * 32 * 16 + (y % 8) as usize * 2;
        let mut overlap = false;
        // One tile at a time: shift the row so bit 0 is the tile's first pixel.
        let mut tile = (x >> 3) as usize;
        let mut rest = u64::from(bits) << (x & 7);
        while rest != 0 {
            let span = (rest & 0xFF) as u32;
            if span != 0 {
                let mut mask = 0u32;
                for p in 0..8 {
                    if span & (1 << p) != 0 {
                        mask |= 0xF << (p * 4);
                    }
                }
                let w = row + tile * 16;
                let mut old = u32::from(tiles[w]) | u32::from(tiles[w + 1]) << 16;
                overlap = overlap || old & mask != 0;
                old = (old & !mask) | (mask & ink.wrapping_mul(0x1111_1111));
                tiles[w] = old as u16;
                tiles[w + 1] = (old >> 16) as u16;
            }
            rest >>= 8;
            tile += 1;
        }
        overlap
    });
    if TRACK.get() && overlap {
        OVERLAP.set(true);
    }
}

static ROM_FONT: Global<Option<&'static [u8]>> = Global::new(None);

fn rom_glyph(cp: u16) -> Option<ffi::Glyph> {
    let font = *ROM_FONT.borrow_mut();
    ffi::font_glyph(font?, cp)
}

/// Draws one glyph (or only measures it when c is None); returns the x after it, with one pixel
/// of spacing. ASCII comes from the console font, anything else from the ROM font when there is
/// one, otherwise '?'.
fn glyph(c: Option<Surface>, x: i32, y: i32, mut cp: u32) -> i32 {
    if cp >= 128 {
        match u16::try_from(cp).ok().and_then(rom_glyph) {
            None => cp = u32::from(b'?'),
            Some(g) => {
                if let Some(c) = c {
                    for row in 0..usize::from(g.height).min(16) {
                        blit_row(
                            c,
                            x,
                            y + row as i32,
                            u32::from(g.rows[row]) & ((1u32 << g.width) - 1),
                        );
                    }
                }
                return x + if g.width != 0 {
                    i32::from(g.width) + 1
                } else {
                    SPACE_WIDTH
                };
            }
        }
    }
    let ch = cp as u8;
    let (first, count, digit_width, width, rows) = {
        let f = FONT.borrow_mut();
        (
            f.first,
            f.count,
            f.digit_width,
            i32::from(f.width[usize::from(ch & 0x7F)]),
            f.rows[usize::from(ch & 0x7F)],
        )
    };
    let ix = i32::from(ch) - first;
    if ix < 0 || ix >= count {
        return x + SPACE_WIDTH;
    }
    let digit = ch.is_ascii_digit();
    let digit_pad = if digit {
        (digit_width - 1 - width) / 2
    } else {
        0
    };
    if let Some(c) = c {
        for (row, bits) in rows.iter().enumerate() {
            blit_row(c, x + digit_pad, y + row as i32, u32::from(*bits));
        }
    }
    if digit {
        x + digit_width
    } else {
        x + width + 1
    }
}

/// Decodes one UTF-8 sequence at s[*i] and advances *i; malformed or cut-off bytes give '?'.
fn next_cp(s: &[u8], i: &mut usize) -> u32 {
    let at = |k: usize| s.get(k).copied().unwrap_or(0);
    let b = at(*i);
    let n = if b < 0x80 {
        0
    } else if b & 0xE0 == 0xC0 {
        1
    } else if b & 0xF0 == 0xE0 {
        2
    } else {
        -1
    };
    *i += 1;
    if n < 0 {
        return u32::from(b'?');
    }
    let mut cp = u32::from(if n == 0 {
        b
    } else {
        b & if n == 1 { 0x1F } else { 0x0F }
    });
    for _ in 0..n {
        let t = at(*i);
        if t & 0xC0 != 0x80 {
            return u32::from(b'?');
        }
        cp = (cp << 6) | u32::from(t & 0x3F);
        *i += 1;
    }
    cp
}

// ---- theme ---------------------------------------------------------------------------------
// Each screen has a frame layer (BG1) under the text (BG0): backdrop, dark bars, cream panels with
// a dark rounded border, badges, key chips and box slots, all from tiles generated here. Text picks
// its ink from the frame cell it starts in (dark on cream, light on navy); "\x01" in a string
// toggles the muted shade for labels.

/// Toggles the muted label shade inside a string.
pub const MUTED: &str = "\x01";
/// Toggles red text on a panel ("\x02" toggles blue).
pub const RED: &str = "\x03";

// Palette (BG, both layers): 0 navy backdrop (transparent), then:
const C_CREAM: u8 = 1;
const C_INK: u8 = 2;
const C_MUTED: u8 = 3; // labels on cream
const C_MUTED_L: u8 = 4; // labels on navy
const C_LINE: u8 = 5; // backdrop scanline
const C_SHADOW: u8 = 6; // panel bottom inset
const C_PEACH: u8 = 7;
const C_ORANGE: u8 = 8;
const C_LEGAL: u8 = 9;
const C_FISHY: u8 = 10;
const C_INVALID: u8 = 11;
const C_EMPTY: u8 = 12;
const C_EMPTY_EDGE: u8 = 13;
const C_SAFE: u8 = 14;
const C_WHITE: u8 = 15;

const THEME: [u16; 16] = [
    rgb15(6, 8, 13),
    rgb15(31, 30, 29),
    rgb15(4, 4, 7),
    rgb15(11, 12, 15),
    rgb15(25, 25, 27),
    rgb15(8, 10, 16),
    rgb15(27, 27, 24),
    rgb15(31, 28, 23),
    rgb15(30, 17, 5),
    rgb15(5, 13, 24),
    rgb15(30, 22, 7),
    rgb15(23, 6, 5),
    rgb15(9, 11, 17),
    rgb15(5, 6, 10),
    rgb15(22, 10, 3),
    rgb15(31, 31, 31),
];

const K_BACK: u8 = 0;
const K_BAR: u8 = 1;
const K_PANEL: u8 = 2;
const K_CHIP: u8 = 3;
const K_SLOT: u8 = 4;

// Tile numbers in the frame layer.
const T_BACK: u16 = 1;
const T_BAR: u16 = 2;
const T_PANEL: u16 = 3; // 25: the 5x5 slices of a canonical panel (ring, corners, edges, middle)
const T_SLOT: u16 = 28; // 3 styles x 20 (5x4 cells, 40x32 px)
const T_CHIP: u16 = 88; // left, middle, right, single
const T_BADGE: u16 = 92; // 4 verdict dots (stride 3)
const T_BETWEEN: u16 = 104; // 5: one panel's bottom ring over the next one's top ring, in one row
const T_KEYSEL: u16 = 109; // 3: a selected key on a panel (peach pill, orange edge)
const T_PILL: u16 = 112; // 4: left, middle, right, single; fill 15, edge 13 (set per palette bank)

static KIND: Global<[[u8; TILES]; 2]> = Global::new([[K_BACK; TILES]; 2]);
static IS_MUTED: Var<bool> = Var::new(false);
static TINT: Var<u8> = Var::new(0); // a text colour on panels (BLUE, RED), 0 for none

/// A screen's text layer: 0 top, 1 bottom.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct Surface(pub usize);

pub fn top() -> Surface {
    Surface(0)
}

pub fn bottom() -> Surface {
    Surface(1)
}

fn kind_at(s: usize, i: usize) -> u8 {
    KIND.borrow_mut()[s][i]
}

fn ink_for(c: Surface, x: i32, y: i32) -> u8 {
    let (col, row) = (x / 8, y / 8);
    let k = if (0..32).contains(&col) && (0..24).contains(&row) {
        kind_at(c.0, (row * 32 + col) as usize)
    } else {
        K_BACK
    };
    let dark = k == K_PANEL || k == K_CHIP || k == K_SLOT;
    if TINT.get() != 0 && dark {
        return TINT.get();
    }
    if IS_MUTED.get() {
        return if dark { C_MUTED } else { C_MUTED_L };
    }
    if dark { C_INK } else { C_CREAM }
}

fn inside(px: i32, py: i32, x0: i32, y0: i32, x1: i32, y1: i32, r: i32) -> bool {
    if px < x0 || px >= x1 || py < y0 || py >= y1 {
        return false;
    }
    let cx = if px < x0 + r {
        x0 + r
    } else if px >= x1 - r {
        x1 - r - 1
    } else {
        px
    };
    let cy = if py < y0 + r {
        y0 + r
    } else if py >= y1 - r {
        y1 - r - 1
    } else {
        py
    };
    let (dx, dy) = (px - cx, py - cy);
    dx * dx + dy * dy <= r * r
}

/// Pixel colour of a rounded rectangle [x0,x1)x[y0,y1) with a border `edge` px wide and corner
/// radius r, or -1 outside it.
#[allow(clippy::too_many_arguments)]
fn rounded(
    px: i32,
    py: i32,
    x0: i32,
    y0: i32,
    x1: i32,
    y1: i32,
    r: i32,
    edge: i32,
    fill: i32,
    border: i32,
) -> i32 {
    if !inside(px, py, x0, y0, x1, y1, r) {
        return -1;
    }
    let ri = (r - edge).max(0);
    if inside(px, py, x0 + edge, y0 + edge, x1 - edge, y1 - edge, ri) {
        fill
    } else {
        border
    }
}

fn put_tile(tiles: *mut u16, t: u16, px: &[u8; 64]) {
    for y in 0..8 {
        for h in 0..2 {
            let p = &px[y * 8 + h * 4..];
            let v = u16::from(p[0])
                | u16::from(p[1]) << 4
                | u16::from(p[2]) << 8
                | u16::from(p[3]) << 12;
            // SAFETY: `tiles` is the frame layer's tile VRAM, which holds all `T_COUNT` tiles.
            unsafe { tiles.add(usize::from(t) * 16 + y * 2 + h).write_volatile(v) };
        }
    }
}

fn backdrop(y: i32) -> u8 {
    if y % 8 == 0 { C_LINE } else { 0 }
}

fn panel_px(x: i32, y: i32) -> u8 {
    let mut v = rounded(x, y, 4, 4, 36, 36, 6, 2, C_CREAM.into(), C_INK.into());
    if v == i32::from(C_CREAM) && y >= 32 {
        v = C_SHADOW.into();
    }
    if v < 0 { backdrop(y) } else { v as u8 }
}

fn make_frame_tiles(tiles: *mut u16) {
    let mut px = [0u8; 64];
    let solid = |t: u16, c: u8| put_tile(tiles, t, &[c; 64]);
    put_tile(tiles, 0, &[0; 64]);
    for (i, p) in px.iter_mut().enumerate() {
        *p = backdrop(i as i32 / 8);
    }
    put_tile(tiles, T_BACK, &px);
    solid(T_BAR, C_INK);
    // Panel: content cells 1..3 of a 5x5 grid. The ring cells around them hold 2 px of cream
    // padding, then the 2 px border (so text never touches it), with an inset shadow at the bottom.
    for ty in 0..5 {
        for tx in 0..5 {
            for y in 0..8 {
                for x in 0..8 {
                    px[(y * 8 + x) as usize] = panel_px(tx * 8 + x, ty * 8 + y);
                }
            }
            put_tile(tiles, T_PANEL + (ty * 5 + tx) as u16, &px);
        }
    }
    // Between two panels one row apart: the bottom ring (its upper half) over the top ring (its
    // lower half).
    for tx in 0..5 {
        for y in 0..8 {
            for x in 0..8 {
                px[(y * 8 + x) as usize] = if y < 4 {
                    panel_px(tx * 8 + x, 32 + y)
                } else {
                    panel_px(tx * 8 + x, y)
                };
            }
        }
        put_tile(tiles, T_BETWEEN + tx as u16, &px);
    }
    // Slots (40x32, 4 px apart): filled, empty, selected. Icon frames are 32x32 with the
    // Pokemon drawn low, so the slot is wider than the frame and the icon sits 4 px up in it.
    let fill = [C_CREAM, C_EMPTY, C_PEACH];
    let edge = [C_INK, C_EMPTY_EDGE, C_ORANGE];
    for st in 0..3 {
        for t in 0..20 {
            for y in 0..8 {
                for x in 0..8 {
                    let (gx, gy) = ((t % 5) * 8 + x, (t / 5) * 8 + y);
                    let e = if st == 2 { 3 } else { 2 };
                    let v = rounded(gx, gy, 2, 0, 38, 32, 6, e, fill[st].into(), edge[st].into());
                    px[(y * 8 + x) as usize] = if v < 0 { backdrop(y) } else { v as u8 };
                }
            }
            put_tile(tiles, T_SLOT + (st * 20) as u16 + t as u16, &px);
        }
    }
    // Key chips on a dark bar: a 24 px pill (left, middle, right) and an 8 px round one.
    let cream = i32::from(C_CREAM);
    for t in 0..4 {
        for y in 0..8 {
            for x in 0..8 {
                let v = if t < 3 {
                    rounded(t * 8 + x, y, 0, 0, 24, 8, 3, 0, cream, cream)
                } else {
                    rounded(x, y, 0, 0, 8, 8, 3, 0, cream, cream)
                };
                px[(y * 8 + x) as usize] = if v < 0 { C_INK } else { v as u8 };
            }
        }
        put_tile(tiles, T_CHIP + t as u16, &px);
    }
    // Pills on a cream panel, coloured by palette bank (see pill()).
    for t in 0..4 {
        for y in 0..8 {
            for x in 0..8 {
                let v = if t < 3 {
                    rounded(t * 8 + x, y, 0, 0, 24, 8, 3, 1, 15, 13)
                } else {
                    rounded(x, y, 0, 0, 8, 8, 3, 1, 15, 13)
                };
                px[(y * 8 + x) as usize] = if v < 0 { C_CREAM } else { v as u8 };
            }
        }
        put_tile(tiles, T_PILL + t as u16, &px);
    }
    // A selected key on a cream panel.
    for t in 0..3 {
        for y in 0..8 {
            for x in 0..8 {
                let v = rounded(
                    t * 8 + x,
                    y,
                    0,
                    0,
                    24,
                    8,
                    3,
                    1,
                    C_PEACH.into(),
                    C_ORANGE.into(),
                );
                px[(y * 8 + x) as usize] = if v < 0 { C_CREAM } else { v as u8 };
            }
        }
        put_tile(tiles, T_KEYSEL + t as u16, &px);
    }
    // Verdict dots on a cream panel (the verdict word follows in its colour).
    let badge = [C_LEGAL, C_FISHY, C_INVALID, C_SAFE];
    for (b, colour) in badge.iter().enumerate() {
        for y in 0..8 {
            for x in 0..8 {
                let v = rounded(x, y, 2, 1, 8, 7, 3, 1, (*colour).into(), C_INK.into());
                px[(y * 8 + x) as usize] = if v < 0 { C_CREAM } else { v as u8 };
            }
        }
        put_tile(tiles, T_BADGE + b as u16 * 3, &px);
    }
}

fn set_cell(c: Surface, col: i32, row: i32, tile: u16, kind: u8) {
    if !(0..32).contains(&col) || !(0..24).contains(&row) {
        return;
    }
    let i = (row * 32 + col) as usize;
    display::with_frame_map(c.0, |m| m[i] = tile);
    KIND.borrow_mut()[c.0][i] = kind;
}

/// Clears text rows [row0, row1) of a surface.
fn clear_text_rows(c: Surface, row0: i32, row1: i32) {
    display::with_text(c.0, |tiles, _| {
        tiles[row0 as usize * 32 * 16..row1 as usize * 32 * 16].fill(0);
    });
    display::touch_text_rows(c.0, row0, row1);
}

const NO_STOP: i32 = 1 << 30;

/// Lays text out from (x, y), drawing it (or only measuring when c is None), and returns the x
/// after it. grid_col >= 0 lines runs of two or more spaces up with the cell grid counted from that
/// column (at()'s table gaps). Stops before a glyph whose ink would pass stop_x and draws `suffix`
/// there. Leaves the muted and tint state as the text left it. ink_end (when set) gets the x just
/// past the last glyph's ink (trailing spaces draw nothing).
#[allow(clippy::too_many_arguments)]
fn layout(
    c: Option<Surface>,
    mut x: i32,
    y: i32,
    text: &[u8],
    grid_col: i32,
    stop_x: i32,
    suffix: Option<&[u8]>,
    mut ink_end: Option<&mut i32>,
) -> i32 {
    let mut chars = 0; // characters so far (table gaps line up by character, not byte)
    let mut i = 0;
    while i < text.len() && text[i] != 0 {
        match text[i] {
            1 => {
                IS_MUTED.set(!IS_MUTED.get());
                i += 1;
                continue;
            }
            2 | 3 => {
                let t = if text[i] == 2 { C_LEGAL } else { C_INVALID };
                TINT.set(if TINT.get() == t { 0 } else { t });
                i += 1;
                continue;
            }
            b' ' => {
                let mut run = 1;
                while grid_col >= 0 && text.get(i + run) == Some(&b' ') {
                    run += 1;
                }
                x += SPACE_WIDTH;
                chars += run as i32;
                if run > 1 {
                    x = x.max((grid_col + chars) * 8);
                }
                i += run;
                continue;
            }
            _ => {}
        }
        let cp = next_cp(text, &mut i);
        if stop_x != NO_STOP && glyph(None, x, y, cp) - 1 > stop_x {
            if let Some(suffix) = suffix {
                x = layout(c, x, y, suffix, -1, SCREEN.x1, None, None);
            }
            return x;
        }
        if let Some(c) = c {
            INK.set(ink_for(c, x, y));
        }
        x = glyph(c, x, y, cp);
        if let Some(e) = ink_end.as_deref_mut() {
            *e = x - 1;
        }
        chars += 1;
    }
    x
}

fn reset_style() {
    IS_MUTED.set(false);
    TINT.set(0);
}

fn measure_grid(text: &[u8], grid_col: i32) -> i32 {
    let w = layout(None, 0, 0, text, grid_col, NO_STOP, None, None);
    reset_style();
    w
}

fn measure(text: &[u8]) -> i32 {
    measure_grid(text, -1)
}

fn fault(what: &str, c: Surface, x: i32, y: i32, text: &[u8], over: i32) {
    let side = if c.0 == 1 { "bottom" } else { "top" };
    let mut t = if over != 0 {
        t!(what, " ", side, " ", x, ",", y, " +", over, "px: ")
    } else {
        t!(what, " ", side, " ", x, ",", y, ": ")
    };
    for &b in text {
        if b >= b' ' {
            t.push_byte(b);
        }
    }
    hooks::layout_fault(&t.cap(hooks::LAYOUT_TEXT_LEN));
}

/// Panels (with what sits on them), bars, slots and the backdrop: text stays in the region it
/// starts on.
fn region_group(k: u8) -> i32 {
    match k {
        K_BAR | K_CHIP => 1,
        K_PANEL => 2,
        K_SLOT => 3,
        _ => 0,
    }
}

/// The region around the cell where text drawn at (x, y) starts, in text-layer pixels: the run of
/// cells of its group along that row and down that column. Text stays off a panel's edge pixels.
fn region_at(c: Surface, x: i32, y: i32) -> Clip {
    let kinds = KIND.borrow_mut();
    let kind = &kinds[c.0];
    let col = ((x + 2) / 8).clamp(0, 31);
    let row = (y / 8).clamp(0, 23);
    let at = |r: i32, c: i32| region_group(kind[(r * 32 + c) as usize]);
    let g = at(row, col);
    let (mut l, mut r, mut t, mut b) = (col, col + 1, row, row + 1);
    while l > 0 && at(row, l - 1) == g {
        l -= 1;
    }
    while r < 32 && at(row, r) == g {
        r += 1;
    }
    while t > 0 && at(t - 1, col) == g {
        t -= 1;
    }
    while b < 24 && at(b, col) == g {
        b += 1;
    }
    let pad = if g == 2 { 1 } else { 0 };
    Clip {
        x0: (l * 8 - 2 + pad).max(0),
        y0: t * 8,
        x1: (r * 8 - 2 - pad).min(SCREEN.x1),
        y1: b * 8,
    }
}

/// The x of text's first glyph (after leading spaces and colour toggles).
fn first_glyph_x(x: i32, text: &[u8], grid_col: i32) -> i32 {
    let x = layout(None, x, 0, text, grid_col, -NO_STOP, None, None);
    reset_style();
    x
}

/// Cells from (col, row) to the right edge of the region that cell is in (at least 1).
fn region_cells(c: Surface, col: i32, row: i32) -> i32 {
    if !(0..32).contains(&col) || !(0..24).contains(&row) {
        return 1;
    }
    let kinds = KIND.borrow_mut();
    let kind = &kinds[c.0][(row * 32) as usize..];
    let g = region_group(kind[col as usize]);
    let mut r = col + 1;
    while r < 32 && region_group(kind[r as usize]) == g {
        r += 1;
    }
    r - col
}

/// Draws text inside `clip`, cut with "..." when it doesn't fit; a cut is a fault unless `planned`.
/// Returns the end x. `tint` colours the text (on panels), as a RED or BLUE run would.
#[allow(clippy::too_many_arguments)]
fn draw_in(
    c: Surface,
    x: i32,
    y: i32,
    text: &[u8],
    grid_col: i32,
    clip: Clip,
    planned: bool,
    tint: u8,
) -> i32 {
    let mut ink_end = x;
    layout(
        None,
        x,
        y,
        text,
        grid_col,
        NO_STOP,
        None,
        Some(&mut ink_end),
    );
    reset_style();
    let cut = ink_end > clip.x1;
    let stop = if cut {
        clip.x1 - measure(b"...") - 1
    } else {
        NO_STOP
    };
    CLIP.set(clip);
    TRACK.set(true);
    CLIPPED.set(false);
    OVERLAP.set(false);
    TINT.set(tint);
    let end = layout(
        Some(c),
        x,
        y,
        text,
        grid_col,
        stop,
        if cut { Some(b"...") } else { None },
        None,
    );
    CLIP.set(SCREEN);
    TRACK.set(false);
    reset_style();
    if cut && !planned {
        fault("cut", c, x, y, text, ink_end - clip.x1);
    }
    if CLIPPED.get() {
        fault("clipped", c, x, y, text, 0);
    }
    if OVERLAP.get() {
        fault("overlap", c, x, y, text, 0);
    }
    end
}

/// Draws text at a pixel position without clearing, within the region it starts on (inks follow
/// the frame). Returns the end x.
fn draw_text(c: Surface, x: i32, y: i32, text: &[u8]) -> i32 {
    let clip = region_at(c, first_glyph_x(x, text, -1), y);
    draw_in(c, x, y, text, -1, clip, false, 0)
}

pub fn clear(c: Surface) {
    clear_text_rows(c, 0, 24);
}

/// Text at the 8x8 cell (col, row); clears to the end of the line. Runs of two or more spaces
/// are table gaps and line the next text up with the cell grid.
pub fn at(c: Surface, col: i32, row: i32, text: &[u8]) {
    let text = Text::from(text).cap(128);
    if !(0..24).contains(&row) || !(0..32).contains(&col) {
        return;
    }
    display::with_text(c.0, |tiles, _| {
        let from = (row * 32 + col) as usize * 16;
        tiles[from..from + (32 - col) as usize * 16].fill(0);
    });
    display::touch_text_rows(c.0, row, row + 1);
    let clip = region_at(c, first_glyph_x(col * 8, &text, col), row * 8);
    draw_in(c, col * 8, row * 8, &text, col, clip, false, 0);
}

/// UTF-16 to drawable UTF-8: ASCII as is; other characters as UTF-8 when the ROM font has them,
/// otherwise common accented letters folded and the rest '?'. Cut short rather than split one.
/// Stops at a 0 or the end of `s`, and fits a C buffer of `cap` bytes.
pub fn utf16_to_text(s: &[u16], cap: usize) -> Text {
    let mut out = Text::new();
    let mut n = 0;
    for &c in s.iter().take_while(|&&c| c != 0) {
        if n + 1 >= cap {
            break;
        }
        let o = if (0x20..0x7F).contains(&c) {
            c as u8
        } else if c == 0x2642 {
            b'M' // male sign (Nidoran)
        } else if c == 0x2640 {
            b'F'
        } else if c == 0x3000 {
            b' '
        } else if c >= 0x80 && rom_glyph(c).is_some() {
            let len = if c < 0x800 { 2 } else { 3 };
            if n + len + 1 > cap {
                break;
            }
            if len == 2 {
                out.push_byte(0xC0 | (c >> 6) as u8);
            } else {
                out.push_byte(0xE0 | (c >> 12) as u8);
                out.push_byte(0x80 | ((c >> 6) & 0x3F) as u8);
            }
            out.push_byte(0x80 | (c & 0x3F) as u8);
            n += len;
            continue;
        } else if (0xE8..=0xEB).contains(&c) || (0xC8..=0xCB).contains(&c) {
            b'e'
        } else {
            b'?'
        };
        out.push_byte(o);
        n += 1;
    }
    out
}

/// The font cache built from the user's ROM, kept for the program's lifetime.
pub fn set_rom_font(cache: &'static [u8]) {
    *ROM_FONT.borrow_mut() = Some(cache);
}

pub fn has_rom_font() -> bool {
    ROM_FONT.borrow_mut().is_some()
}

/// Whether c can be drawn (ASCII, or in the ROM font).
pub fn has_glyph(c: u16) -> bool {
    (0x20..0x7F).contains(&c) || rom_glyph(c).is_some()
}

/// Pill colours per screen: palette bank b (1..15) is the theme with colour 15 = colours[b].
struct Pills {
    colours: [[u16; 16]; 2],
    used: [i32; 2],
}
static PILLS: Global<Pills> = Global::new(Pills {
    colours: [[0; 16]; 2],
    used: [0; 2],
});

/// Scene changes reset both screens to the backdrop. Text on a panel, chip or light badge is
/// drawn in dark ink, elsewhere in light ink.
pub fn frame_clear(c: Surface) {
    display::with_frame_map(c.0, |m| m[..TILES].fill(T_BACK));
    KIND.borrow_mut()[c.0].fill(K_BACK);
    PILLS.borrow_mut().used[c.0] = 0;
}

/// Back to the backdrop (frame and text) for rows [row0, row1).
pub fn frame_clear_rows(c: Surface, row0: i32, row1: i32) {
    for row in row0..row1.min(24) {
        for col in 0..32 {
            set_cell(c, col, row, T_BACK, K_BACK);
        }
        clear_text_rows(c, row, row + 1);
    }
}

/// A rounded pill of a colour with its text centred (white or dark ink to suit the colour), for
/// types and move categories on a panel; returns the column after it. Up to 15 colours per screen
/// between frame_clear calls; past that, pills are white.
pub fn pill(c: Surface, col: i32, row: i32, color: u16, text: &[u8]) -> i32 {
    let s = c.0;
    let mut bank = 0;
    {
        let mut p = PILLS.borrow_mut();
        for b in 1..=p.used[s] {
            if p.colours[s][b as usize] == color {
                bank = b;
                break;
            }
        }
        if bank == 0 && p.used[s] < 15 {
            p.used[s] += 1;
            bank = p.used[s];
            p.colours[s][bank as usize] = color;
            let (r, g, b) = (color & 31, color >> 5 & 31, color >> 10 & 31);
            display::with_bg_palette(s, |pal| {
                let pal = &mut pal[16 * bank as usize..16 * bank as usize + 16];
                pal.copy_from_slice(&THEME);
                pal[15] = color;
                pal[13] = rgb15(r * 3 / 4, g * 3 / 4, b * 3 / 4); // edge
            });
        }
    }
    // White text unless the colour is light (luma on a 0..31 scale).
    let (r, g, b) = (
        i32::from(color & 31),
        i32::from(color >> 5 & 31),
        i32::from(color >> 10 & 31),
    );
    let ink = if bank != 0 && (r * 3 + g * 6 + b) / 10 < 18 {
        C_WHITE
    } else {
        C_INK
    };
    let w = measure(text);
    let room = region_cells(c, col, row); // a long pill stops at its panel's edge (its text is cut)
    let cells = ((w + 8 + 7) / 8).min(room);
    let pb = (bank as u16) << 12;
    for k in 0..cells {
        let t = if cells == 1 {
            3
        } else if k == 0 {
            0
        } else if k == cells - 1 {
            2
        } else {
            1
        };
        set_cell(c, col + k, row, pb | (T_PILL + t), K_PANEL);
    }
    let (x, y) = (col * 8 + (cells * 8 - w) / 2 - 2, row * 8);
    let clip = region_at(c, first_glyph_x(x, text, -1), y);
    draw_in(c, x, y, text, -1, clip, false, ink);
    col + cells
}

/// Row 0 as a dark bar with a title.
pub fn title(c: Surface, text: &[u8]) {
    let text = Text::from(text).cap(96);
    for col in 0..32 {
        set_cell(c, col, 0, T_BAR, K_BAR);
    }
    clear_text_rows(c, 0, 1);
    draw_text(c, 4, 0, &text);
}

/// Right-aligned text on the title bar.
pub fn title_right(c: Surface, text: &[u8]) {
    let w = measure(text);
    draw_text(c, 250 - w, 0, text);
}

/// Lays out button hints on row 23 with at least `gap` px before each chip after the first; draws
/// them when c is set. Returns the x after the last label.
fn hint_layout(c: Option<Surface>, spec: &[u8], gap: i32) -> i32 {
    const ROW: i32 = 23;
    let spec = Text::from(spec).cap(96);
    let (mut col, mut x) = (0, 0);
    for item in spec.split(|&b| b == b'|').filter(|i| !i.is_empty()) {
        if col >= 31 {
            break;
        }
        let (key, label) = match item.iter().position(|&b| b == b':') {
            Some(i) => (&item[..i], Some(&item[i + 1..])),
            None => (item, None),
        };
        let kw = measure(key);
        let cells = if kw <= 6 { 1 } else { (kw + 9) / 8 };
        if let Some(c) = c {
            if cells == 1 {
                set_cell(c, col, ROW, T_CHIP + 3, K_CHIP);
            } else {
                for k in 0..cells {
                    let t = if k == 0 {
                        0
                    } else if k == cells - 1 {
                        2
                    } else {
                        1
                    };
                    set_cell(c, col + k, ROW, T_CHIP + t, K_CHIP);
                }
            }
            draw_text(c, col * 8 + (cells * 8 - kw) / 2 - 1, ROW * 8, key);
        }
        x = col * 8 + cells * 8 + 2;
        if let Some(label) = label {
            x = match c {
                Some(c) => draw_text(c, x, ROW * 8, label),
                None => x + measure(label),
            };
        }
        col = (x + gap + 7) / 8;
    }
    x
}

/// Row 23 as a dark bar of button hints: "A:Open|B:Back|L/R:Box".
pub fn hints(c: Surface, spec: &[u8]) {
    const ROW: i32 = 23;
    for col in 0..32 {
        set_cell(c, col, ROW, T_BAR, K_BAR);
    }
    clear_text_rows(c, ROW, ROW + 1);
    // Each key gets a chip, then its label, at least 5 px apart, or 1 px when that doesn't fit
    // the bar.
    let gap = if hint_layout(None, spec, 5) - 1 > SCREEN.x1 {
        1
    } else {
        5
    };
    hint_layout(Some(c), spec, gap);
}

/// Row 0 as a tab bar: equal segments, the active one a cream chip.
pub fn tabs(c: Surface, names: &[&str], active: i32) {
    for col in 0..32 {
        set_cell(c, col, 0, T_BAR, K_BAR);
    }
    clear_text_rows(c, 0, 1);
    // n equal segments (taps map by segment); the active tab is a cream chip.
    let n = names.len() as i32;
    for (t, name) in names.iter().enumerate() {
        let t = t as i32;
        let (x0, x1) = (t * 256 / n, (t + 1) * 256 / n);
        let w = measure(name.as_bytes());
        if t == active {
            let cells = (w + 9) / 8;
            let col = (x0 + x1) / 16 - cells / 2;
            for k in 0..cells {
                let tile = if k == 0 {
                    0
                } else if k == cells - 1 {
                    2
                } else {
                    1
                };
                set_cell(c, col + k, 0, T_CHIP + tile, K_CHIP);
            }
            draw_text(c, col * 8 + (cells * 8 - w) / 2 - 1, 0, name.as_bytes());
        } else {
            let muted = t!(MUTED, *name).cap(32);
            draw_text(c, (x0 + x1) / 2 - w / 2 - 2, 0, &muted);
        }
    }
}

/// A 3-cell peach pill on a panel behind a selected key.
pub fn key_highlight(c: Surface, col: i32, row: i32) {
    for k in 0..3 {
        set_cell(c, col + k, row, T_KEYSEL + k as u16, K_PANEL);
    }
}

/// A cream panel over cells [col, col+w) x [row, row+h) (w, h >= 2); its border is drawn just
/// outside them.
pub fn panel(c: Surface, col: i32, row: i32, w: i32, h: i32) {
    if w < 2 || h < 2 {
        return;
    }
    let s = c.0;
    let slice = |v: i32, start: i32, len: i32| {
        if v < start {
            0
        } else if v == start {
            1
        } else if v < start + len - 1 {
            2
        } else if v == start + len - 1 {
            3
        } else {
            4
        }
    };
    for y in row - 1..=row + h {
        for x in col - 1..=col + w {
            let (cx, cy) = (slice(x, col, w), slice(y, row, h));
            let inner = (1..=3).contains(&cx) && (1..=3).contains(&cy);
            let on_screen = (0..32).contains(&x) && (0..24).contains(&y);
            let i = (y * 32 + x) as usize;
            // Leave bars alone where a panel's ring meets them.
            if !inner && on_screen && kind_at(s, i) == K_BAR {
                continue;
            }
            let mut tile = T_PANEL + (cy * 5 + cx) as u16;
            // A top ring meeting another panel's bottom ring (or the reverse) shares the row.
            if (cy == 0 || cy == 4) && on_screen {
                let old = display::frame_map_at(s, i);
                let other = T_PANEL + (if cy == 0 { 4 } else { 0 } * 5 + cx) as u16;
                if old == other || old == T_BETWEEN + cx as u16 {
                    tile = T_BETWEEN + cx as u16;
                }
            }
            set_cell(c, x, y, tile, if inner { K_PANEL } else { K_BACK });
        }
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum Badge {
    Legal,
    Fishy,
    Invalid,
    Safe,
}

/// A verdict on a panel: a coloured dot, then `text` in the verdict's colour. Returns the first
/// column after it.
pub fn badge(c: Surface, col: i32, row: i32, b: Badge, text: &[u8]) -> i32 {
    // The word takes its verdict's colour (amber is too light for text: burnt orange).
    const INKS: [u8; 4] = [C_LEGAL, C_SAFE, C_INVALID, C_SAFE];
    let room = region_cells(c, col, row); // a long badge stops at its panel's edge (its text is cut)
    let cells = (1 + (measure(text) + 9) / 8).min(room);
    set_cell(c, col, row, T_BADGE + b as u16 * 3, K_PANEL);
    for k in 1..cells {
        set_cell(c, col + k, row, T_PANEL + 2 * 5 + 2, K_PANEL);
    }
    let (x, y) = ((col + 1) * 8, row * 8);
    draw_in(
        c,
        x,
        y,
        text,
        -1,
        region_at(c, x, y),
        false,
        INKS[b as usize],
    );
    col + cells
}

/// A box slot, 5x4 cells (40x32 px) at a cell: style 0 filled, 1 empty, 2 selected. Draw the
/// game's icon at SLOT_ICON_X/Y from the slot's corner (its frames draw the Pokemon low).
pub fn slot(c: Surface, col: i32, row: i32, style: i32) {
    for t in 0..20 {
        set_cell(
            c,
            col + t % 5,
            row + t / 5,
            T_SLOT + ((style % 3) * 20 + t) as u16,
            K_SLOT,
        );
    }
}
pub const SLOT_ICON_X: i32 = 4;
pub const SLOT_ICON_Y: i32 = -4;

/// A type's colour (the placeholder icons' palette).
pub fn type_color(t: i32) -> u16 {
    TYPE_COLOR[if (0..TYPES as i32).contains(&t) {
        t as usize
    } else {
        TYPES - 1
    }][0]
}

/// Width in pixels of text as drawn.
pub fn text_width(s: &[u8]) -> i32 {
    measure(s)
}

/// Text at a pixel position (no clearing), for layouts off the 8 px row grid. Panels, badges and
/// pills stay on the grid.
pub fn text(c: Surface, x: i32, y: i32, text: &[u8]) {
    draw_text(c, x, y, &Text::from(text).cap(128));
}

/// Shortens UTF-8 text in place to fit `width` px, ending it with "..." when cut.
pub fn fit<const N: usize>(text: &mut crate::text::TextN<N>, width: i32) {
    if measure(text) <= width {
        return;
    }
    let room = width - measure(b"...");
    let mut n = text.len();
    while n > 0 {
        n -= 1;
        while n > 0 && text[n] & 0xC0 == 0x80 {
            n -= 1;
        }
        if measure(&text[..n]) <= room {
            break;
        }
    }
    text.truncate(n);
    text.push_bytes(b"...");
}

/// Text ending at pixel x_end; returns where it starts.
pub fn text_right(c: Surface, x_end: i32, y: i32, text: &[u8]) -> i32 {
    let text = Text::from(text).cap(128);
    let x = x_end - measure(&text);
    draw_text(c, x, y, &text);
    x
}

/// Text from x that ends before x_end, cut with "..." when longer: for a column with a neighbour
/// on its right (draw the neighbour first, e.g. with text_right). The cut is expected, not a fault.
pub fn text_in(c: Surface, x: i32, x_end: i32, y: i32, text: &[u8]) {
    let text = Text::from(text).cap(128);
    let mut clip = region_at(c, first_glyph_x(x, &text, -1), y);
    clip.x1 = clip.x1.min(x_end);
    draw_in(c, x, y, &text, -1, clip, true, 0);
}

// Spacing rules for text in panels (see docs/research/ui-stack.md, Spacing): plain lines on a
// 12 px pitch (1.5x the 8 px font), lines that carry a badge or pill on the 8 px tile grid 16 px
// apart, and 12 px of space before a new block. Text sits INSET_TOP / INSET_BOTTOM px inside a
// panel's content (top-screen panels start at column 1, bottom-screen panels span the screen).
pub const PITCH: i32 = 12;
pub const GRID_PITCH: i32 = 16;
pub const BLOCK_GAP: i32 = 20;
pub const INSET_TOP: i32 = 16;
pub const INSET_BOTTOM: i32 = 12;

/// A cursor down a panel: line() gives the y for the next plain line, grid_row() the tile row
/// for the next line with a badge or pill (draw its text at row * 8), block() starts a new block.
pub struct Lines {
    pub y: i32, // top of the next line, px
    pending_gap: bool,
}

impl Lines {
    /// Text starts 4 px (about the side padding) below the panel content's top.
    pub fn new(row: i32) -> Self {
        Self::inset(row, 4)
    }

    pub fn inset(row: i32, inset: i32) -> Self {
        Self {
            y: row * 8 + inset,
            pending_gap: false,
        }
    }

    pub fn line(&mut self) -> i32 {
        if self.pending_gap {
            self.y += BLOCK_GAP - PITCH;
        }
        self.pending_gap = false;
        let at = self.y;
        self.y += PITCH;
        at
    }

    pub fn grid_row(&mut self) -> i32 {
        if self.pending_gap {
            self.y += BLOCK_GAP - PITCH;
        }
        self.pending_gap = false;
        let row = (self.y + 7) / 8;
        self.y = row * 8 + GRID_PITCH;
        row
    }

    pub fn block(&mut self) {
        self.pending_gap = true;
    }
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum ItemKind {
    Text,
    Shrink,
    Badge,
    Pill,
}

#[derive(Clone, Copy)]
struct Item {
    kind: ItemKind,
    badge: Badge,
    color: u16,
    right: bool,
    s: Text,
}

/// A line of text and badges on tile row `row` of a panel, laid out like a CSS flex row: items
/// queued with text(), shrink(), badge() and pill() flow from pixel x; after push_right(), the rest
/// are packed against the panel's right edge (margin-left: auto). Badges and pills land on whole
/// cells. A shrink() item (last on the left; on the right it is plain text) is cut with "..." to
/// the room left; anything else that doesn't fit is a layout fault ("crowded"). draw() lays the
/// row out and draws it.
pub struct Row {
    c: Surface,
    row: i32,
    x: i32,
    right: bool,
    items: [Item; 6],
    n: usize,
}

const NO_ITEM: Item = Item {
    kind: ItemKind::Text,
    badge: Badge::Legal,
    color: 0,
    right: false,
    s: Text::new(),
};

impl Row {
    pub fn new(c: Surface, row: i32, x: i32) -> Self {
        Self {
            c,
            row,
            x,
            right: false,
            items: [NO_ITEM; 6],
            n: 0,
        }
    }

    fn add(&mut self, kind: ItemKind, badge: Badge, color: u16, text: &[u8]) -> &mut Self {
        if self.n == self.items.len() {
            fault("crowded", self.c, self.x, self.row * 8, text, 0);
            return self;
        }
        let s = Text::from(text).cap(64);
        self.items[self.n] = Item {
            kind,
            badge,
            color,
            right: self.right,
            s,
        };
        self.n += 1;
        self
    }

    pub fn text(&mut self, text: &[u8]) -> &mut Self {
        self.add(ItemKind::Text, Badge::Legal, 0, text)
    }

    pub fn shrink(&mut self, text: &[u8]) -> &mut Self {
        let kind = if self.right {
            ItemKind::Text
        } else {
            ItemKind::Shrink
        };
        self.add(kind, Badge::Legal, 0, text)
    }

    pub fn badge(&mut self, b: Badge, text: &[u8]) -> &mut Self {
        self.add(ItemKind::Badge, b, 0, text)
    }

    pub fn pill(&mut self, color: u16, text: &[u8]) -> &mut Self {
        self.add(ItemKind::Pill, Badge::Legal, color, text)
    }

    pub fn push_right(&mut self) -> &mut Self {
        self.right = true;
        self
    }

    // Spacing (the rules the screens used by hand): a badge or pill starts on the first whole cell
    // at least 2 px past text before it, text starts 4 px past whatever is before it, text before a
    // badge on the right ends 4 px short of its cell (8 px before a pill), a pill on the right stops
    // a cell short of the panel's edge (a badge reaches it), and text ends 4 px inside the panel.
    pub fn draw(&mut self) {
        let (c, row) = (self.c, self.row);
        let y = row * 8;
        let col0 = self.x / 8;
        let end_col = col0 + region_cells(c, col0, row);
        let text_end = end_col * 8 - 4;
        let cells = |it: &Item| {
            if it.kind == ItemKind::Pill {
                (measure(&it.s) + 15) / 8
            } else {
                1 + (measure(&it.s) + 9) / 8
            }
        };
        let put = |it: &Item, col: i32| {
            if it.kind == ItemKind::Pill {
                pill(c, col, row, it.color, &it.s)
            } else {
                badge(c, col, row, it.badge, &it.s)
            }
        };
        let items = &mut self.items[..self.n];
        let mut gap_before = 4; // between right-hand text and the cell item after it

        // The right-hand items, from the edge inwards.
        let mut right_start = end_col * 8;
        let mut at_edge = true;
        for it in items.iter().rev().take_while(|it| it.right) {
            if matches!(it.kind, ItemKind::Badge | ItemKind::Pill) {
                let edge = if it.kind == ItemKind::Pill {
                    end_col - 1
                } else {
                    end_col
                };
                let col = (if at_edge { edge } else { (right_start - 4) / 8 }) - cells(it);
                put(it, col);
                right_start = col * 8;
                gap_before = if it.kind == ItemKind::Pill { 8 } else { 4 };
            } else {
                let x = (if at_edge {
                    text_end
                } else {
                    right_start - gap_before
                }) - measure(&it.s);
                gap_before = 4;
                draw_text(c, x, y, &it.s);
                right_start = x;
            }
            at_edge = false;
        }
        let any_right = !at_edge;
        let limit = if any_right { right_start - 4 } else { text_end };

        // The left-hand items, in order.
        let mut x = self.x;
        let mut prev = ItemKind::Shrink; // stands for "nothing yet"
        let mut first = true;
        for it in items.iter_mut().take_while(|it| !it.right) {
            if matches!(it.kind, ItemKind::Badge | ItemKind::Pill) {
                let after_cells = matches!(prev, ItemKind::Badge | ItemKind::Pill);
                let col = if first {
                    (x + 7) / 8
                } else if after_cells {
                    x / 8 + 1
                } else {
                    (x + 9) / 8
                };
                let end = put(it, col);
                if any_right && end * 8 > right_start {
                    fault("crowded", c, col * 8, y, &it.s, end * 8 - right_start);
                }
                x = end * 8;
            } else {
                if !first {
                    x += 4;
                }
                if it.kind == ItemKind::Shrink && x + measure(&it.s) > limit {
                    let mut s = it.s;
                    fit(&mut s, limit - x);
                    it.s = s.cap(64);
                }
                let w = measure(&it.s);
                if x + w > limit {
                    fault("crowded", c, x, y, &it.s, x + w - limit);
                }
                draw_text(c, x, y, &it.s);
                x += w;
            }
            prev = it.kind;
            first = false;
        }
    }
}

/// Clears text pixel rows [y0, y1) across the screen.
pub fn clear_px(c: Surface, y0: i32, y1: i32) {
    display::with_text(c.0, |tiles, dirty| {
        for y in y0.max(0)..y1.min(192) {
            for col in 0..32 {
                let at = ((y / 8 * 32 + col) * 16 + (y % 8) * 2) as usize;
                tiles[at] = 0;
                tiles[at + 1] = 0;
                *dirty |= 1 << (y / 8);
            }
        }
    });
}

/// Draws text at a pixel position with a line pitch, word-wrapped to width px, on at most
/// max_rows rows (no clearing); returns the rows used. A word that doesn't fit on the last row
/// ends it with "...". Muted: the label shade.
#[allow(clippy::too_many_arguments)]
pub fn wrap_px(
    c: Surface,
    x: i32,
    y: i32,
    pitch: i32,
    width: i32,
    max_rows: i32,
    text: &[u8],
    muted: bool,
) -> i32 {
    const LINE: usize = 96;
    let text = crate::text::cstr(text);
    let at = |k: usize| text.get(k).copied().unwrap_or(0);
    let mut rows = 0;
    let mut p = 0;
    while at(p) != 0 && rows < max_rows {
        while at(p) == b' ' {
            p += 1;
        }
        let mut n = 0; // bytes of p on this line
        loop {
            let mut w = p + n;
            while at(w) == b' ' {
                w += 1;
            }
            let mut e = w;
            while at(e) != 0 && at(e) != b' ' {
                e += 1;
            }
            if e == w {
                break;
            }
            let len = e - p;
            if len >= LINE {
                break;
            }
            if n != 0 && text_width(&text[p..e]) > width {
                break;
            }
            n = len;
        }
        if n == 0 {
            // One word wider than the line: cut it.
            n = (text.len() - p).min(LINE - 1);
        }
        let rest = &text[(p + n).min(text.len())..];
        let last = rows == max_rows - 1 && rest.iter().any(|&b| b != b' ');
        let mut line = Text::from(&text[p..p + n]);
        if last {
            // Make room for "..." by dropping words from the end.
            let mut m = line.len();
            while m > 0 && text_width(&line[..m]) + text_width(b"...") > width {
                while m > 0 && line[m - 1] != b' ' {
                    m -= 1;
                }
                while m > 0 && line[m - 1] == b' ' {
                    m -= 1;
                }
            }
            line.truncate(m);
            line.push_bytes(b"...");
        }
        if muted {
            self::text(c, x, y + rows * pitch, &t!(MUTED, &line));
        } else {
            self::text(c, x, y + rows * pitch, &line);
        }
        rows += 1;
        p += n;
    }
    rows
}

/// Characters (not bytes) in UTF-8 text, for centring.
pub fn text_len(s: &[u8]) -> i32 {
    s.iter().filter(|&&b| b & 0xC0 != 0x80).count() as i32
}

/// A name from the strings blob ("?" when unknown).
pub fn name(category: u32, id: u32) -> Text {
    let mut w = [0u16; 32];
    if ffi::name(category, id, &mut w) == 0 {
        return Text::from(b"?");
    }
    utf16_to_text(&w, 32)
}

/// A location name ("Loc <id>" when unknown).
pub fn location(generation: u8, id: u16) -> Text {
    let mut w = [0u16; 32];
    if ffi::location_name(generation, id, &mut w) == 0 {
        return t!("Loc ", id);
    }
    utf16_to_text(&w, 32)
}

// ---- sprites -------------------------------------------------------------------------------

const TYPES: usize = 18;

/// Type colours (original palette): {body, shade}.
const TYPE_COLOR: [[u16; 2]; TYPES] = [
    [rgb15(22, 22, 18), rgb15(14, 14, 11)], // Normal
    [rgb15(24, 8, 6), rgb15(14, 4, 3)],     // Fighting
    [rgb15(18, 20, 30), rgb15(10, 12, 20)], // Flying
    [rgb15(20, 10, 22), rgb15(12, 5, 14)],  // Poison
    [rgb15(26, 22, 12), rgb15(17, 13, 6)],  // Ground
    [rgb15(22, 19, 10), rgb15(13, 11, 5)],  // Rock
    [rgb15(18, 24, 6), rgb15(10, 15, 3)],   // Bug
    [rgb15(14, 12, 22), rgb15(7, 6, 13)],   // Ghost
    [rgb15(22, 22, 26), rgb15(13, 13, 17)], // Steel
    [rgb15(30, 14, 6), rgb15(20, 7, 3)],    // Fire
    [rgb15(10, 16, 30), rgb15(5, 9, 20)],   // Water
    [rgb15(12, 26, 10), rgb15(6, 16, 5)],   // Grass
    [rgb15(31, 26, 6), rgb15(21, 16, 2)],   // Electric
    [rgb15(30, 12, 18), rgb15(20, 6, 10)],  // Psychic
    [rgb15(18, 28, 28), rgb15(10, 18, 19)], // Ice
    [rgb15(14, 8, 30), rgb15(8, 4, 19)],    // Dragon
    [rgb15(10, 8, 8), rgb15(5, 4, 4)],      // Dark
    [rgb15(20, 20, 20), rgb15(12, 12, 12)], // unknown
];

struct Sprites {
    icon: [Option<NonNull<u16>>; TYPES], // 8bpp 32x32, one per type
    egg: Option<NonNull<u16>>,
    star: Option<NonNull<u16>>,
    /// Row highlights, 8, 12 and 16 px tall: [height index][on a panel].
    bars: [[Option<NonNull<u16>>; 2]; 3],
    rom_slot: [Option<Gfx>; 128], // one 8bpp 32x32 graphic per OAM id that shows a ROM icon
    item: [Option<Gfx>; 2],
    pokemon: [Option<Gfx>; 4],
    logo: Option<Gfx>,
    small: [Option<Gfx>; SMALL_ICONS],
}

static SPRITES: Global<Sprites> = Global::new(Sprites {
    icon: [None; TYPES],
    egg: None,
    star: None,
    bars: [[None; 2]; 3],
    rom_slot: [None; 128],
    item: [None; 2],
    pokemon: [None; 4],
    logo: None,
    small: [None; SMALL_ICONS],
});

/// Sprite palette (8bpp, shared): 1 outline, 2 eye, 3 egg, 4 egg spots, 5 star, 6 orange,
/// 7 bar, then 16 + 2*type body/shade.
fn palette(pal: &mut [u16]) {
    pal[1] = rgb15(2, 3, 5);
    pal[2] = rgb15(31, 31, 31);
    pal[3] = rgb15(30, 29, 24);
    pal[4] = rgb15(12, 24, 12);
    pal[5] = rgb15(31, 28, 4);
    pal[6] = rgb15(30, 17, 5); // theme orange
    pal[7] = rgb15(8, 14, 24); // highlight on navy (scenes without panels)
    pal[8] = rgb15(31, 28, 23); // highlight on a panel: peach
    pal[9] = rgb15(30, 17, 5); //   with an orange edge
    for (t, c) in TYPE_COLOR.iter().enumerate() {
        pal[16 + 2 * t] = c[0];
        pal[17 + 2 * t] = c[1];
    }
}

/// Writes an 8bpp pixel into a sprite buffer of width w (1D tile mapping).
fn put8(buf: &mut [u8], w: i32, x: i32, y: i32, c: u8) {
    let tile = (y / 8) * (w / 8) + x / 8;
    buf[(tile * 64 + (y % 8) * 8 + (x % 8)) as usize] = c;
}

/// Little-endian bytes to sprite VRAM.
fn copy_to_vram(dst: NonNull<u16>, src: &[u8]) {
    for (i, h) in src.as_chunks::<2>().0.iter().enumerate() {
        // SAFETY: `dst` is sprite VRAM from `oamAllocateGfx` for a graphic of `src.len()` bytes.
        unsafe {
            dst.as_ptr()
                .add(i)
                .write_volatile(u16::from(h[0]) | u16::from(h[1]) << 8)
        };
    }
}

/// Little-endian bytes to a RAM copy.
fn copy16(dst: &mut [u16], src: &[u8]) {
    for (d, h) in dst.iter_mut().zip(src.as_chunks::<2>().0) {
        *d = u16::from_le_bytes(*h);
    }
}

/// A round creature: body with a lighter top, outline, two eyes and small ears.
fn draw_creature(buf: &mut [u8], body: u8, shade: u8) {
    buf[..32 * 32].fill(0);
    for y in 0..32 {
        for x in 0..32 {
            let (dx, dy) = (x - 16, y - 18);
            let d = dx * dx + (dy * dy * 5) / 4;
            let ear = (4..10).contains(&y)
                && (((7..11).contains(&x) && x - 7 >= 9 - y)
                    || ((21..25).contains(&x) && 24 - x >= 9 - y));
            if d < 11 * 11 || ear {
                let c = if d >= 10 * 10 && !ear {
                    1
                } else if dy > 3 {
                    shade
                } else {
                    body
                };
                put8(buf, 32, x, y, c);
            }
        }
    }
    for ex in [12, 19] {
        for y in 15..18 {
            for x in ex..ex + 2 {
                put8(buf, 32, x, y, 1);
            }
        }
    }
    put8(buf, 32, 12, 15, 2);
    put8(buf, 32, 19, 15, 2);
}

fn draw_egg(buf: &mut [u8]) {
    buf[..32 * 32].fill(0);
    for y in 0..32 {
        for x in 0..32 {
            let (dx, dy) = (x - 16, y - 17);
            let d = dx * dx * 16 / 10 + dy * dy;
            if d < 12 * 12 {
                let c = if d >= 11 * 11 {
                    1
                } else if (x * 7 + y * 3) % 11 == 0 {
                    4
                } else {
                    3
                };
                put8(buf, 32, x, y, c);
            }
        }
    }
}

/// Sets up both screens: modes, VRAM, the text and frame layers, sprites and their art.
pub fn sprites_init() {
    // Consoles also live here so all display setup is in one place.
    video::set_mode(Engine::Main, sys::MODE_0_2D);
    video::set_mode(Engine::Sub, sys::MODE_0_2D);
    video::set_primary_banks(
        sys::VRAM_A_MAIN_BG,
        sys::VRAM_B_MAIN_SPRITE,
        sys::VRAM_C_SUB_BG,
        sys::VRAM_D_SUB_SPRITE,
    );
    measure_font();
    let (text4, size) = (sys::BgType_Text4bpp, sys::BgSize_T_256x256);
    let main_bg = bg::init(Engine::Main, 0, text4, size, 31, 1);
    let sub_bg = bg::init(Engine::Sub, 0, text4, size, 31, 1);
    let main_frame = bg::init(Engine::Main, 1, text4, size, 30, 3);
    let sub_frame = bg::init(Engine::Sub, 1, text4, size, 30, 3);
    make_frame_tiles(main_frame.gfx_ptr());
    make_frame_tiles(sub_frame.gfx_ptr());
    main_frame.set_priority(3);
    sub_frame.set_priority(3);
    for s in 0..2 {
        let (text_bg, frame_bg) = if s == 1 {
            (sub_bg, sub_frame)
        } else {
            (main_bg, main_frame)
        };
        // The text layer's map is fixed (one tile per cell), so it is written once here.
        let map = text_bg.map_ptr();
        for i in 0..32 * 32 {
            let v = if i < TILES { i as u16 } else { 0 };
            // SAFETY: the layer's 32x32 map in VRAM.
            unsafe { map.add(i).write_volatile(v) };
        }
        display::init(s, text_bg.gfx_ptr(), frame_bg.map_ptr());
        clear(Surface(s));
        frame_clear(Surface(s));
        display::with_bg_palette(s, |pal| pal[..16].copy_from_slice(&THEME));
    }
    main_bg.set_priority(0);
    sub_bg.set_priority(0);
    // Text sits 2 px right of the cell grid, so it clears panel edges.
    main_bg.set_scroll(-2, 0);
    sub_bg.set_scroll(-2, 0);
    bg::update();

    Oam::main().init(sys::SpriteMapping_1D_64, false);
    Oam::sub().init(sys::SpriteMapping_1D_64, false);
    display::with_sprite_palette(0, palette);
    display::with_sprite_palette(1, palette);

    let (s32, s8, s64x32) = (
        sys::SpriteSize_32x32,
        sys::SpriteSize_8x8,
        sys::SpriteSize_64x32,
    );
    let c256 = sys::SpriteColorFormat_256Color;
    let sub = Oam::sub();
    let mut sp = SPRITES.borrow_mut();
    let mut buf = alloc::vec![0u8; 64 * 32];
    for (t, icon) in sp.icon.iter_mut().enumerate() {
        *icon = sub.alloc_gfx(s32, c256);
        draw_creature(&mut buf, 16 + 2 * t as u8, 17 + 2 * t as u8);
        if let Some(g) = *icon {
            copy_to_vram(g, &buf[..32 * 32]);
        }
    }
    sp.egg = sub.alloc_gfx(s32, c256);
    draw_egg(&mut buf);
    if let Some(g) = sp.egg {
        copy_to_vram(g, &buf[..32 * 32]);
    }

    sp.star = sub.alloc_gfx(s8, c256);
    const STAR: &str = concat!(
        "...X....", "..XXX...", "XXXXXXX.", ".XXXXX..", "..XXX...", ".XX.XX..", ".X...X..",
        "........"
    );
    for (b, s) in buf.iter_mut().zip(STAR.bytes()) {
        *b = if s == b'X' { 5 } else { 0 };
    }
    if let Some(g) = sp.star {
        copy_to_vram(g, &buf[..64]);
    }

    // Navy blue off panels; peach with an orange edge on them.
    for (k, bars) in sp.bars.iter_mut().enumerate() {
        for (on, bar) in bars.iter_mut().enumerate() {
            let h = 8 + 4 * k as i32;
            *bar = sub.alloc_gfx(s64x32, c256);
            buf.fill(0);
            for y in 0..h {
                for x in 0..64 {
                    let c = if on == 0 {
                        7
                    } else if y == 0 || y == h - 1 {
                        9
                    } else {
                        8
                    };
                    put8(&mut buf, 64, x, y, c);
                }
            }
            if let Some(g) = *bar {
                copy_to_vram(g, &buf);
            }
        }
    }
    drop(sp);
    // Show the cleared screens (main may draw a message before the scene loop starts).
    display::present();
}

pub fn sprites_hide_all() {
    Oam::sub().clear(0, 128);
    Oam::main().clear(0, 128);
}

#[allow(clippy::too_many_arguments)]
fn set(
    o: Oam,
    id: i32,
    x: i32,
    y: i32,
    priority: u8,
    palette: u8,
    size: sys::SpriteSize,
    format: sys::SpriteColorFormat,
    gfx: Option<NonNull<u16>>,
    hide: bool,
) {
    let Some(gfx) = gfx else { return };
    let mut s = Sprite::new(x, y, size, format, gfx);
    s.priority = priority;
    s.palette = palette;
    s.hide = hide;
    o.set(id as u8, &s);
}

/// Placeholder Pokémon icon (original art), coloured by type; egg and shiny variants.
pub fn icon(id: i32, x: i32, y: i32, ty: u8, egg: bool, shiny: bool) {
    let sp = SPRITES.borrow_mut();
    let gfx = if egg {
        sp.egg
    } else {
        sp.icon[usize::from(ty).min(TYPES - 1)]
    };
    let c256 = sys::SpriteColorFormat_256Color;
    set(
        Oam::sub(),
        id,
        x,
        y,
        2,
        0,
        sys::SpriteSize_32x32,
        c256,
        gfx,
        false,
    );
    set(
        Oam::sub(),
        id + 1,
        x + 23,
        y + 1,
        1,
        0,
        sys::SpriteSize_8x8,
        c256,
        sp.star,
        !shiny || egg,
    );
}

const ROM_PAL_BASE: usize = 64; // ROM icon palettes take 64.. of the 256-colour sprite palette

/// Sets colours [first, first + 15] of a screen's sprite palette from colours 1..15 of `colors`.
fn sprite_colors(screen: usize, first: usize, colors: &[u16]) {
    display::with_sprite_palette(screen, |pal| {
        pal[first + 1..first + 16].copy_from_slice(&colors[1..16])
    });
}

/// 4 bpp (low nibble first) tiles to 8 bpp on palette colours base + 1.., same 1D tile order.
fn to_8bpp(g: Gfx, tiles: &[u8], base: usize) {
    display::with_gfx(g, |ram| {
        for (r, &t) in ram.iter_mut().zip(&tiles[..512]) {
            let (lo, hi) = (usize::from(t & 15), usize::from(t >> 4));
            let lo = if lo != 0 { base + lo } else { 0 };
            let hi = if hi != 0 { base + hi } else { 0 };
            *r = (lo | hi << 8) as u16;
        }
    });
}

/// ROM icon palettes (16 BGR555 colours each, at most 4).
pub fn set_icon_palettes(pal: &[[u16; 16]]) {
    for (p, colors) in pal.iter().enumerate() {
        sprite_colors(1, ROM_PAL_BASE + 16 * p, colors);
    }
}

/// A box icon from the ROM: 4 bpp tiles (1D 32x32) drawn with ROM palette `palette`; shiny star.
pub fn icon_rom(id: i32, x: i32, y: i32, tiles: &[u8], palette: u8, shiny: bool) {
    if !(0..128).contains(&id) {
        return;
    }
    let c256 = sys::SpriteColorFormat_256Color;
    let mut sp = SPRITES.borrow_mut();
    if sp.rom_slot[id as usize].is_none() {
        sp.rom_slot[id as usize] = display::sprite_gfx(Oam::sub(), sys::SpriteSize_32x32, c256);
    }
    let Some(g) = sp.rom_slot[id as usize] else {
        return;
    };
    to_8bpp(g, tiles, ROM_PAL_BASE + 16 * usize::from(palette & 3));
    set(
        Oam::sub(),
        id,
        x,
        y,
        2,
        0,
        sys::SpriteSize_32x32,
        c256,
        Some(display::gfx_vram(g)),
        false,
    );
    set(
        Oam::sub(),
        id + 1,
        x + 23,
        y + 1,
        1,
        0,
        sys::SpriteSize_8x8,
        c256,
        sp.star,
        !shiny,
    );
}

/// A held item's icon from the ROM (4 bpp 1D 32x32 tiles, its own 16 colours) at x, y on the top
/// or bottom screen; one per screen. `None` hides it.
pub fn item_icon(on_top: bool, x: i32, y: i32, icon: Option<(&[u8], &[u16])>) {
    // The last OAM entry of each screen and colours 128..143 of its sprite palette.
    const ID: i32 = 127;
    const BASE: usize = 128;
    let o = if on_top { Oam::main() } else { Oam::sub() };
    let Some((tiles, colors)) = icon else {
        o.clear(ID as u8, 1);
        return;
    };
    let c256 = sys::SpriteColorFormat_256Color;
    let k = if on_top { 0 } else { 1 };
    let g = {
        let mut sp = SPRITES.borrow_mut();
        if sp.item[k].is_none() {
            sp.item[k] = display::sprite_gfx(o, sys::SpriteSize_32x32, c256);
        }
        sp.item[k]
    };
    let Some(g) = g else { return };
    sprite_colors(if on_top { 0 } else { 1 }, BASE, colors);
    to_8bpp(g, tiles, BASE);
    // Priority 0: above the row highlight bar, which crosses the icon on the Item row.
    set(
        o,
        ID,
        x,
        y,
        0,
        0,
        sys::SpriteSize_32x32,
        c256,
        Some(display::gfx_vram(g)),
        false,
    );
}

// Top screen OAM: small icons 110.., the Pokémon sprite 120-123, the item icon 127 (behind the
// sprite: lower IDs draw on top). Palette banks: 8 item icon, 9 sprite, 10.. small icons.
const SMALL_ID: i32 = 110;
const SPRITE_ID: i32 = 120;
const SPRITE_BANK: u8 = 9;
const SMALL_BANK: u8 = 10;
/// Slots for small_icon().
pub const SMALL_ICONS: usize = 6;

/// A Pokémon's front sprite on the top screen (96x96: 4608 tile bytes in 1D blocks 64x64, 32x64,
/// 64x32, 32x32, and 16 colours); `None` hides it. Drawn over the item icon (a ball behind it).
pub fn pokemon_sprite(x: i32, y: i32, sprite: Option<(&[u8], &[u16])>) {
    const SIZE: [sys::SpriteSize; 4] = [
        sys::SpriteSize_64x64,
        sys::SpriteSize_32x64,
        sys::SpriteSize_64x32,
        sys::SpriteSize_32x32,
    ];
    const DX: [i32; 4] = [0, 64, 0, 64];
    const DY: [i32; 4] = [0, 0, 64, 64];
    const BYTES: [usize; 4] = [2048, 1024, 1024, 512];
    let Some((tiles, colors)) = sprite else {
        Oam::main().clear(SPRITE_ID as u8, 4);
        return;
    };
    let c16 = sys::SpriteColorFormat_16Color;
    sprite_colors(0, 16 * usize::from(SPRITE_BANK), colors);
    let mut off = 0;
    for b in 0..4 {
        let g = {
            let mut sp = SPRITES.borrow_mut();
            if sp.pokemon[b].is_none() {
                sp.pokemon[b] = display::sprite_gfx(Oam::main(), SIZE[b], c16);
            }
            sp.pokemon[b]
        };
        let Some(g) = g else { return };
        display::with_gfx(g, |ram| copy16(ram, &tiles[off..off + BYTES[b]]));
        off += BYTES[b];
        let vram = Some(display::gfx_vram(g));
        set(
            Oam::main(),
            SPRITE_ID + b as i32,
            x + DX[b],
            y + DY[b],
            2,
            SPRITE_BANK,
            SIZE[b],
            c16,
            vram,
            false,
        );
    }
}

/// verity's mark (48x48 inside a 64x64 sprite) on the top screen, in the Pokémon sprite's slot; a
/// scene change hides it.
pub fn logo(x: i32, y: i32) {
    let c16 = sys::SpriteColorFormat_16Color;
    let g = {
        let mut sp = SPRITES.borrow_mut();
        if sp.logo.is_none() {
            let Some(g) = display::sprite_gfx(Oam::main(), sys::SpriteSize_64x64, c16) else {
                return;
            };
            display::with_gfx(g, |ram| copy16(ram, &crate::logo::TILES));
            sp.logo = Some(g);
        }
        sp.logo
    };
    let Some(g) = g else { return };
    sprite_colors(0, 16 * usize::from(SPRITE_BANK), &crate::logo::COLORS);
    // The Pokémon sprite's four blocks share this slot; only the first is the logo.
    Oam::main().clear(SPRITE_ID as u8 + 1, 3);
    let vram = Some(display::gfx_vram(g));
    set(
        Oam::main(),
        SPRITE_ID,
        x,
        y,
        2,
        SPRITE_BANK,
        sys::SpriteSize_64x64,
        c16,
        vram,
        false,
    );
}

/// A 32x16 icon (type or move category: 256 tile bytes, 16 colours) on the top screen in one of
/// SMALL_ICONS slots; `None` hides the slot.
pub fn small_icon(slot: usize, x: i32, y: i32, icon: Option<(&[u8], &[u16])>) {
    if slot >= SMALL_ICONS {
        return;
    }
    let Some((tiles, colors)) = icon else {
        Oam::main().clear((SMALL_ID + slot as i32) as u8, 1);
        return;
    };
    let c16 = sys::SpriteColorFormat_16Color;
    let g = {
        let mut sp = SPRITES.borrow_mut();
        if sp.small[slot].is_none() {
            sp.small[slot] = display::sprite_gfx(Oam::main(), sys::SpriteSize_32x16, c16);
        }
        sp.small[slot]
    };
    let Some(g) = g else { return };
    let bank = SMALL_BANK + slot as u8;
    sprite_colors(0, 16 * usize::from(bank), colors);
    display::with_gfx(g, |ram| copy16(ram, &tiles[..256]));
    let vram = Some(display::gfx_vram(g));
    set(
        Oam::main(),
        SMALL_ID + slot as i32,
        x,
        y,
        1,
        bank,
        sys::SpriteSize_32x16,
        c16,
        vram,
        false,
    );
}

/// Hides the sprite and the small icons.
pub fn top_art_hide() {
    Oam::main().clear(SMALL_ID as u8, SMALL_ICONS as u8);
    Oam::main().clear(SPRITE_ID as u8, 4);
}

/// Shapes for shape(): the six box markings, then these.
pub const SHINY: i32 = 6;
pub const MALE: i32 = 7;
pub const FEMALE: i32 = 8;

/// A 7x7 shape drawn into the text layer: the six box markings (circle, triangle, square, heart,
/// star, diamond), SHINY (the shiny star), MALE or FEMALE. Tone 0 ink, 1 muted (an unset mark),
/// 2 gold, 3 blue, 4 red.
pub fn shape(c: Surface, x: i32, y: i32, kind: i32, tone: i32) {
    // 7 rows of 7 bits (bit 6 is the left column).
    const K: [[u8; 7]; 9] = [
        [0x1C, 0x3E, 0x7F, 0x7F, 0x7F, 0x3E, 0x1C], // circle
        [0x08, 0x1C, 0x1C, 0x3E, 0x3E, 0x7F, 0x7F], // triangle
        [0x00, 0x7E, 0x7E, 0x7E, 0x7E, 0x7E, 0x7E], // square
        [0x36, 0x7F, 0x7F, 0x7F, 0x3E, 0x1C, 0x08], // heart
        [0x08, 0x08, 0x7F, 0x3E, 0x1C, 0x36, 0x22], // star
        [0x08, 0x1C, 0x3E, 0x7F, 0x3E, 0x1C, 0x08], // diamond
        [0x08, 0x08, 0x7F, 0x3E, 0x1C, 0x36, 0x22], // shiny star
        [0x07, 0x03, 0x3A, 0x44, 0x44, 0x44, 0x38], // male
        [0x1C, 0x22, 0x22, 0x1C, 0x08, 0x3E, 0x08], // female
    ];
    if !(0..=FEMALE).contains(&kind) {
        return;
    }
    let muted = IS_MUTED.get();
    IS_MUTED.set(tone == 1);
    const FIXED: [u8; 5] = [0, 0, C_FISHY, C_LEGAL, C_INVALID];
    INK.set(if (2..=4).contains(&tone) {
        FIXED[tone as usize]
    } else {
        ink_for(c, x, y)
    });
    IS_MUTED.set(muted);
    for (r, bits) in K[kind as usize].iter().enumerate() {
        for col in 0..7 {
            if bits & (0x40 >> col) != 0 {
                plot(c, x + col, y + r as i32);
            }
        }
    }
}

/// Highlight bar (`height` px high, full width) on the bottom screen.
pub fn bar(first: i32, y: i32, visible: bool, height: i32) {
    // Over a panel the highlight is peach with an orange edge (dark text); elsewhere navy blue.
    let row = (y + height / 2) / 8;
    let on_panel = (0..24).contains(&row) && kind_at(1, (row * 32 + 4) as usize) == K_PANEL;
    let k = if height >= 16 {
        2
    } else if height >= 12 {
        1
    } else {
        0
    };
    let g = SPRITES.borrow_mut().bars[k][usize::from(on_panel)];
    for i in 0..4 {
        let c256 = sys::SpriteColorFormat_256Color;
        set(
            Oam::sub(),
            first + i,
            i * 64,
            y,
            1,
            0,
            sys::SpriteSize_64x32,
            c256,
            g,
            !visible,
        );
    }
}
