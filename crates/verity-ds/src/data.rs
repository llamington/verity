//! The open save, the data blobs and the caches built from the user's ROM, over the core. The UI
//! never touches save bytes.

use alloc::vec;
use alloc::vec::Vec;
use core::cell::RefMut;
use core::ffi::CStr;

use blocksds::fs::{self, File, SeekFrom};
use blocksds::timer;

use crate::cell::Global;
use crate::ffi::{self, BlobKind, Loc, SaveInfo};
use crate::text::{LongText, Text, cstr, eq_ignore_case};
use crate::{long, t, ui};

/// A path as a C string.
pub struct CPath([u8; 256]);

impl CPath {
    pub fn new(path: &[u8]) -> Self {
        let mut p = [0u8; 256];
        let n = path.len().min(255);
        p[..n].copy_from_slice(&path[..n]);
        Self(p)
    }

    pub fn as_cstr(&self) -> &CStr {
        CStr::from_bytes_until_nul(&self.0).unwrap_or(c"")
    }
}

pub struct Doc {
    pub path: LongText,
    pub buf: Vec<u8>,
    pub info: SaveInfo,
    pub boxes: u32,
    pub dirty: bool, // unsaved changes
    /// The checksums of the file as last read or written. Edits leave the block checksums in
    /// `buf` stale until saving recomputes them, so `info.checksums_valid` is false while dirty.
    pub file_checksums_ok: bool,
}

impl Doc {
    pub fn loaded(&self) -> bool {
        !self.buf.is_empty()
    }
}

static DOC: Global<Doc> = Global::new(Doc {
    path: LongText::new(),
    buf: Vec::new(),
    info: SaveInfo {
        kind: 0,
        generation: 0,
        party_count: 0,
        checksums_valid: 0,
        tid: 0,
        sid: 0,
        ot_name: [0; 13],
        played_hours: 0,
        played_minutes: 0,
        played_seconds: 0,
        game_version: 0,
    },
    boxes: 0,
    dirty: false,
    file_checksums_ok: false,
});

/// The open save. Don't hold it across other calls into this module, which borrow it too.
pub fn doc() -> RefMut<'static, Doc> {
    DOC.borrow_mut()
}

struct Blobs {
    nitro_tried: bool,
    nitro: bool,
    loaded: [bool; 24],
    data_ok: bool,
}

static BLOBS: Global<Blobs> = Global::new(Blobs {
    nitro_tried: false,
    nitro: false,
    loaded: [false; 24],
    data_ok: false,
});

const MAX_SAVE: u32 = 1024 * 1024;

fn read_all(path: &[u8], max: u32) -> Option<Vec<u8>> {
    let mut f = File::open(CPath::new(path).as_cstr()).ok()?;
    let n = f.size().ok()?;
    if n == 0 || n > max {
        return None;
    }
    let mut buf = vec![0u8; n as usize];
    f.read_exact(&mut buf).ok()?;
    Some(buf)
}

fn attach(kind: BlobKind, file: &str) -> bool {
    if BLOBS.borrow_mut().loaded[kind as usize] {
        return true;
    }
    let Some(buf) = read_all(&t!("nitro:/data/", file).cap(64), 2 * 1024 * 1024) else {
        return false;
    };
    // Kept for the program's lifetime.
    if ffi::data_attach(kind, buf.leak()) != ffi::Err::Ok {
        return false;
    }
    BLOBS.borrow_mut().loaded[kind as usize] = true;
    true
}

/// Reads and probes a save file. On failure the previous document is kept and the error says why.
pub fn open(path: &[u8]) -> Result<(), &'static str> {
    let Some(mut buf) = read_all(path, MAX_SAVE) else {
        return Err("cannot read file");
    };
    let Some(info) = ffi::save_probe(&mut buf) else {
        return Err("not a Gen 4/5 save");
    };
    let mut d = doc();
    d.boxes = ffi::box_count(&mut buf);
    d.buf = buf;
    d.info = info;
    d.file_checksums_ok = info.checksums_valid != 0;
    d.dirty = false; // a fresh file (unsaved edits to the previous one are gone)
    d.path = LongText::from(path).cap(192);
    Ok(())
}

