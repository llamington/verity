//! The ROM font: builds verity's font cache from the user's own ROM (read through a C callback)
//! and hands glyphs to the DS text renderer. See `docs/research/rom-font.md`.

use core::ffi::c_void;
use core::slice;

use verity::text::{TERMINATOR, value_to_char_g4};
use verity_rom::font::{self, CachedFont, Keys};
use verity_rom::icons::{self, CachedIcons, IconKey};
use verity_rom::items::{self, CachedItems};
use verity_rom::moves::{self, CachedMoves};
use verity_rom::nds::{Game, Rom};
use verity_rom::ribbons::{self, CachedRibbons};
use verity_rom::sprites::{self, CachedSprites};
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
#[unsafe(no_mangle)]
pub unsafe extern "C" fn verity_rom_font_rank(code: *const u8) -> u8 {
    if code.is_null() {
        return 0;
    }
    // SAFETY: `code` is non-null (checked above) and valid for `4` elements (this function's
    // contract).
    let c = unsafe { slice::from_raw_parts(code, 4) };
    font_rank([c[0], c[1], c[2], c[3]])
}

/// C ABI: builds the font cache from a ROM read through `read`. Returns a buffer of `*out_len`
/// bytes to write to the SD card and release with `verity_rom_free`, or null on failure.
///
/// # Safety
///
/// `read` must be safe to call with `ctx` for the whole call; `out_len` must be writable.
#[unsafe(no_mangle)]
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
            // SAFETY: `out_len` is non-null (checked above) and points to a writable value (this
            // function's contract).
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
#[unsafe(no_mangle)]
pub unsafe extern "C" fn verity_rom_free(p: *mut u8, len: usize) {
    if !p.is_null() {
        // SAFETY: `p` and `len` describe a boxed slice from one cache build, freed only here and
        // only once (this function's contract).
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
#[unsafe(no_mangle)]
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
            // SAFETY: `out_len` is non-null (checked above) and points to a writable value (this
            // function's contract).
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
#[unsafe(no_mangle)]
pub unsafe extern "C" fn verity_icons_head_len(p: *const u8, len: usize) -> usize {
    if p.is_null() {
        return 0;
    }
    // SAFETY: `p` is non-null (checked above) and valid for `len` elements (this function's
    // contract).
    CachedIcons::head_len(unsafe { slice::from_raw_parts(p, len) }).unwrap_or(0)
}

/// # Safety
///
/// `p` must be null or valid for `len` bytes, unchanged while the parsed icons are in use.
unsafe fn cached_icons(p: *const u8, len: usize) -> Option<CachedIcons<'static>> {
    if p.is_null() {
        return None;
    }
    // SAFETY: `p` is non-null and valid for `len` bytes (this function's contract).
    CachedIcons::parse(unsafe { slice::from_raw_parts(p, len) }).ok()
}

/// C ABI: palette `index` (16 BGR555 colours) of the icon cache head; false if there is none.
///
/// # Safety
///
/// `head` is null or valid for `len` bytes; `out` is null or has room for 16 `u16`s.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn verity_icons_palette(
    head: *const u8,
    len: usize,
    index: u8,
    out: *mut u16,
) -> bool {
    // SAFETY: `head` is null or valid for `len` bytes (this function's contract).
    let Some(pal) = unsafe { cached_icons(head, len) }.and_then(|c| c.palette(index)) else {
        return false;
    };
    if out.is_null() {
        return false;
    }
    // SAFETY: `out` is non-null (checked above) and has room for 16 `u16`s (this function's
    // contract); `pal` is a separate `[u16; 16]`.
    unsafe { core::ptr::copy_nonoverlapping(pal.as_ptr(), out, 16) };
    true
}

/// C ABI: the number of palettes in the icon cache head (0 if it isn't one).
///
/// # Safety
///
/// `head` is null or valid for `len` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn verity_icons_palette_count(head: *const u8, len: usize) -> u8 {
    // SAFETY: `head` is null or valid for `len` bytes (this function's contract).
    (unsafe { cached_icons(head, len) }).map_or(0, |c| c.palette_count)
}

