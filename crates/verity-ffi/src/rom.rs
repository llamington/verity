//! The ROM font: builds verity's font cache from the user's own ROM (read through a C callback)
//! and hands glyphs to the DS text renderer. See `docs/research/rom-font.md`.

use core::ffi::c_void;
use core::slice;

use verity::text::{value_to_char_g4, TERMINATOR};
use verity_rom::font::{self, CachedFont, Keys};
use verity_rom::nds::{Game, Rom};
use verity_rom::{Error, Source};

/// Reads `len` bytes at `offset` of the ROM file into `buf`; false on failure.
pub type ReadFn =
    unsafe extern "C" fn(ctx: *mut c_void, offset: u32, buf: *mut u8, len: u32) -> bool;

struct Callback {
    read: ReadFn,
    ctx: *mut c_void,
}

impl Source for Callback {
    fn read_at(&mut self, offset: u32, buf: &mut [u8]) -> Result<(), Error> {
        let len = u32::try_from(buf.len()).map_err(|_| Error::Io)?;
        // SAFETY: the caller of `verity_rom_build_font_cache` vouches for `read` and `ctx`.
        if unsafe { (self.read)(self.ctx, offset, buf.as_mut_ptr(), len) } {
            Ok(())
        } else {
            Err(Error::Io)
        }
    }
}

/// Gen 4 glyphs are 16 px cells with the stroke in rows 2..=11.
const GEN4_TOP: u8 = 2;
const GEN4_ROWS: u8 = 10;
/// The DS UI's line height: a taller glyph is cut by the next line.
const LINE: u8 = 8;

/// How good a ROM's font is for verity (higher is better; 0 = not a supported game). Any Gen 5
/// ROM covers kana, kanji and Hangul; a Korean Gen 4 ROM may hold Hangul; other Gen 4 ROMs kana.
pub fn font_rank(code: [u8; 4]) -> u8 {
    match Game::from_code(code) {
        Some(g) if g.generation() == 5 => 3,
        Some(_) if code[3] == b'K' => 2,
        Some(_) => 1,
        None => 0,
    }
}

/// Builds the font cache file for the ROM read by `src`.
pub fn build_font_cache<S: Source>(src: S) -> Result<alloc::vec::Vec<u8>, Error> {
    let mut rom = Rom::open(src)?;
    let f = font::load(&mut rom)?;
    let f = match f.keys_are {
        Keys::Unicode => f,
        Keys::Gen4Codes => f
            .remap(Keys::Unicode, |k| {
                Some(value_to_char_g4(k)).filter(|&c| c != TERMINATOR && c != 0)
            })
            .without_shadow()
            .crop_rows(GEN4_TOP, GEN4_ROWS),
    }
    .fit_rows(LINE);
    Ok(f.to_cache(rom.game_code()))
}

/// C ABI: the font rank of a 4-letter game code (see `font_rank`); 0 for null.
///
/// # Safety
///
/// `code` is null or points to 4 readable bytes.
#[no_mangle]
pub unsafe extern "C" fn verity_rom_font_rank(code: *const u8) -> u8 {
    if code.is_null() {
        return 0;
    }
    let c = unsafe { slice::from_raw_parts(code, 4) };
    font_rank([c[0], c[1], c[2], c[3]])
}

/// C ABI: builds the font cache from a ROM read through `read`. Returns a buffer of `*out_len`
/// bytes to write to the SD card and release with `verity_rom_free`, or null on failure.
///
/// # Safety
///
/// `read` must be safe to call with `ctx` for the whole call; `out_len` must be writable.
#[no_mangle]
pub unsafe extern "C" fn verity_rom_build_font_cache(
    read: Option<ReadFn>,
    ctx: *mut c_void,
    out_len: *mut usize,
) -> *mut u8 {
    let Some(read) = read else {
        return core::ptr::null_mut();
    };
    if out_len.is_null() {
        return core::ptr::null_mut();
    }
    match build_font_cache(Callback { read, ctx }) {
        Ok(v) => {
            let b = v.into_boxed_slice();
            unsafe { *out_len = b.len() };
            alloc::boxed::Box::into_raw(b) as *mut u8
        }
        Err(_) => core::ptr::null_mut(),
    }
}

/// C ABI: frees a buffer from `verity_rom_build_font_cache`.
///
/// # Safety
///
/// `p` and `len` come from one `verity_rom_build_font_cache` call and are freed once.
#[no_mangle]
pub unsafe extern "C" fn verity_rom_free(p: *mut u8, len: usize) {
    if !p.is_null() {
        drop(unsafe { alloc::boxed::Box::from_raw(core::ptr::slice_from_raw_parts_mut(p, len)) });
    }
}

/// A glyph for the DS renderer: bit `x` of `rows[y]` is an ink pixel (stroke or shadow; the games
/// draw both, and stroke alone is broken). Ink starts at column 0 and is `width` wide.
#[repr(C)]
pub struct VerityGlyph {
    pub left: u8,
    pub width: u8,
    pub advance: u8,
    pub height: u8,
    pub rows: [u16; 16],
}