/// Loads (once) the blobs the save's game needs from NitroFS and attaches them. Returns false if
/// NitroFS or a blob is missing (the UI then shows IDs instead of names).
pub fn load_data_for(kind: u8) -> bool {
    {
        let mut b = BLOBS.borrow_mut();
        if !b.nitro_tried {
            b.nitro_tried = true;
            b.nitro = fs::init_nitrofs(None).is_ok();
        }
        if !b.nitro {
            b.data_ok = false;
            return false;
        }
    }
    let mut ok = attach(BlobKind::Strings, "strings_en.bin");
    match kind {
        1 => ok &= attach(BlobKind::Dp, "dp.bin"),
        2 => ok &= attach(BlobKind::Pt, "pt.bin"),
        3 => ok &= attach(BlobKind::Hgss, "hgss.bin"),
        4 => ok &= attach(BlobKind::Bw, "bw.bin"),
        5 => ok &= attach(BlobKind::B2w2, "b2w2.bin"),
        _ => {}
    }
    BLOBS.borrow_mut().data_ok = ok;
    ok
}

pub fn data_available() -> bool {
    BLOBS.borrow_mut().data_ok
}

pub fn slot(box_index: u8, slot: u8) -> ffi::PkmView {
    use ffi::Zeroed;
    let mut d = doc();
    if !d.loaded() {
        return ffi::PkmView::zeroed();
    }
    ffi::slot_view(&mut d.buf, Loc { box_index, slot }).unwrap_or_else(ffi::PkmView::zeroed)
}

/// Legality of a slot, and the milliseconds it took. Loads the legality data on first use.
pub fn legality(box_index: u8, slot: u8) -> (ffi::Legality, u32) {
    use ffi::Zeroed;
    if !doc().loaded() {
        return (ffi::Legality::zeroed(), 0);
    }
    ensure_save_data();
    timer::cpu_start_timing(0);
    let r = ffi::legality(&mut doc().buf, Loc { box_index, slot })
        .unwrap_or_else(ffi::Legality::zeroed);
    let ticks = timer::cpu_end_timing();
    (r, timer::ticks_to_ms(ticks))
}

pub fn check_name(id: u8) -> &'static str {
    // legality::analysis4::CheckId order.
    const NAMES: [&str; 27] = [
        "CurrentMove",
        "Encounter",
        "Shiny",
        "EC",
        "PID",
        "Gender",
        "EVs",
        "Language",
        "Nickname",
        "Trainer",
        "IVs",
        "Level",
        "Ball",
        "Form",
        "Egg",
        "Misc",
        "Fateful",
        "Ribbon",
        "Ability",
        "Evolution",
        "Nature",
        "GameOrigin",
        "HeldItem",
        "Marking",
        "TrashBytes",
        "SlotType",
        "Memory",
    ];
    NAMES.get(usize::from(id)).copied().unwrap_or("?")
}

/// A box's name, fitting a C buffer of `cap` bytes.
pub fn box_name(b: u32, cap: usize) -> Text {
    let mut w = [0u16; 21];
    let ok = {
        let mut d = doc();
        d.loaded() && ffi::box_name(&mut d.buf, b, &mut w)
    };
    if !ok {
        return t!("Box ", b + 1).cap(cap);
    }
    ui::utf16_to_text(&w, cap)
}

fn config_path() -> Text {
    // The SD card on DSi, the flashcart otherwise; fat:/ also works under melonDS DLDI.
    let root = if fs::metadata(c"sd:/").is_ok() {
        "sd:"
    } else {
        "fat:"
    };
    t!(root, "/_nds/verity").cap(48)
}

fn ensure_config_dir() {
    let dir = config_path();
    let nds = &dir[..dir.iter().rposition(|&b| b == b'/').unwrap_or(0)];
    let _ = fs::create_dir(CPath::new(nds).as_cstr());
    let _ = fs::create_dir(CPath::new(&dir).as_cstr());
}

/// The last opened file, kept in /_nds/verity/last.txt on the SD card.
pub fn remember(p: &[u8]) {
    ensure_config_dir();
    let file = t!(&config_path(), "/last.txt").cap(80);
    let Ok(mut f) = File::open_with(CPath::new(&file).as_cstr(), c"w") else {
        return;
    };
    let _ = f.write_all(&t!(p, "\n"));
}