/// C ABI: the icon of a Pokémon (`female` and `egg` 0/1). Writes its palette and the file offset of
/// its 512 bytes of 4 bpp tiles (1D, 32x32). False if the cache has no icon for it.
///
/// # Safety
///
/// `head` is null or valid for `len` bytes; `palette` and `tiles_at` are null or writable.
#[unsafe(no_mangle)]
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
    // SAFETY: `head` is null or valid for `len` bytes (this function's contract).
    let Some(r) = unsafe { cached_icons(head, len) }.and_then(|c| c.find(key)) else {
        return false;
    };
    // SAFETY: `palette` and `tiles_at` are non-null (checked above) and writable (this
    // function's contract).
    unsafe {
        *palette = r.palette;
        *tiles_at = r.tiles_at;
    }
    true
}

/// Builds the ribbon image cache file for the ROM read by `src`.
pub fn build_ribbon_cache<S: Source>(src: S) -> Result<alloc::vec::Vec<u8>, Error> {
    let mut rom = Rom::open(src)?;
    Ok(ribbons::load(&mut rom)?.to_cache(rom.game_code()))
}

/// C ABI: builds the ribbon image cache from a ROM, like `verity_rom_build_font_cache` (free the
/// result with `verity_rom_free`). Null for a ROM verity can't read ribbons from (Diamond/Pearl).
///
/// # Safety
///
/// As for `verity_rom_build_font_cache`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn verity_rom_build_ribbon_cache(
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
    match build_ribbon_cache(Callback { read, ctx }) {
        Ok(v) => {
            let b = v.into_boxed_slice();
            // SAFETY: `out_len` is non-null (checked above) and points to a writable value (this
            // function's contract).
            unsafe { *out_len = b.len() };
            alloc::boxed::Box::into_raw(b) as *mut u8
        }
        Err(_) => core::ptr::null_mut(),
    }
}

/// C ABI: true if `p` is a ribbon cache.
///
/// # Safety
///
/// `p` is null or valid for `len` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn verity_ribbons_check(p: *const u8, len: usize) -> bool {
    // SAFETY: `p` is non-null (checked above) and valid for `len` elements (this function's
    // contract).
    !p.is_null() && CachedRibbons::parse(unsafe { slice::from_raw_parts(p, len) }).is_ok()
}

/// C ABI: the image of the ribbon at position `index` of verity's list (Sinnoh set, Hoenn set,
/// Sinnoh contests): 512 tile bytes (4 bpp, 1D 32x32) into `tiles` and 16 BGR555 colours into
/// `colors`. False if there is none.
///
/// # Safety
///
/// `cache` is null or valid for `len` bytes; `tiles` is null or writable for 512 bytes and
/// `colors` for 16 `u16`s.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn verity_ribbon_icon(
    cache: *const u8,
    len: usize,
    index: u8,
    tiles: *mut u8,
    colors: *mut u16,
) -> bool {
    if cache.is_null() || tiles.is_null() || colors.is_null() {
        return false;
    }
    // SAFETY: `cache` is non-null (checked above) and valid for `len` elements (this function's
    // contract).
    let c = CachedRibbons::parse(unsafe { slice::from_raw_parts(cache, len) }).ok();
    let Some((t, pal)) = c.and_then(|c| c.icon(usize::from(index))) else {
        return false;
    };
    // SAFETY: `tiles` and `colors` are non-null (checked above) with room for 512 bytes and 16
    // `u16`s (this function's contract); `t` is one 512-byte tile block of the cache and `pal` a
    // local array, so neither overlaps the output.
    unsafe {
        core::ptr::copy_nonoverlapping(t.as_ptr(), tiles, t.len());
        core::ptr::copy_nonoverlapping(pal.as_ptr(), colors, 16);
    }
    true
}

/// Appends `len` bytes to the file being written; false on failure.
pub type WriteFn = unsafe extern "C" fn(ctx: *mut c_void, buf: *const u8, len: u32) -> bool;