fn cached(p: *const u8, len: usize) -> Option<CachedFont<'static>> {
    if p.is_null() {
        return None;
    }
    // SAFETY: callers vouch that `p` is valid for `len` bytes while the font is in use.
    let b = unsafe { slice::from_raw_parts(p, len) };
    CachedFont::parse(b)
        .ok()
        .filter(|f| f.cell_w <= 16 && f.cell_h <= 16 && f.keys_are == Keys::Unicode)
}

/// C ABI: true if the buffer is a usable font cache (call once after reading the file).
///
/// # Safety
///
/// `p` is null or valid for `len` bytes.
#[no_mangle]
pub unsafe extern "C" fn verity_font_check(p: *const u8, len: usize) -> bool {
    cached(p, len).is_some_and(|f| f.validate().is_ok())
}

/// C ABI: looks up the glyph for UTF-16 code unit `c`; false if the font has none.
///
/// # Safety
///
/// `p` is null or valid for `len` bytes; `out` is null or writable.
#[no_mangle]
pub unsafe extern "C" fn verity_font_glyph(
    p: *const u8,
    len: usize,
    c: u16,
    out: *mut VerityGlyph,
) -> bool {
    let (Some(f), false) = (cached(p, len), out.is_null()) else {
        return false;
    };
    let Some(g) = f.glyph(c) else { return false };
    let mut rows = [0u16; 16];
    for (y, row) in rows.iter_mut().enumerate().take(usize::from(g.h)) {
        for x in 0..g.w {
            if g.pixel(x, y as u8) != 0 {
                *row |= 1 << x;
            }
        }
    }
    let m = g.metrics;
    unsafe {
        *out = VerityGlyph {
            left: m.left,
            width: m.width,
            advance: m.advance,
            height: g.h,
            rows,
        };
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rom(name: &str) -> Option<std::vec::Vec<u8>> {
        let p = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../roms")
            .join(name);
        std::fs::read(p).ok()
    }

    unsafe extern "C" fn read_slice(ctx: *mut c_void, offset: u32, buf: *mut u8, len: u32) -> bool {
        let rom = unsafe { &*(ctx as *const std::vec::Vec<u8>) };
        let Some(src) = rom.get(offset as usize..offset as usize + len as usize) else {
            return false;
        };
        unsafe { core::ptr::copy_nonoverlapping(src.as_ptr(), buf, src.len()) };
        true
    }

    fn build(name: &str) -> Option<std::vec::Vec<u8>> {
        let mut rom = rom(name)?;
        let mut len = 0;
        let p = unsafe {
            verity_rom_build_font_cache(
                Some(read_slice),
                &mut rom as *mut _ as *mut c_void,
                &mut len,
            )
        };
        assert!(!p.is_null(), "{name}");
        let v = unsafe { slice::from_raw_parts(p, len) }.to_vec();
        unsafe { verity_rom_free(p, len) };
        assert!(unsafe { verity_font_check(v.as_ptr(), v.len()) });
        Some(v)
    }

    fn glyph(cache: &[u8], c: char) -> Option<VerityGlyph> {
        let mut g = VerityGlyph {
            left: 0,
            width: 0,
            advance: 0,
            height: 0,
            rows: [0; 16],
        };
        unsafe { verity_font_glyph(cache.as_ptr(), cache.len(), c as u16, &mut g) }.then_some(g)
    }

    #[test]
    fn ranks() {
        assert_eq!(font_rank(*b"IREO"), 3);
        assert_eq!(font_rank(*b"IPKK"), 2);
        assert_eq!(font_rank(*b"CPUE"), 1);
        assert_eq!(font_rank(*b"####"), 0);
        assert_eq!(unsafe { verity_rom_font_rank(core::ptr::null()) }, 0);
    }

    #[test]
    fn gen4_cache_is_unicode_kana() {
        let Some(c) = build("platinum.nds") else {
            return;
        };
        let a = glyph(&c, 'あ').expect("hiragana");
        assert_eq!(a.height, LINE);
        assert!(a.rows.iter().any(|&r| r != 0));
        assert!(glyph(&c, 'A').is_some() && glyph(&c, '한').is_none());
    }

    #[test]
    fn gen5_cache_covers_cjk() {
        let Some(c) = build("black2.nds") else { return };
        assert!(c.len() < 200 * 1024);
        for ch in "ポ漢한é".chars() {
            let g = glyph(&c, ch).unwrap_or_else(|| panic!("{ch}"));
            assert!(g.rows.iter().any(|&r| r != 0) && g.height == LINE, "{ch}");
        }
        assert!(glyph(&c, '\u{FFFE}').is_none());
        // A damaged cache is refused.
        assert!(!unsafe { verity_font_check(c.as_ptr(), c.len() - 1) });
    }

    #[test]
    fn unreadable_rom_fails_cleanly() {
        let mut empty = std::vec::Vec::<u8>::new();
        let mut len = 0;
        let p = unsafe {
            verity_rom_build_font_cache(
                Some(read_slice),
                &mut empty as *mut _ as *mut c_void,
                &mut len,
            )
        };
        assert!(p.is_null());
    }
}