/// The last opened file, at most `cap - 1` bytes.
pub fn last(cap: usize) -> Option<LongText> {
    let file = t!(&config_path(), "/last.txt").cap(80);
    let mut f = File::open_with(CPath::new(&file).as_cstr(), c"r").ok()?;
    let mut buf = [0u8; 256];
    let n = f.read(&mut buf[..cap.min(256) - 1]).ok()?;
    let line = &buf[..n];
    let line = &line[..line
        .iter()
        .position(|&b| b == b'\n')
        .map_or(line.len(), |i| i + 1)];
    let end = line
        .iter()
        .position(|&b| b == b'\r' || b == b'\n')
        .unwrap_or(line.len());
    (n > 0 && end > 0).then(|| LongText::from(&line[..end]))
}

// ---- ROM assets ----------------------------------------------------------------------------
// The font comes from the user's own ROM: the first time a Gen 4/5 .nds is on the SD card, its
// font goes into font.vfnt and the ROM isn't needed again. A better ROM (Gen 5 covers kana,
// kanji and Hangul) replaces a cache built from a worse one. See docs/research/rom-font.md.

/// A cache file kept open, with its head (or tail) in memory.
struct OpenCache {
    file: File,
    head: Vec<u8>,
}

struct Sprites {
    file: File,
    tail: Vec<u8>, // icons, index and footer (about 20 KB)
    start: u32,
}

struct Assets {
    font_game: [u8; 5],
    icons: Option<OpenCache>, // icons.vicn, kept open: tiles are read per icon
    ribbons: Vec<u8>,         // ribbons.vrib, kept whole in memory (about 28 KB)
    items: [Option<OpenCache>; 2], // items4/5.vitm: the index in memory, icons read per use
    sprites: [Option<Sprites>; 2], // sprites4/5.vspr, kept open: sprites read per view
    moves: [Vec<u8>; 2],      // move caches for Gen 4 and Gen 5 saves
}

static ASSETS: Global<Assets> = Global::new(Assets {
    font_game: [0; 5],
    icons: None,
    ribbons: Vec::new(),
    items: [None, None],
    sprites: [None, None],
    moves: [Vec::new(), Vec::new()],
});

fn rom_rank(path: &[u8]) -> u8 {
    let Ok(mut f) = File::open(CPath::new(path).as_cstr()) else {
        return 0;
    };
    let mut code = [0u8; 4];
    if f.seek(SeekFrom::Start(0x0C)).is_ok() && f.read_exact(&mut code).is_ok() {
        ffi::rom_font_rank(&code)
    } else {
        0
    }
}

fn file_rank(code: &[u8]) -> u8 {
    code.get(..4)
        .and_then(|c| <&[u8; 4]>::try_from(c).ok())
        .map_or(0, ffi::rom_font_rank)
}

/// The best ROM overall (font and icons) and the best of each generation (move stats, which
/// differ between generations).
#[derive(Default)]
struct RomScan {
    best: LongText,
    rank: u8,
    generation: [LongText; 2], // Gen 4, Gen 5
    gen_rank: [u8; 2],
}

fn scan_roms(dir: &[u8], depth: i32, scan: &mut RomScan) {
    let Ok(entries) = fs::read_dir(CPath::new(dir).as_cstr()) else {
        return;
    };
    for e in entries {
        if scan.rank == 3 && scan.gen_rank[0] > 0 {
            break;
        }
        let name = e.name().to_bytes();
        if name.first() == Some(&b'.') {
            continue;
        }
        let sep: &[u8] = if dir.last() == Some(&b'/') { b"" } else { b"/" };
        let path = long!(dir, sep, name);
        if path.len() >= 192 {
            continue;
        }
        if e.is_dir() {
            if depth > 0 {
                scan_roms(&path, depth - 1, scan);
            }
        } else if name
            .iter()
            .rposition(|&b| b == b'.')
            .is_some_and(|i| eq_ignore_case(&name[i..], b".nds"))
        {
            let r = rom_rank(&path);
            if r > scan.rank {
                scan.rank = r;
                scan.best = path;
            }
            let g = if r == 3 { 1 } else { 0 };
            if r > 0 && r > scan.gen_rank[g] {
                scan.gen_rank[g] = r;
                scan.generation[g] = path;
            }
        }
    }
}

fn load_moves(g: usize, file: &[u8]) {
    // Stats and descriptions (about 50 KB).
    let b = read_all(file, 96 * 1024).filter(|b| usize::from(ffi::moves_generation(b)) == 4 + g);
    ASSETS.borrow_mut().moves[g] = b.unwrap_or_default();
}

