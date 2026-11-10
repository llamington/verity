//! The ROM font: builds verity's font cache from the user's own ROM (read through a C callback)
//! and hands glyphs to the DS text renderer. See `docs/research/rom-font.md`.

use core::ffi::c_void;
use core::slice;

use verity::text::{value_to_char_g4, TERMINATOR};
use verity_rom::font::{self, CachedFont, Keys};
use verity_rom::icons::{self, CachedIcons, IconKey};
use verity_rom::moves::{self, CachedMoves};
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

/// Builds the icon cache file for the ROM read by `src`.
pub fn build_icon_cache<S: Source>(src: S) -> Result<alloc::vec::Vec<u8>, Error> {
    let mut rom = Rom::open(src)?;
    let set = icons::load(&mut rom)?;
    Ok(set.to_cache(rom.game_code()))
}

/// C ABI: builds the box icon cache from a ROM read through `read`, like
/// `verity_rom_build_font_cache` (free the result with `verity_rom_free`). Null on failure,
/// including a ROM whose icons verity can't place.
///
/// # Safety
///
/// As for `verity_rom_build_font_cache`.
#[no_mangle]
pub unsafe extern "C" fn verity_rom_build_icon_cache(
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
    match build_icon_cache(Callback { read, ctx }) {
        Ok(v) => {
            let b = v.into_boxed_slice();
            unsafe { *out_len = b.len() };
            alloc::boxed::Box::into_raw(b) as *mut u8
        }
        Err(_) => core::ptr::null_mut(),
    }
}

/// C ABI: how many bytes from the start of an icon cache file the DS keeps in memory (the head:
/// palettes and index), from its first 16 bytes; 0 if it isn't an icon cache.
///
/// # Safety
///
/// `p` is null or valid for `len` bytes.
#[no_mangle]
pub unsafe extern "C" fn verity_icons_head_len(p: *const u8, len: usize) -> usize {
    if p.is_null() {
        return 0;
    }
    CachedIcons::head_len(unsafe { slice::from_raw_parts(p, len) }).unwrap_or(0)
}

fn cached_icons(p: *const u8, len: usize) -> Option<CachedIcons<'static>> {
    if p.is_null() {
        return None;
    }
    // SAFETY: callers vouch that `p` is valid for `len` bytes while the icons are in use.
    CachedIcons::parse(unsafe { slice::from_raw_parts(p, len) }).ok()
}

/// C ABI: palette `index` (16 BGR555 colours) of the icon cache head; false if there is none.
///
/// # Safety
///
/// `head` is null or valid for `len` bytes; `out` is null or has room for 16 `u16`s.
#[no_mangle]
pub unsafe extern "C" fn verity_icons_palette(
    head: *const u8,
    len: usize,
    index: u8,
    out: *mut u16,
) -> bool {
    let Some(pal) = cached_icons(head, len).and_then(|c| c.palette(index)) else {
        return false;
    };
    if out.is_null() {
        return false;
    }
    unsafe { core::ptr::copy_nonoverlapping(pal.as_ptr(), out, 16) };
    true
}

/// C ABI: the number of palettes in the icon cache head (0 if it isn't one).
///
/// # Safety
///
/// `head` is null or valid for `len` bytes.
#[no_mangle]
pub unsafe extern "C" fn verity_icons_palette_count(head: *const u8, len: usize) -> u8 {
    cached_icons(head, len).map_or(0, |c| c.palette_count)
}

/// C ABI: the icon of a Pokémon (`female` and `egg` 0/1). Writes its palette and the file offset of
/// its 512 bytes of 4 bpp tiles (1D, 32x32). False if the cache has no icon for it.
///
/// # Safety
///
/// `head` is null or valid for `len` bytes; `palette` and `tiles_at` are null or writable.
#[no_mangle]
pub unsafe extern "C" fn verity_icons_find(
    head: *const u8,
    len: usize,
    species: u16,
    form: u8,
    female: u8,
    egg: u8,
    palette: *mut u8,
    tiles_at: *mut u32,
) -> bool {
    if palette.is_null() || tiles_at.is_null() {
        return false;
    }
    let key = IconKey {
        species,
        form,
        female: female != 0,
        egg: egg != 0,
    };
    let Some(r) = cached_icons(head, len).and_then(|c| c.find(key)) else {
        return false;
    };
    unsafe {
        *palette = r.palette;
        *tiles_at = r.tiles_at;
    }
    true
}

/// Builds the move cache file (category, power, accuracy) for the ROM read by `src`.
pub fn build_moves_cache<S: Source>(src: S) -> Result<alloc::vec::Vec<u8>, Error> {
    let mut rom = Rom::open(src)?;
    Ok(moves::load(&mut rom)?.to_cache(rom.game_code()))
}

/// C ABI: builds a move cache from a ROM, like `verity_rom_build_font_cache` (free the result with
/// `verity_rom_free`). The cache serves saves of the ROM's generation only.
///
/// # Safety
///
/// As for `verity_rom_build_font_cache`.
#[no_mangle]
pub unsafe extern "C" fn verity_rom_build_moves_cache(
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
    match build_moves_cache(Callback { read, ctx }) {
        Ok(v) => {
            let b = v.into_boxed_slice();
            unsafe { *out_len = b.len() };
            alloc::boxed::Box::into_raw(b) as *mut u8
        }
        Err(_) => core::ptr::null_mut(),
    }
}