/// C ABI: writes the sprite cache of the ROM's generation (Pokémon front sprites, type and
/// category icons) through `write`, in order, as it reads the ROM; the cache is too big to build
/// in memory. Returns the number of sprites, 0 on failure (the file is then incomplete).
///
/// # Safety
///
/// As for `verity_rom_build_font_cache`; `write` is called with `wctx`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn verity_rom_build_sprite_cache(
    read: Option<ReadFn>,
    ctx: *mut c_void,
    write: Option<WriteFn>,
    wctx: *mut c_void,
) -> u32 {
    let (Some(read), Some(write)) = (read, write) else {
        return 0;
    };
    let Ok(mut rom) = Rom::open(Callback { read, ctx }) else {
        return 0;
    };
    let mut out = |b: &[u8]| -> Result<(), Error> {
        let len = u32::try_from(b.len()).map_err(|_| Error::Io)?;
        // SAFETY: the caller vouches for `write` and `wctx`; `b` is valid for `len` bytes.
        if unsafe { write(wctx, b.as_ptr(), len) } {
            Ok(())
        } else {
            Err(Error::Io)
        }
    };
    sprites::build(&mut rom, &mut out).map_or(0, |n| n as u32)
}

/// C ABI: from a sprite cache's last `VERITY_SPRITES_FOOTER` bytes and the file length, the file
/// offset of the tail the DS keeps in memory (icons, index, footer: about 20 KB); `u32::MAX` if
/// the file isn't a sprite cache.
///
/// # Safety
///
/// `footer` is null or valid for `len` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn verity_sprites_tail_start(
    footer: *const u8,
    len: usize,
    file_len: u32,
) -> u32 {
    if footer.is_null() {
        return u32::MAX;
    }
    // SAFETY: `footer` is non-null (checked above) and valid for `len` elements (this function's
    // contract).
    let f = unsafe { slice::from_raw_parts(footer, len) };
    CachedSprites::tail_start(f, file_len).unwrap_or(u32::MAX)
}

/// # Safety
///
/// `tail` must be null or valid for `len` bytes, unchanged while the parsed sprites are in use.
unsafe fn cached_sprites(
    tail: *const u8,
    len: usize,
    start: u32,
) -> Option<CachedSprites<'static>> {
    if tail.is_null() {
        return None;
    }
    // SAFETY: `tail` is non-null and valid for `len` bytes (this function's contract).
    CachedSprites::parse(unsafe { slice::from_raw_parts(tail, len) }, start).ok()
}

/// C ABI: the generation of a sprite cache tail read from file offset `start`, 0 if invalid.
///
/// # Safety
///
/// `tail` is null or valid for `len` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn verity_sprites_generation(tail: *const u8, len: usize, start: u32) -> u8 {
    // SAFETY: `tail` is null or valid for `len` bytes (this function's contract).
    (unsafe { cached_sprites(tail, len, start) }).map_or(0, |c| c.generation)
}

/// C ABI: where a Pokémon's sprite is in the cache file: 64 palette bytes at `palette_at` (normal
/// then shiny, 16 BGR555 colours each) and `tiles_len` bytes at `tiles_at` for
/// `verity_sprite_decode`. Falls back to the male's sprite, then the base form's.
///
/// # Safety
///
/// `tail` as for `verity_sprites_generation`; the outputs are null or writable.
#[unsafe(no_mangle)]
#[allow(clippy::too_many_arguments)]
pub unsafe extern "C" fn verity_sprites_find(
    tail: *const u8,
    len: usize,
    start: u32,
    species: u16,
    form: u8,
    female: bool,
    egg: bool,
    palette_at: *mut u32,
    tiles_at: *mut u32,
    tiles_len: *mut u32,
) -> bool {
    // SAFETY: `tail` is null or valid for `len` bytes (this function's contract).
    let Some(c) = (unsafe { cached_sprites(tail, len, start) }) else {
        return false;
    };
    let Some(r) = c.find(IconKey {
        species,
        form,
        female,
        egg,
    }) else {
        return false;
    };
    if palette_at.is_null() || tiles_at.is_null() || tiles_len.is_null() {
        return false;
    }
    // SAFETY: the three outputs are non-null (checked above) and writable (this function's
    // contract).
    unsafe {
        *palette_at = r.palette;
        *tiles_at = r.tiles;
        *tiles_len = r.tiles_len;
    }
    true
}