/// Writes a cache the core built; true if the file was written whole.
fn write_cache(file: &[u8], built: ffi::Built) -> bool {
    ensure_config_dir();
    let path = CPath::new(file);
    let ok = match File::create(path.as_cstr()) {
        Ok(mut out) => {
            let wrote = out.write_all(built.bytes()).is_ok();
            out.close().is_ok() && wrote
        }
        Err(_) => false,
    };
    if !ok {
        let _ = fs::remove(path.as_cstr());
    }
    ok
}

/// Opens a cache with an index at its start: reads its first 16 bytes, asks `head_len` how long
/// the index is, and keeps it if `valid`.
fn open_head(
    file: &[u8],
    max: usize,
    head_len: fn(&[u8]) -> usize,
    valid: impl Fn(&[u8]) -> bool,
) -> Option<OpenCache> {
    let mut f = File::open(CPath::new(file).as_cstr()).ok()?;
    let mut h = [0u8; 16];
    f.read_exact(&mut h).ok()?;
    let n = head_len(&h);
    if !(16..max).contains(&n) {
        return None;
    }
    let mut head = vec![0u8; n];
    f.seek(SeekFrom::Start(0)).ok()?;
    f.read_exact(&mut head).ok()?;
    valid(&head).then_some(OpenCache { file: f, head })
}

/// Opens icons.vicn and keeps its head (palettes and index) in memory; returns its rank.
fn open_icons(file: &[u8]) -> u8 {
    let icons = open_head(file, 64 * 1024, ffi::icons_head_len, |h| {
        ffi::icons_palette_count(h) != 0
    });
    let rank = icons.as_ref().map_or(0, |c| file_rank(&c.head[12..]));
    ASSETS.borrow_mut().icons = icons;
    rank
}

/// Loads ribbons.vrib; returns its rank (0 for none).
fn load_ribbons(file: &[u8]) -> u8 {
    let b = read_all(file, 64 * 1024)
        .filter(|b| ffi::ribbons_check(b))
        .unwrap_or_default();
    let rank = if b.is_empty() { 0 } else { file_rank(&b[12..]) };
    ASSETS.borrow_mut().ribbons = b;
    rank
}

fn open_items(g: usize, file: &[u8]) {
    let items = open_head(file, 16 * 1024, ffi::items_head_len, |h| {
        usize::from(ffi::items_generation(h)) == 4 + g
    });
    ASSETS.borrow_mut().items[g] = items;
}

fn open_sprites(g: usize, file: &[u8]) {
    ASSETS.borrow_mut().sprites[g] = None;
    let footer = ffi::SPRITES_FOOTER as u32;
    let Ok(mut f) = File::open(CPath::new(file).as_cstr()) else {
        return;
    };
    let size = f.seek(SeekFrom::End(0)).ok();
    let mut foot = [0u8; ffi::SPRITES_FOOTER];
    let start = match size {
        Some(size)
            if size > footer
                && f.seek(SeekFrom::Start(size - footer)).is_ok()
                && f.read_exact(&mut foot).is_ok() =>
        {
            ffi::sprites_tail_start(&foot, size)
        }
        _ => u32::MAX,
    };
    let size = size.unwrap_or(0);
    let n = if start < size {
        (size - start) as usize
    } else {
        0
    };
    if n == 0 || n >= 64 * 1024 {
        return;
    }
    let mut tail = vec![0u8; n];
    if f.seek(SeekFrom::Start(start)).is_err()
        || f.read_exact(&mut tail).is_err()
        || usize::from(ffi::sprites_generation(&tail, start)) != 4 + g
    {
        return;
    }
    ASSETS.borrow_mut().sprites[g] = Some(Sprites {
        file: f,
        tail,
        start,
    });
}

/// Streams the sprite cache straight to the card (a Gen 4 one is about 1 MB).
fn build_sprites(g: usize, rom: &mut File, file: &[u8]) {
    ui::at(
        ui::top(),
        0,
        2 + 2 * g as i32,
        &t!("Copying Gen ", 4 + g, " sprites (once)..."),
    );
    ui::present();
    ensure_config_dir();
    let path = CPath::new(file);
    let Ok(mut out) = File::create(path.as_cstr()) else {
        return;
    };
    let mut ok = ffi::build_sprite_cache(rom, &mut out) > 0;
    ok = out.close().is_ok() && ok;
    if !ok {
        let _ = fs::remove(path.as_cstr());
    } else {
        open_sprites(g, file);
    }
}