/// C ABI: the generation a move cache serves (4 or 5), or 0 if it isn't one.
///
/// # Safety
///
/// `p` is null or valid for `len` bytes.
#[no_mangle]
pub unsafe extern "C" fn verity_moves_generation(p: *const u8, len: usize) -> u8 {
    if p.is_null() {
        return 0;
    }
    CachedMoves::parse(unsafe { slice::from_raw_parts(p, len) }).map_or(0, |c| c.generation)
}

/// What the move pickers show. `type_` and `pp` come from verity's data (0xFF when the move
/// data isn't loaded); `category` (0 physical, 1 special, 2 status), `power` and `accuracy` from
/// the ROM's move table, with `from_rom` 0 when there is no cache for this generation.
#[repr(C)]
pub struct VerityMoveInfo {
    pub type_: u8,
    pub pp: u8,
    pub from_rom: u8,
    pub category: u8,
    pub power: u8,
    pub accuracy: u8,
}

/// C ABI: info about move `mv` in a Gen `generation` save. `rom_moves` is the move cache for
/// that generation, or null. False if `mv` is out of range for the generation.
///
/// # Safety
///
/// `rom_moves` is null or valid for `len` bytes; `out` is writable.
#[no_mangle]
pub unsafe extern "C" fn verity_move_info(
    generation: u8,
    mv: u16,
    rom_moves: *const u8,
    len: usize,
    out: *mut VerityMoveInfo,
) -> bool {
    if out.is_null() {
        return false;
    }
    let kind = if generation == 4 {
        crate::ui::BlobKind::Moves4
    } else {
        crate::ui::BlobKind::Moves5
    };
    let data = crate::ui::blob(kind).and_then(|b| verity::data::MoveData::load(b).ok());
    if data.is_some_and(|d| mv > d.max_move()) {
        return false;
    }
    let mut info = VerityMoveInfo {
        type_: data.map_or(0xFF, |d| d.move_type(mv)),
        pp: data.map_or(0xFF, |d| d.base_pp(mv)),
        from_rom: 0,
        category: 0,
        power: 0,
        accuracy: 0,
    };
    if !rom_moves.is_null() {
        let cache = CachedMoves::parse(unsafe { slice::from_raw_parts(rom_moves, len) }).ok();
        if let Some(m) = cache
            .filter(|c| c.generation == generation)
            .and_then(|c| c.get(mv))
        {
            info.from_rom = 1;
            info.category = m.category as u8;
            info.power = m.power;
            info.accuracy = m.accuracy;
        }
    }
    unsafe { *out = info };
    true
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
    fn icon_cache_through_the_c_abi() {
        let Some(mut rom) = rom("black2.nds") else {
            return;
        };
        let mut len = 0;
        let p = unsafe {
            verity_rom_build_icon_cache(
                Some(read_slice),
                &mut rom as *mut _ as *mut c_void,
                &mut len,
            )
        };
        assert!(!p.is_null());
        let v = unsafe { slice::from_raw_parts(p, len) }.to_vec();
        unsafe { verity_rom_free(p, len) };
        let head = unsafe { verity_icons_head_len(v.as_ptr(), 16) };
        assert!(head > 0 && head < v.len());
        assert_eq!(unsafe { verity_icons_palette_count(v.as_ptr(), head) }, 3);
        let mut pal = [0u16; 16];
        assert!(unsafe { verity_icons_palette(v.as_ptr(), head, 2, pal.as_mut_ptr()) });
        assert!(!unsafe { verity_icons_palette(v.as_ptr(), head, 3, pal.as_mut_ptr()) });
        let (mut pi, mut at) = (0u8, 0u32);
        assert!(unsafe { verity_icons_find(v.as_ptr(), head, 25, 0, 0, 0, &mut pi, &mut at) });
        assert!(at as usize + 512 <= v.len());
        assert!(!unsafe { verity_icons_find(v.as_ptr(), head, 2000, 0, 0, 0, &mut pi, &mut at) });
    }

    #[test]
    fn move_info_uses_the_matching_generation_only() {
        let Some(mut rom) = rom("platinum.nds") else {
            return;
        };
        let mut len = 0;
        let p = unsafe {
            verity_rom_build_moves_cache(
                Some(read_slice),
                &mut rom as *mut _ as *mut c_void,
                &mut len,
            )
        };
        assert!(!p.is_null());
        let v = unsafe { slice::from_raw_parts(p, len) }.to_vec();
        unsafe { verity_rom_free(p, len) };
        assert_eq!(unsafe { verity_moves_generation(v.as_ptr(), v.len()) }, 4);
        let mut out = VerityMoveInfo {
            type_: 0,
            pp: 0,
            from_rom: 9,
            category: 9,
            power: 0,
            accuracy: 0,
        };
        assert!(unsafe { verity_move_info(4, 33, v.as_ptr(), v.len(), &mut out) });
        assert_eq!(
            (out.from_rom, out.category, out.power, out.accuracy),
            (1, 0, 35, 95),
            "Gen 4 Tackle"
        );
        // A Gen 4 cache says nothing about a Gen 5 save.
        assert!(unsafe { verity_move_info(5, 33, v.as_ptr(), v.len(), &mut out) });
        assert_eq!(out.from_rom, 0);
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