/// C ABI: decodes sprite data read from the cache into `out` (4608 bytes: 96x96, 4 bpp, in four
/// 1D blocks of 64x64, 32x64, 64x32 and 32x32).
///
/// # Safety
///
/// `blob` is null or valid for `len` bytes; `out` is null or writable for 4608 bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn verity_sprite_decode(blob: *const u8, len: usize, out: *mut u8) -> bool {
    if blob.is_null() || out.is_null() {
        return false;
    }
    // SAFETY: `blob` is non-null (checked above) and valid for `len` elements (this function's
    // contract).
    match sprites::decode(unsafe { slice::from_raw_parts(blob, len) }) {
        Ok(tiles) => {
            // SAFETY: `out` is non-null (checked above) and writable for 4608 bytes (this
            // function's contract); `tiles` is a local `[u8; SPRITE_BYTES]` (4608).
            unsafe { core::ptr::copy_nonoverlapping(tiles.as_ptr(), out, tiles.len()) };
            true
        }
        Err(_) => false,
    }
}

/// C ABI: a type icon (`kind` 0, type in verity's numbering) or move category icon (`kind` 1:
/// 0 physical, 1 special, 2 status) from the cache tail: 256 tile bytes (32x16, 4 bpp, tiles in
/// rows of four) and 16 BGR555 colours.
///
/// # Safety
///
/// `tail` as for `verity_sprites_generation`; `tiles` is null or writable for 256 bytes and
/// `colors` for 16 values.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn verity_sprites_icon(
    tail: *const u8,
    len: usize,
    start: u32,
    kind: u8,
    index: u8,
    tiles: *mut u8,
    colors: *mut u16,
) -> bool {
    // SAFETY: `tail` is null or valid for `len` bytes (this function's contract).
    let Some((t, c)) =
        unsafe { cached_sprites(tail, len, start) }.and_then(|s| s.icon(kind, index.into()))
    else {
        return false;
    };
    if tiles.is_null() || colors.is_null() {
        return false;
    }
    // SAFETY: `tiles` and `colors` are non-null (checked above) with room for 256 bytes and 16
    // `u16`s (this function's contract); `t` is a local `[u8; ICON_BYTES]` (256) and `c` a local
    // `[u16; 16]`.
    unsafe {
        core::ptr::copy_nonoverlapping(t.as_ptr(), tiles, t.len());
        core::ptr::copy_nonoverlapping(c.as_ptr(), colors, c.len());
    }
    true
}

/// Builds the held item icon cache file for the ROM read by `src`.
pub fn build_items_cache<S: Source>(src: S) -> Result<alloc::vec::Vec<u8>, Error> {
    let mut rom = Rom::open(src)?;
    Ok(items::load(&mut rom)?.to_cache(rom.game_code()))
}

/// C ABI: builds an item icon cache from a ROM, like `verity_rom_build_font_cache` (free the
/// result with `verity_rom_free`). The cache serves saves of the ROM's generation only.
///
/// # Safety
///
/// As for `verity_rom_build_font_cache`.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn verity_rom_build_items_cache(
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
    match build_items_cache(Callback { read, ctx }) {
        Ok(v) => {
            let b = v.into_boxed_slice();
            // SAFETY: `out_len` is non-null (checked above) and points to a writable value (this
            // function's contract).
            unsafe { *out_len = b.len() };
            alloc::boxed::Box::into_raw(b) as *mut u8
        }
        Err(_) => core::ptr::null_mut(),
    }
}

/// # Safety
///
/// `p` must be null or valid for `len` bytes, unchanged while the parsed items are in use.
unsafe fn cached_items(p: *const u8, len: usize) -> Option<CachedItems<'static>> {
    if p.is_null() {
        return None;
    }
    // SAFETY: `p` is non-null and valid for `len` bytes (this function's contract).
    CachedItems::parse(unsafe { slice::from_raw_parts(p, len) }).ok()
}

/// C ABI: how many bytes from the start of an item icon cache the DS keeps in memory (header and
/// index), from its first 16 bytes; 0 if it isn't one.
///
/// # Safety
///
/// `p` is null or valid for `len` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn verity_items_head_len(p: *const u8, len: usize) -> usize {
    if p.is_null() {
        return 0;
    }
    // SAFETY: `p` is non-null (checked above) and valid for `len` elements (this function's
    // contract).
    CachedItems::head_len(unsafe { slice::from_raw_parts(p, len) }).unwrap_or(0)
}