/// Loads the ROM font and box icon caches, building them from the best Gen 4/5 ROM on the SD card
/// when they are missing or a better ROM appears. Without a ROM: Latin text, placeholder icons.
pub fn rom_assets_init() {
    let config = config_path();
    let font_file = t!(&config, "/font.vfnt").cap(80);
    let icon_file = t!(&config, "/icons.vicn").cap(80);
    let ribbon_file = t!(&config, "/ribbons.vrib").cap(80);
    let mut cached = read_all(&font_file, 512 * 1024).filter(|c| ffi::font_check(c));
    let font_rank = cached.as_ref().map_or(0, |c| file_rank(&c[12..]));
    let icon_rank = open_icons(&icon_file);
    let ribbon_rank = load_ribbons(&ribbon_file);
    let mut move_file = [Text::new(), Text::new()];
    let mut item_file = [Text::new(), Text::new()];
    let mut sprite_file = [Text::new(), Text::new()];
    for g in 0..2 {
        sprite_file[g] = t!(&config, "/sprites", 4 + g, ".vspr").cap(80);
        open_sprites(g, &sprite_file[g]);
        move_file[g] = t!(&config, "/moves", 4 + g, ".vmov").cap(80);
        load_moves(g, &move_file[g]);
        item_file[g] = t!(&config, "/items", 4 + g, ".vitm").cap(80);
        open_items(g, &item_file[g]);
    }
    let complete = |g: usize| {
        let a = ASSETS.borrow_mut();
        (
            !a.moves[g].is_empty(),
            a.items[g].is_some(),
            a.sprites[g].is_some(),
        )
    };
    let all = (0..2).all(|g| complete(g) == (true, true, true));
    if font_rank < 3 || icon_rank < 3 || ribbon_rank < 3 || !all {
        let mut scan = RomScan::default();
        for root in [&b"sd:/"[..], b"fat:/"] {
            scan_roms(root, 3, &mut scan);
        }
        for g in 0..2 {
            let (moves, items, sprites) = complete(g);
            if (moves && items && sprites) || scan.gen_rank[g] == 0 {
                continue;
            }
            let Ok(mut f) = File::open(CPath::new(&scan.generation[g]).as_cstr()) else {
                continue;
            };
            if !moves
                && let Some(built) = ffi::build_moves_cache(&mut f)
                && write_cache(&move_file[g], built)
            {
                load_moves(g, &move_file[g]);
            }
            if !items
                && let Some(built) = ffi::build_items_cache(&mut f)
                && write_cache(&item_file[g], built)
            {
                open_items(g, &item_file[g]);
            }
            if !sprites {
                build_sprites(g, &mut f, &sprite_file[g]);
            }
        }
        let best_rank = scan.rank;
        if (best_rank > font_rank || best_rank > icon_rank || best_rank > ribbon_rank)
            && let Ok(mut f) = File::open(CPath::new(&scan.best).as_cstr())
        {
            if best_rank > font_rank
                && let Some(built) = ffi::build_font_cache(&mut f)
                && write_cache(&font_file, built)
            {
                cached = read_all(&font_file, 512 * 1024).filter(|c| ffi::font_check(c));
            }
            if best_rank > icon_rank {
                ASSETS.borrow_mut().icons = None;
                if let Some(built) = ffi::build_icon_cache(&mut f) {
                    write_cache(&icon_file, built);
                }
                open_icons(&icon_file);
            }
            if best_rank > ribbon_rank
                && let Some(built) = ffi::build_ribbon_cache(&mut f)
                && write_cache(&ribbon_file, built)
            {
                load_ribbons(&ribbon_file);
            }
        }
    }
    if let Some(font) = cached {
        let font = font.leak();
        let mut a = ASSETS.borrow_mut();
        a.font_game[..4].copy_from_slice(&font[12..16]);
        drop(a);
        ui::set_rom_font(font);
    }
    let pal = {
        let a = ASSETS.borrow_mut();
        a.icons.as_ref().map(|icons| {
            let count = usize::from(ffi::icons_palette_count(&icons.head)).min(4);
            let mut pal = [[0u16; 16]; 4];
            for (p, colors) in pal.iter_mut().enumerate().take(count) {
                ffi::icons_palette(&icons.head, p as u8, colors);
            }
            (pal, count)
        })
    };
    if let Some((pal, count)) = pal {
        ui::set_icon_palettes(&pal[..count]);
    }
}

/// A Pokémon's box icon from the ROM: 512 bytes of 4 bpp tiles (1D 32x32), and its palette.
pub fn icon_tiles(
    species: u16,
    form: u8,
    female: bool,
    egg: bool,
    tiles: &mut [u8; 512],
) -> Option<u8> {
    let mut a = ASSETS.borrow_mut();
    let icons = a.icons.as_mut()?;
    let (palette, at) = ffi::icons_find(&icons.head, species, form, female, egg)?;
    (icons.file.seek(SeekFrom::Start(at)).is_ok() && icons.file.read_exact(tiles).is_ok())
        .then_some(palette)
}

/// A ribbon's image from the ROM by verity's list position: 512 bytes of 4 bpp tiles (1D 32x32)
/// and 16 colours. False without a ROM.
pub fn ribbon_icon(index: i32, tiles: &mut [u8; 512], colors: &mut [u16; 16]) -> bool {
    let a = ASSETS.borrow_mut();
    (0..80).contains(&index) && ffi::ribbon_icon(&a.ribbons, index as u8, tiles, colors)
}

fn read_colors(f: &mut File, colors: &mut [u16; 16]) -> bool {
    let mut b = [0u8; 32];
    if f.read_exact(&mut b).is_err() {
        return false;
    }
    for (c, h) in colors.iter_mut().zip(b.as_chunks::<2>().0) {
        *c = u16::from_le_bytes(*h);
    }
    true
}

/// A held item's icon from a ROM of the save's generation: 512 bytes of 4 bpp tiles (1D 32x32)
/// and 16 BGR555 colours. False without one (and for item 0).
pub fn item_icon(generation: u8, item: u16, tiles: &mut [u8; 512], colors: &mut [u16; 16]) -> bool {
    let g = if generation == 5 { 1 } else { 0 };
    let mut a = ASSETS.borrow_mut();
    let Some(c) = a.items[g].as_mut() else {
        return false;
    };
    let Some((pal_at, at)) = ffi::items_find(&c.head, item) else {
        return false;
    };
    c.file.seek(SeekFrom::Start(pal_at)).is_ok()
        && read_colors(&mut c.file, colors)
        && c.file.seek(SeekFrom::Start(at)).is_ok()
        && c.file.read_exact(tiles).is_ok()
}

/// A Pokémon's front sprite from the generation's sprite cache: 4608 tile bytes (96x96 4 bpp, 1D
/// blocks 64x64, 32x64, 64x32, 32x32) and 16 colours (normal or shiny). False without a cache.
#[allow(clippy::too_many_arguments)]
pub fn pokemon_sprite(
    generation: u8,
    species: u16,
    form: u8,
    female: bool,
    egg: bool,
    shiny: bool,
    tiles: &mut [u8; 4608],
    colors: &mut [u16; 16],
) -> bool {
    let g = if generation == 5 { 1 } else { 0 };
    let mut a = ASSETS.borrow_mut();
    let Some(s) = a.sprites[g].as_mut() else {
        return false;
    };
    let Some((pal_at, at, len)) = ffi::sprites_find(&s.tail, s.start, species, form, female, egg)
    else {
        return false;
    };
    if len > 8 * 1024 {
        return false;
    }
    let mut blob = vec![0u8; len as usize];
    s.file
        .seek(SeekFrom::Start(pal_at + if shiny { 32 } else { 0 }))
        .is_ok()
        && read_colors(&mut s.file, colors)
        && s.file.seek(SeekFrom::Start(at)).is_ok()
        && s.file.read_exact(&mut blob).is_ok()
        && ffi::sprite_decode(&blob, tiles)
}

/// A type icon (kind 0, verity's type number) or move category icon (kind 1: physical, special,
/// status) from the sprite cache: 256 tile bytes (32x16 4 bpp) and 16 colours.
pub fn type_icon(
    generation: u8,
    kind: u8,
    index: i32,
    tiles: &mut [u8; 256],
    colors: &mut [u16; 16],
) -> bool {
    let g = if generation == 5 { 1 } else { 0 };
    let a = ASSETS.borrow_mut();
    let Some(s) = a.sprites[g].as_ref() else {
        return false;
    };
    (0..32).contains(&index)
        && ffi::sprites_icon(&s.tail, s.start, kind, index as u8, tiles, colors)
}