/// C ABI: the generation an item icon cache head serves (4 or 5), or 0 if it isn't one.
///
/// # Safety
///
/// `head` is null or valid for `len` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn verity_items_generation(head: *const u8, len: usize) -> u8 {
    // SAFETY: `head` is null or valid for `len` bytes (this function's contract).
    (unsafe { cached_items(head, len) }).map_or(0, |c| c.generation)
}

/// C ABI: the icon of held item `item`: file offsets of its 16 colours and 512 tile bytes. False
/// if the cache has no icon for it (including item 0).
///
/// # Safety
///
/// `head` is null or valid for `len` bytes; `palette_at` and `tiles_at` are null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn verity_items_find(
    head: *const u8,
    len: usize,
    item: u16,
    palette_at: *mut u32,
    tiles_at: *mut u32,
) -> bool {
    if palette_at.is_null() || tiles_at.is_null() {
        return false;
    }
    // SAFETY: `head` is null or valid for `len` bytes (this function's contract).
    let Some(r) = unsafe { cached_items(head, len) }.and_then(|c| c.find(item)) else {
        return false;
    };
    // SAFETY: `palette_at` and `tiles_at` are non-null (checked above) and writable (this
    // function's contract).
    unsafe {
        *palette_at = r.palette_at;
        *tiles_at = r.tiles_at;
    }
    true
}

/// Builds the move cache file (category, power, accuracy) for the ROM read by `src`.
/// Descriptions are added when the ROM's text can be read; without them the stats still serve.
pub fn build_moves_cache<S: Source>(src: S) -> Result<alloc::vec::Vec<u8>, Error> {
    let mut rom = Rom::open(src)?;
    let mut table = moves::load(&mut rom)?;
    let text = if table.generation == 4 {
        moves::load_descriptions(&mut rom, |c| {
            Some(value_to_char_g4(c))
                .filter(|&u| u != TERMINATOR && u != 0)
                .and_then(|u| char::from_u32(u.into()))
        })
    } else {
        moves::load_descriptions(&mut rom, |c| char::from_u32(c.into()))
    };
    if let Ok(d) = text {
        table.set_descriptions(d);
    }
    Ok(table.to_cache(rom.game_code()))
}

/// C ABI: the description of move `mv` from a move cache of generation `generation`, as UTF-8
/// into `out` (`cap` bytes, NUL-terminated, cut at a character boundary). False if the cache
/// isn't of that generation or has no description for the move.
///
/// # Safety
///
/// `rom_moves` is null or valid for `len` bytes; `out` is null or writable for `cap` bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn verity_move_description(
    generation: u8,
    mv: u16,
    rom_moves: *const u8,
    len: usize,
    out: *mut u8,
    cap: usize,
) -> bool {
    if rom_moves.is_null() || out.is_null() || cap == 0 {
        return false;
    }
    // SAFETY: `rom_moves` is non-null (checked above) and valid for `len` elements (this function's
    // contract).
    let cache = CachedMoves::parse(unsafe { slice::from_raw_parts(rom_moves, len) }).ok();
    let Some(text) = cache
        .filter(|c| c.generation == generation)
        .and_then(|c| c.description(mv))
        .filter(|t| !t.is_empty())
    else {
        return false;
    };
    let mut n = text.len().min(cap - 1);
    while !text.is_char_boundary(n) {
        n -= 1;
    }
    // SAFETY: `out` is non-null and `cap` > 0 (checked above), writable for `cap` bytes (this
    // function's contract); `n` < `cap`, so the text and its terminator fit, and `text` lives in
    // the move cache, not in `out`.
    unsafe {
        core::ptr::copy_nonoverlapping(text.as_ptr(), out, n);
        *out.add(n) = 0;
    }
    true
}