/// The item icon of the ball a Pokémon is in (a Poké Ball for none or an unknown ball).
pub fn ball_icon(generation: u8, ball: u8, tiles: &mut [u8; 512], colors: &mut [u16; 16]) -> bool {
    // Balls 1-16 are items 1-16; Fast to Sport (17-24) are 492-499; the Dream Ball (25) is 576.
    let item = match ball {
        1..=16 => u16::from(ball),
        17..=24 => 492 + u16::from(ball - 17),
        25 => 576,
        _ => 4,
    };
    item_icon(generation, item, tiles, colors)
}

/// Type and PP of a move, plus category, power and accuracy when a ROM of the save's generation
/// was found (from_rom). None for a move number the generation doesn't have.
pub fn move_info(generation: u8, mv: u16) -> Option<ffi::MoveInfo> {
    let g = if generation == 5 { 1 } else { 0 };
    ffi::move_info(generation, mv, &ASSETS.borrow_mut().moves[g])
}

/// A move's description (UTF-8) from a ROM of the save's generation, fitting a C buffer of `cap`.
pub fn move_description(generation: u8, mv: u16, cap: usize) -> Option<LongText> {
    let g = if generation == 5 { 1 } else { 0 };
    let mut out = [0u8; 256];
    let cap = cap.min(256);
    ffi::move_description(
        generation,
        mv,
        &ASSETS.borrow_mut().moves[g],
        &mut out[..cap],
    )
    .then(|| LongText::from(cstr(&out[..cap])))
}

/// The game code of the ROM the font came from ("" for none).
pub fn font_game() -> Text {
    Text::from(cstr(&ASSETS.borrow_mut().font_game))
}

/// Reads a whole save file.
pub fn read_save_file(path: &[u8]) -> Option<Vec<u8>> {
    read_all(path, MAX_SAVE)
}

pub fn game_name(kind: u8) -> &'static str {
    match kind {
        1 => "Diamond/Pearl",
        2 => "Platinum",
        3 => "HeartGold/SoulSilver",
        4 => "Black/White",
        5 => "Black 2/White 2",
        _ => "?",
    }
}

/// The version whose wild slots the add-Pokémon wizard offers for this save (10/11/12).
pub fn wizard_version() -> u8 {
    // D/P saves get Diamond slots, Pt saves Platinum, HG/SS saves HeartGold (the save does not
    // record which of each pair it is; version exclusives differ).
    let info = doc().info;
    match info.kind {
        1 => 10,
        3 => 7,
        4 => {
            if info.game_version == 21 {
                21
            } else {
                20
            }
        }
        5 => {
            if info.game_version == 23 {
                23
            } else {
                22
            }
        }
        _ => 12,
    }
}

fn nitro_tried() -> bool {
    BLOBS.borrow_mut().nitro_tried
}

/// Makes sure the Gen 4 legality/generation data is attached.
pub fn ensure_gen4_data() -> bool {
    if !nitro_tried() {
        load_data_for(doc().info.kind);
    }
    // Everything the Gen 4 legality context reads.
    let mut ok = attach(BlobKind::Hgss, "hgss.bin");
    ok &= attach(BlobKind::Pt, "pt.bin");
    ok &= attach(BlobKind::Dp, "dp.bin");
    ok &= attach(BlobKind::B2w2, "b2w2.bin");
    ok &= attach(BlobKind::Moves4, "moves4.bin");
    ok &= attach(BlobKind::EncDppt, "enc_dppt.bin");
    ok &= attach(BlobKind::Names4, "names4.bin");
    ok &= attach(BlobKind::EncHgss, "enc_hgss.bin");
    ok &= attach(BlobKind::Gifts4, "gifts4.bin");
    attach(BlobKind::G3Origin, "g3_origin.bin"); // Optional: Pal Park Pokémon stay unchecked.
    ok
}

fn ensure_gen5_data() -> bool {
    if !nitro_tried() {
        load_data_for(doc().info.kind);
    }
    let mut ok = attach(BlobKind::Bw, "bw.bin");
    ok &= attach(BlobKind::B2w2, "b2w2.bin");
    ok &= attach(BlobKind::Moves5, "moves5.bin");
    ok &= attach(BlobKind::EncBw, "enc_bw.bin");
    ok &= attach(BlobKind::EncB2w2, "enc_b2w2.bin");
    ok &= attach(BlobKind::EncDr, "enc_dr.bin");
    ok &= attach(BlobKind::Names5, "names5.bin");
    attach(BlobKind::Gifts5, "gifts5.bin"); // Optional: Wonder Card Pokémon stay unchecked.
    // Poké Transfer: Gen 3/4 origins. Optional, they stay unchecked without it.
    if ok {
        ensure_gen4_data();
    }
    ok
}

pub fn ensure_save_data() -> bool {
    let generation = doc().info.generation;
    if generation == 5 {
        ensure_gen5_data()
    } else {
        ensure_gen4_data()
    }
}

/// Attaches the Mystery Gift databases (first use only; about 500 KB).
pub fn ensure_events_data() -> bool {
    if !nitro_tried() {
        load_data_for(doc().info.kind);
    }
    if doc().info.generation == 5 {
        let ok5 = attach(BlobKind::Gifts5, "gifts5.bin");
        return ok5 & attach(BlobKind::Events5, "events5.bin");
    }
    let ok = attach(BlobKind::Gifts4, "gifts4.bin");
    ok & attach(BlobKind::Events4, "events4.bin")
}

fn write_file(path: &CStr, buf: &[u8]) -> bool {
    let Ok(mut f) = File::create(path) else {
        return false;
    };
    let mut ok = f.write_all(buf).is_ok();
    ok &= f.sync().is_ok();
    ok &= f.close().is_ok();
    ok
}

/// Writes the open save back to its file: finalise, validate (never writes if the validator
/// fails), rotate backups (<name>.bak1..3), write a temp file, swap it in, re-read and compare.
/// Returns what happened, for the user.
pub fn save_to_disk() -> Text {
    if !doc().loaded() {
        return Text::from(b"no save open");
    }
    // The validator reads the game data, which no earlier screen may have needed.
    ensure_save_data();
    let mut d = doc();
    ffi::save_finalize(&mut d.buf);
    let (r, c) = ffi::save_check(&mut d.buf);
    if r != ffi::Err::Ok || c.issue != 0 {
        return if c.issue == 3 {
            let party = c.box_index == 0xFF;
            let n = if party {
                u32::from(c.slot)
            } else {
                u32::from(c.box_index) * 30 + u32::from(c.slot)
            };
            t!(
                "unsafe Pokemon at ",
                if party { "party" } else { "box" },
                " ",
                n + 1
            )
            .cap(48)
        } else {
            t!("validator refused (issue ", c.issue, ")").cap(48)
        };
    }
    let path = d.path;
    let tmp = CPath::new(&long!(&path, ".tmp").cap(200));
    let bak = [1, 2, 3].map(|i| CPath::new(&long!(&path, ".bak", i).cap(200)));
    let file = CPath::new(&path);
    if !write_file(tmp.as_cstr(), &d.buf) {
        let _ = fs::remove(tmp.as_cstr());
        return Text::from(b"could not write temp file");
    }
    // Rotate: bak2 -> bak3, bak1 -> bak2, current file -> bak1, temp -> current.
    let _ = fs::remove(bak[2].as_cstr());
    let _ = fs::rename(bak[1].as_cstr(), bak[2].as_cstr());
    let _ = fs::rename(bak[0].as_cstr(), bak[1].as_cstr());
    if fs::rename(file.as_cstr(), bak[0].as_cstr()).is_err()
        || fs::rename(tmp.as_cstr(), file.as_cstr()).is_err()
    {
        return Text::from(b"could not replace the file (temp kept)");
    }
    // Read back in chunks: a second save-sized buffer may not fit next to the save, the game data
    // and the ROM caches (a Gen 5 save is 512 KB).
    let mut same = false;
    if let Ok(mut f) = File::open(file.as_cstr()) {
        let mut chunk = vec![0u8; 4096];
        let mut at = 0;
        same = true;
        loop {
            match f.read(&mut chunk) {
                Ok(0) => break,
                Ok(n) => {
                    same = at + n <= d.buf.len() && chunk[..n] == d.buf[at..at + n];
                    at += n;
                    if !same {
                        break;
                    }
                }
                Err(_) => {
                    same = false;
                    break;
                }
            }
        }
        same = same && at == d.buf.len();
    }
    if !same {
        return Text::from(b"verify failed: restore from .bak1");
    }
    d.dirty = false;
    if let Some(info) = ffi::save_probe(&mut d.buf) {
        d.info = info;
    }
    d.file_checksums_ok = d.info.checksums_valid != 0;
    Text::from(b"saved (backup: .bak1)")
}