/// C ABI: builds a move cache from a ROM, like `verity_rom_build_font_cache` (free the result with
/// `verity_rom_free`). The cache serves saves of the ROM's generation only.
///
/// # Safety
///
/// As for `verity_rom_build_font_cache`.
#[unsafe(no_mangle)]
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
            // SAFETY: `out_len` is non-null (checked above) and points to a writable value (this
            // function's contract).
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
#[unsafe(no_mangle)]
pub unsafe extern "C" fn verity_moves_generation(p: *const u8, len: usize) -> u8 {
    if p.is_null() {
        return 0;
    }
    // SAFETY: `p` is non-null (checked above) and valid for `len` elements (this function's
    // contract).
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
#[unsafe(no_mangle)]
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
        // SAFETY: `rom_moves` is non-null (checked above) and valid for `len` elements (this
        // function's contract).
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
    // SAFETY: `out` is non-null (checked above) and points to a writable value (this function's
    // contract).
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

/// # Safety
///
/// `p` must be null or valid for `len` bytes, unchanged while the parsed font are in use.
unsafe fn cached(p: *const u8, len: usize) -> Option<CachedFont<'static>> {
    if p.is_null() {
        return None;
    }
    // SAFETY: `p` is non-null and valid for `len` bytes (this function's contract).
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
#[unsafe(no_mangle)]
pub unsafe extern "C" fn verity_font_check(p: *const u8, len: usize) -> bool {
    // SAFETY: `p` is null or valid for `len` bytes (this function's contract).
    (unsafe { cached(p, len) }).is_some_and(|f| f.validate().is_ok())
}

/// C ABI: looks up the glyph for UTF-16 code unit `c`; false if the font has none.
///
/// # Safety
///
/// `p` is null or valid for `len` bytes; `out` is null or writable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn verity_font_glyph(
    p: *const u8,
    len: usize,
    c: u16,
    out: *mut VerityGlyph,
) -> bool {
    // SAFETY: `p` is null or valid for `len` bytes (this function's contract).
    let (Some(f), false) = (unsafe { cached(p, len) }, out.is_null()) else {
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
    // SAFETY: `out` is non-null (checked above) and writable (this function's contract).
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
// Tests call the C ABI with buffers they own and keep alive for the call.
#[allow(clippy::undocumented_unsafe_blocks)]
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
    fn ribbon_cache_through_the_c_abi() {
        let Some(rom) = rom("white.nds") else {
            return;
        };
        let v = build_ribbon_cache(rom.as_slice()).unwrap();
        assert!(unsafe { verity_ribbons_check(v.as_ptr(), v.len()) });
        let (mut t, mut c) = ([0u8; 512], [0u16; 16]);
        for i in 0..80 {
            assert!(unsafe {
                verity_ribbon_icon(v.as_ptr(), v.len(), i, t.as_mut_ptr(), c.as_mut_ptr())
            });
        }
        assert!(!unsafe {
            verity_ribbon_icon(v.as_ptr(), v.len(), 80, t.as_mut_ptr(), c.as_mut_ptr())
        });
        assert!(!unsafe { verity_ribbons_check(v.as_ptr(), v.len() - 1) });
    }

    #[test]
    fn item_cache_through_the_c_abi() {
        let Some(mut rom) = rom("heartgold.nds") else {
            return;
        };
        let mut len = 0;
        let p = unsafe {
            verity_rom_build_items_cache(
                Some(read_slice),
                &mut rom as *mut _ as *mut c_void,
                &mut len,
            )
        };
        assert!(!p.is_null());
        let v = unsafe { slice::from_raw_parts(p, len) }.to_vec();
        unsafe { verity_rom_free(p, len) };
        let head = unsafe { verity_items_head_len(v.as_ptr(), 16) };
        assert_eq!(head, 16 + 4 * 537);
        assert_eq!(unsafe { verity_items_generation(v.as_ptr(), head) }, 4);
        let (mut pal, mut at) = (0u32, 0u32);
        assert!(
            unsafe { verity_items_find(v.as_ptr(), head, 234, &mut pal, &mut at) },
            "Leftovers"
        );
        assert!(pal as usize + 32 <= at as usize && at as usize + 512 <= v.len());
        assert!(!unsafe { verity_items_find(v.as_ptr(), head, 0, &mut pal, &mut at) });
        assert!(!unsafe { verity_items_find(v.as_ptr(), head, 537, &mut pal, &mut at) });
    }

    unsafe extern "C" fn write_vec(ctx: *mut c_void, buf: *const u8, len: u32) -> bool {
        let out = unsafe { &mut *(ctx as *mut std::vec::Vec<u8>) };
        out.extend_from_slice(unsafe { slice::from_raw_parts(buf, len as usize) });
        true
    }

    #[test]
    fn sprite_cache_through_the_c_abi() {
        for (name, generation) in [("platinum.nds", 4u8), ("black2.nds", 5)] {
            let Some(mut rom) = rom(name) else {
                continue;
            };
            let mut cache = std::vec::Vec::new();
            let n = unsafe {
                verity_rom_build_sprite_cache(
                    Some(read_slice),
                    &mut rom as *mut _ as *mut c_void,
                    Some(write_vec),
                    &mut cache as *mut _ as *mut c_void,
                )
            };
            assert!(n > 600, "{name}: {n}");
            let foot = &cache[cache.len() - 24..];
            let start = unsafe { verity_sprites_tail_start(foot.as_ptr(), 24, cache.len() as u32) };
            let tail = &cache[start as usize..];
            assert_eq!(
                unsafe { verity_sprites_generation(tail.as_ptr(), tail.len(), start) },
                generation
            );
            let (mut pal, mut at, mut len) = (0, 0, 0);
            // Pikachu, female: falls back to the male's sprite where there's no own one.
            assert!(unsafe {
                verity_sprites_find(
                    tail.as_ptr(),
                    tail.len(),
                    start,
                    25,
                    0,
                    true,
                    false,
                    &mut pal,
                    &mut at,
                    &mut len,
                )
            });
            let mut out = [0u8; 4608];
            let blob = &cache[at as usize..(at + len) as usize];
            assert!(unsafe { verity_sprite_decode(blob.as_ptr(), blob.len(), out.as_mut_ptr()) });
            assert!(out.iter().any(|&b| b != 0));
            let (mut tiles, mut colors) = ([0u8; 256], [0u16; 16]);
            assert!(unsafe {
                verity_sprites_icon(
                    tail.as_ptr(),
                    tail.len(),
                    start,
                    0,
                    16,
                    tiles.as_mut_ptr(),
                    colors.as_mut_ptr(),
                )
            });
            assert!(!unsafe {
                verity_sprites_icon(
                    tail.as_ptr(),
                    tail.len(),
                    start,
                    0,
                    17,
                    tiles.as_mut_ptr(),
                    colors.as_mut_ptr(),
                )
            });
        }
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
    fn move_descriptions_from_each_rom() {
        let tackle4 = "A physical attack in which the user charges and slams into the foe with \
                       its whole body.";
        let tackle5 = "A physical attack in which the user charges and slams into the target \
                       with its whole body.";
        for (name, generation, tackle) in [
            ("platinum.nds", 4, tackle4),
            ("heartgold.nds", 4, tackle4),
            ("white.nds", 5, tackle5),
            ("black2.nds", 5, tackle5),
        ] {
            let Some(rom) = rom(name) else { continue };
            let v = build_moves_cache(rom.as_slice()).unwrap();
            let desc = |mv: u16, cap: usize| {
                let mut buf = std::vec![0u8; cap];
                let ok = unsafe {
                    verity_move_description(
                        generation,
                        mv,
                        v.as_ptr(),
                        v.len(),
                        buf.as_mut_ptr(),
                        cap,
                    )
                };
                ok.then(|| {
                    let end = buf.iter().position(|&b| b == 0).unwrap();
                    std::string::String::from_utf8(buf[..end].to_vec()).unwrap()
                })
            };
            assert_eq!(desc(33, 256).as_deref(), Some(tackle), "{name}");
            assert_eq!(desc(0, 256), None, "{name}: no move");
            assert_eq!(
                desc(33, 10).as_deref(),
                Some("A physica"),
                "{name}: cut to fit"
            );
            // Every real move has a description with no control-code leftovers or double spaces.
            let last = if generation == 4 { 467 } else { 559 };
            for mv in 1..=last {
                let d = desc(mv, 256).unwrap_or_else(|| panic!("{name}: move {mv}"));
                assert!(
                    !d.contains("  ") && !d.starts_with(' ') && !d.ends_with(' '),
                    "{name} {mv}: {d:?}"
                );
                assert!(
                    d.chars().all(|c| c >= ' ' && c != '\u{FFFD}'),
                    "{name} {mv}: {d:?}"
                );
                assert!(d.len() < 130, "{name} {mv}: {} {d:?}", d.len());
            }
            // The other generation gets nothing from this cache.
            let other = 9 - generation;
            let mut buf = [0u8; 8];
            assert!(!unsafe {
                verity_move_description(other, 33, v.as_ptr(), v.len(), buf.as_mut_ptr(), 8)
            });
        }
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
