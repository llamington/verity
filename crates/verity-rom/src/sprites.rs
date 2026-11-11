//! Pokémon front sprites, type icons and move category icons, and verity's sprite cache (one per
//! generation). Research: `docs/research/rom-sprites.md`.
//!
//! Every sprite is stored as a 96x96 4 bpp image in four 1D sprite blocks (64x64, 32x64, 64x32,
//! 32x32), the layout of the Gen 5 front sprites, so the DS draws any sprite as four objects. Gen 4
//! sprites (80x80, scanned and encrypted) are converted when the cache is built; Gen 5 ones are
//! kept as the ROM's LZ-compressed NCGR.
//!
//! The cache is written front to back while the ROM is read (a Gen 4 cache doesn't fit in the
//! DS's memory), so its index comes last: sprite data, the icon block, the index, a footer.

use alloc::vec;
use alloc::vec::Vec;

use crate::icons::IconKey;
use crate::items::palette_n;
use crate::narc::Narc;
use crate::nds::{FileRange, Game, Rom};
use crate::{lz, u16_at, u32_at, Error, Result, Source};

/// Bytes of a decoded sprite: 96x96 pixels, 4 bpp.
pub const SPRITE_BYTES: usize = 96 * 96 / 2;
/// Bytes of a type or category icon: 32x16 pixels, 4 bpp, tiles in rows of four.
pub const ICON_BYTES: usize = 256;
/// Types in verity's numbering (Gen 5's: no ???).
pub const TYPES: usize = 17;
/// Categories: physical, special, status.
pub const CATEGORIES: usize = 3;

const NCGR_DATA: usize = 0x30;
/// The four blocks of a sprite: (x, y, width, height).
const BLOCKS: [(usize, usize, usize, usize); 4] = [
    (0, 0, 64, 64),
    (64, 0, 32, 64),
    (0, 64, 64, 32),
    (64, 64, 32, 32),
];

// ---- Gen 4 ---------------------------------------------------------------------------------

/// The byte offset in a 96x96 block-layout sprite of the 8-pixel tile row at (x, y), x a
/// multiple of 8.
fn tile_row(x: usize, y: usize) -> usize {
    let mut tile = 0;
    for (bx, by, bw, bh) in BLOCKS {
        if x >= bx && x < bx + bw && y >= by && y < by + bh {
            let (lx, ly) = (x - bx, y - by);
            return (tile + (ly / 8) * (bw / 8) + lx / 8) * 32 + (ly % 8) * 4;
        }
        tile += bw * bh / 64;
    }
    0
}

/// Undoes the Gen 4 sprite encryption: each u16 is XORed with an LCG stream seeded from the first
/// word, walking forwards (Platinum, HeartGold), or from the last, walking backwards (Diamond and
/// Pearl, by their tools; with `forward` None the direction that leaves more of the image
/// transparent wins). Then places frame 0 (the left 80x80 of the scanned 160x80 image) in the
/// 96x96 layout, centred across and standing on the bottom edge, where Gen 5 sprites stand.
fn gen4_sprite(ncgr: &[u8], forward: Option<bool>) -> Result<[u8; SPRITE_BYTES]> {
    let data = ncgr
        .get(NCGR_DATA..NCGR_DATA + 160 * 80 / 2)
        .ok_or(Error::Format("sprite NCGR"))?;
    let decrypt = |forward: bool| -> Vec<u8> {
        let mut out = data.to_vec();
        let n = out.len() / 2;
        let word = |i: usize| u16::from_le_bytes([data[2 * i], data[2 * i + 1]]);
        let mut key = u32::from(if forward { word(0) } else { word(n - 1) });
        for k in 0..n {
            let i = if forward { k } else { n - 1 - k };
            let v = word(i) ^ key as u16;
            out[2 * i..2 * i + 2].copy_from_slice(&v.to_le_bytes());
            key = key.wrapping_mul(0x4E6D).wrapping_add(0x6073);
        }
        out
    };
    let plain = match forward {
        Some(f) => decrypt(f),
        None => {
            let zeros = |b: &[u8]| b.iter().filter(|&&v| v == 0).count();
            let (f, b) = (decrypt(true), decrypt(false));
            if zeros(&f) >= zeros(&b) {
                f
            } else {
                b
            }
        }
    };
    let mut out = [0u8; SPRITE_BYTES];
    for y in 0..80 {
        for tx in 0..10 {
            let src = y * 80 + tx * 4;
            let dst = tile_row(8 + tx * 8, y + 16);
            out[dst..dst + 4].copy_from_slice(&plain[src..src + 4]);
        }
    }
    Ok(out)
}

/// Gen 4 alternate forms and eggs, from the form archive (`pl_otherpoke`): the front sprite of
/// form f is member `chr + f * chr_step`, its palette `pal + f * pal_step + shiny * shiny_step`.
/// pokeplatinum `BuildPokemonSpriteTemplate`, pokeheartgold `GetMonSpriteCharAndPlttNarcIdsEx`.
/// Palette numbers are Platinum's, whose archive has 154 sprites before its palettes;
/// HeartGold/SoulSilver's has 158 (Pichu's), so its palettes are 4 later.
struct Form4 {
    species: u16,
    forms: u8,
    chr: usize,
    chr_step: usize,
    pal: usize,
    pal_step: usize,
    shiny_step: usize,
}

const fn f4(
    species: u16,
    forms: u8,
    chr: usize,
    chr_step: usize,
    pal: usize,
    pal_step: usize,
    shiny_step: usize,
) -> Form4 {
    Form4 {
        species,
        forms,
        chr,
        chr_step,
        pal,
        pal_step,
        shiny_step,
    }
}

const FORMS4: [Form4; 12] = [
    f4(386, 4, 1, 2, 154, 0, 1),   // Deoxys
    f4(201, 28, 9, 2, 156, 0, 1),  // Unown
    f4(351, 4, 68, 1, 158, 1, 4),  // Castform
    f4(412, 3, 73, 2, 166, 2, 1),  // Burmy
    f4(413, 3, 79, 2, 172, 2, 1),  // Wormadam
    f4(422, 2, 86, 1, 178, 2, 1),  // Shellos
    f4(423, 2, 90, 1, 182, 2, 1),  // Gastrodon
    f4(421, 2, 94, 1, 186, 1, 2),  // Cherrim
    f4(493, 18, 97, 2, 190, 2, 1), // Arceus
    f4(492, 2, 135, 2, 228, 2, 1), // Shaymin
    f4(479, 6, 139, 2, 232, 2, 1), // Rotom
    f4(487, 2, 151, 2, 244, 2, 1), // Giratina
];
/// HeartGold/SoulSilver's spiky-eared Pichu.
const PICHU4: Form4 = f4(172, 2, 155, 2, 248, 2, 1);
/// Eggs: the egg, then Manaphy's.
const EGG4_CHR: usize = 132;
const EGG4_PAL: usize = 226;

/// The palette of an NCLR that may be LZ-compressed.
fn nclr(b: &[u8]) -> Result<[u16; 16]> {
    if matches!(b.first(), Some(0x10 | 0x11)) {
        palette_n(&lz::decompress(b)?, 0)
    } else {
        palette_n(b, 0)
    }
}

fn pal_bytes(normal: [u16; 16], shiny: [u16; 16]) -> [u8; 64] {
    let mut out = [0u8; 64];
    for (i, c) in normal.iter().chain(shiny.iter()).enumerate() {
        out[2 * i..2 * i + 2].copy_from_slice(&c.to_le_bytes());
    }
    out
}

// ---- LZ10 for the cache ---------------------------------------------------------------------

/// LZ10 with runs only (copies of the previous byte): sprites are mostly transparent, so this
/// shrinks them about fourfold in one pass, fast enough on the DS.
pub fn compress_runs(src: &[u8]) -> Vec<u8> {
    let n = src.len();
    let mut out = vec![0x10, n as u8, (n >> 8) as u8, (n >> 16) as u8];
    let mut i = 0;
    while i < n {
        let flags_at = out.len();
        out.push(0);
        for bit in (0..8).rev() {
            if i >= n {
                break;
            }
            let mut run = 0;
            if i > 0 {
                while run < 18 && i + run < n && src[i + run] == src[i - 1] {
                    run += 1;
                }
            }
            if run >= 3 {
                out[flags_at] |= 1 << bit;
                out.push(((run - 3) << 4) as u8);
                out.push(0);
                i += run;
            } else {
                out.push(src[i]);
                i += 1;
            }
        }
    }
    out
}

// ---- cache writer ---------------------------------------------------------------------------

const CACHE_MAGIC: &[u8; 4] = b"VSPR";
const CACHE_VERSION: u8 = 1;
/// Footer: magic, version, generation, entry count (u16), index offset (u32), icon block offset
/// (u32), the source ROM's game code, 4 reserved bytes.
pub const FOOTER: usize = 24;
const ENTRY: usize = 16;
/// Icon block: three palettes, then per type and per category a palette number and the tiles.
const ICON_BLOCK: usize = 3 * 32 + (TYPES + CATEGORIES) * (1 + ICON_BYTES);

const FLAG_FEMALE: u8 = 1;
const FLAG_EGG: u8 = 2;

struct Writer<'w, W: FnMut(&[u8]) -> Result<()>> {
    out: &'w mut W,
    pos: u32,
    index: Vec<(IconKey, u32, u32, u32)>,
}

impl<W: FnMut(&[u8]) -> Result<()>> Writer<'_, W> {
    fn write(&mut self, b: &[u8]) -> Result<u32> {
        let at = self.pos;
        (self.out)(b)?;
        self.pos += b.len() as u32;
        Ok(at)
    }

    fn sprite(&mut self, key: IconKey, pal: u32, tiles: u32, len: u32) {
        self.index.push((key, pal, tiles, len));
    }

    /// Writes the icon block, the index and the footer.
    fn finish(mut self, generation: u8, icons: &[u8], code: [u8; 4]) -> Result<usize> {
        let icons_at = if icons.is_empty() {
            0
        } else {
            self.write(icons)?
        };
        self.index.sort_by_key(|e| e.0);
        self.index.dedup_by_key(|e| e.0);
        let index_at = self.pos;
        let entries = core::mem::take(&mut self.index);
        for (k, pal, tiles, len) in &entries {
            let mut e = [0u8; ENTRY];
            e[0..2].copy_from_slice(&k.species.to_le_bytes());
            e[2] = k.form;
            e[3] = if k.female { FLAG_FEMALE } else { 0 } | if k.egg { FLAG_EGG } else { 0 };
            e[4..8].copy_from_slice(&pal.to_le_bytes());
            e[8..12].copy_from_slice(&tiles.to_le_bytes());
            e[12..16].copy_from_slice(&len.to_le_bytes());
            self.write(&e)?;
        }
        let mut f = [0u8; FOOTER];
        f[0..4].copy_from_slice(CACHE_MAGIC);
        f[4] = CACHE_VERSION;
        f[5] = generation;
        f[6..8].copy_from_slice(&(entries.len() as u16).to_le_bytes());
        f[8..12].copy_from_slice(&index_at.to_le_bytes());
        f[12..16].copy_from_slice(&icons_at.to_le_bytes());
        f[16..20].copy_from_slice(&code);
        self.write(&f)?;
        Ok(entries.len())
    }
}

/// Reads members `first..=last` of `narc` in one read (they are stored in order).
fn read_members<S: Source>(
    rom: &mut Rom<S>,
    narc: &Narc,
    first: usize,
    last: usize,
) -> Result<(Vec<u8>, u32)> {
    let a = narc.member(first)?;
    let b = narc.member(last)?;
    let bytes = rom.read(FileRange {
        start: a.start,
        end: b.end.max(a.start),
    })?;
    Ok((bytes, a.start))
}

fn slice(bytes: &[u8], base: u32, r: FileRange) -> Result<&[u8]> {
    bytes
        .get((r.start - base) as usize..(r.end - base) as usize)
        .ok_or(Error::Format("NARC member"))
}

/// Builds the sprite cache of the ROM's generation, handing it to `out` in pieces. Returns the
/// number of sprites.
pub fn build<S: Source, W: FnMut(&[u8]) -> Result<()>>(
    rom: &mut Rom<S>,
    out: &mut W,
) -> Result<usize> {
    let game = rom.game().ok_or(Error::NotFound)?;
    let code = rom.game_code();
    let mut w = Writer {
        out,
        pos: 0,
        index: Vec::new(),
    };
    let icons = if game.generation() == 4 {
        build4(rom, game, &mut w)?;
        icons4(rom, game).unwrap_or_default()
    } else {
        build5(rom, &mut w)?;
        icons5(rom, game).unwrap_or_default()
    };
    w.finish(game.generation(), &icons, code)
}

fn key(species: u16, form: u8, female: bool, egg: bool) -> IconKey {
    IconKey {
        species,
        form,
        female,
        egg,
    }
}

fn build4<S: Source, W: FnMut(&[u8]) -> Result<()>>(
    rom: &mut Rom<S>,
    game: Game,
    w: &mut Writer<'_, W>,
) -> Result<()> {
    let (main, other) = match game {
        Game::Platinum => (
            "poketool/pokegra/pl_pokegra.narc",
            Some("poketool/pokegra/pl_otherpoke.narc"),
        ),
        Game::HeartGold | Game::SoulSilver => ("a/0/0/4", Some("a/1/1/4")),
        // Diamond/Pearl's form archive is laid out differently (not checked: no ROM).
        _ => ("poketool/pokegra/pokegra.narc", None),
    };
    let hgss = matches!(game, Game::HeartGold | Game::SoulSilver);
    // Platinum and HeartGold/SoulSilver encrypt forwards; Diamond/Pearl is decided per sprite.
    let forward = other.map(|_| true);
    let in_other = |s: u16| {
        other.is_some() && (FORMS4.iter().any(|f| f.species == s) || (hgss && s == PICHU4.species))
    };
    let file = rom.find(main)?;
    let narc = Narc::open(rom.source(), file)?;
    // In archive order, so the card only ever seeks forwards (a FAT seek backwards walks the
    // cluster chain from the file's start).
    for species in 1..=493u16 {
        if in_other(species) {
            continue;
        }
        let base = 6 * usize::from(species);
        if base + 5 >= narc.len() {
            break;
        }
        // Female front, male front, normal palette, shiny palette.
        let (bytes, at) = read_members(rom, &narc, base + 2, base + 5)?;
        let m = |i: usize| slice(&bytes, at, narc.member(base + i)?);
        let pal = pal_bytes(nclr(m(4)?)?, nclr(m(5)?)?);
        let pal_at = w.write(&pal)?;
        // Single-gender species have only one of the two (the other member is empty); most
        // others store the same sprite twice.
        let (f, mm) = (m(2)?, m(3)?);
        let male = gen4_sprite(if mm.is_empty() { f } else { mm }, forward)?;
        let packed = compress_runs(&male);
        let tiles_at = w.write(&packed)?;
        w.sprite(
            key(species, 0, false, false),
            pal_at,
            tiles_at,
            packed.len() as u32,
        );
        if f.is_empty() || mm.is_empty() || f == mm {
            continue;
        }
        let female = gen4_sprite(f, forward)?;
        if female != male {
            let packed = compress_runs(&female);
            let tiles_at = w.write(&packed)?;
            w.sprite(
                key(species, 0, true, false),
                pal_at,
                tiles_at,
                packed.len() as u32,
            );
        }
    }
    let Some(other) = other else {
        return Ok(());
    };
    let file = rom.find(other)?;
    let narc = Narc::open(rom.source(), file)?;
    // Sprites (6448 bytes each) come first, then the palettes: read those in one go.
    let first_palette = (0..narc.len())
        .find(|&i| {
            narc.member(i)
                .is_ok_and(|r| !r.is_empty() && r.len() < 1024)
        })
        .ok_or(Error::Format("form NARC"))?;
    let shift = first_palette
        .checked_sub(154)
        .ok_or(Error::Format("form NARC"))?;
    let (pals, pals_at) = read_members(rom, &narc, first_palette, narc.len() - 1)?;
    let pal = |i: usize| nclr(slice(&pals, pals_at, narc.member(i + shift)?)?);
    // (key, sprite member, normal palette, shiny palette), read in sprite order.
    let mut todo: Vec<(IconKey, usize, usize, usize)> = Vec::new();
    let pichu = [PICHU4];
    let extra: &[Form4] = if hgss { &pichu } else { &[] };
    for f in FORMS4.iter().chain(extra) {
        for form in 0..f.forms {
            let fi = usize::from(form);
            let p = f.pal + fi * f.pal_step;
            todo.push((
                key(f.species, form, false, false),
                f.chr + fi * f.chr_step,
                p,
                p + f.shiny_step,
            ));
        }
    }
    // Eggs have no shiny palette: both halves are the egg's.
    todo.push((key(0, 0, false, true), EGG4_CHR, EGG4_PAL, EGG4_PAL));
    todo.push((
        key(490, 0, false, true),
        EGG4_CHR + 1,
        EGG4_PAL + 1,
        EGG4_PAL + 1,
    ));
    todo.sort_by_key(|t| t.1);
    for (k, chr, normal, shiny) in todo {
        let pal_at = w.write(&pal_bytes(pal(normal)?, pal(shiny)?))?;
        let sprite = narc.read_member(rom.source(), chr)?;
        let packed = compress_runs(&gen4_sprite(&sprite, forward)?);
        let tiles_at = w.write(&packed)?;
        w.sprite(k, pal_at, tiles_at, packed.len() as u32);
    }
    Ok(())
}

// ---- Gen 5 ---------------------------------------------------------------------------------

/// Files per species in the Gen 5 sprite archive: front (male, female), back, animation data,
/// then the normal and shiny palettes at 18 and 19.
const GROUP5: usize = 20;

/// Writes one group's sprites (male, and female when it has its own); returns (palette, tiles,
/// length) of the male sprite.
fn group5<S: Source, W: FnMut(&[u8]) -> Result<()>>(
    rom: &mut Rom<S>,
    narc: &Narc,
    w: &mut Writer<'_, W>,
    g: usize,
    k: IconKey,
) -> Result<Option<(u32, u32, u32)>> {
    // Only the fronts (0, 1) and palettes (18, 19): the rest is most of each group's bytes.
    let base = GROUP5 * g;
    let (fronts, at) = read_members(rom, narc, base, base + 1)?;
    let (pals, pals_at) = read_members(rom, narc, base + 18, base + 19)?;
    let m = |i: usize| slice(&fronts, at, narc.member(base + i)?);
    let p = |i: usize| slice(&pals, pals_at, narc.member(base + i)?);
    let front = m(0)?;
    if !matches!(front.first(), Some(0x10 | 0x11)) {
        return Ok(None);
    }
    let pal = pal_bytes(nclr(p(18)?)?, nclr(p(19)?)?);
    let pal_at = w.write(&pal)?;
    let tiles_at = w.write(front)?;
    let len = front.len() as u32;
    w.sprite(k, pal_at, tiles_at, len);
    let female = m(1)?;
    if matches!(female.first(), Some(0x10 | 0x11)) {
        let f_at = w.write(female)?;
        w.sprite(
            IconKey { female: true, ..k },
            pal_at,
            f_at,
            female.len() as u32,
        );
    }
    Ok(Some((pal_at, tiles_at, len)))
}

fn build5<S: Source, W: FnMut(&[u8]) -> Result<()>>(
    rom: &mut Rom<S>,
    w: &mut Writer<'_, W>,
) -> Result<()> {
    // Species data: form sprite offset (0x1E), form count (0x20), palette-only forms (0x21 bit 7);
    // the same table places the form icons (`icons::load5`).
    // Read whole: one seek instead of 650 (each costs a FAT chain walk on the DS).
    let personal_file = rom.find("a/0/1/6")?;
    let personal = rom.read(personal_file)?;
    let pnarc = Narc::open(
        &mut personal.as_slice(),
        FileRange {
            start: 0,
            end: personal_file.len(),
        },
    )?;
    let mut forms = Vec::with_capacity(650);
    let mut last_form = 0usize;
    for s in 0..650 {
        let r = slice(&personal, 0, pnarc.member(s)?)?;
        let offset = u16_at(r, 0x1E)?;
        let count = *r.get(0x20).ok_or(Error::Format("personal"))?;
        let palette_only = r.get(0x21).is_some_and(|b| b >> 7 != 0);
        if count > 1 && !palette_only {
            last_form = last_form.max(usize::from(offset) + usize::from(count) - 1);
        }
        forms.push((offset, count, palette_only));
    }
    drop(personal);
    let file = rom.find("a/0/0/4")?;
    let narc = Narc::open(rom.source(), file)?;
    // The archive ends with palettes for palette-only forms (Arceus's types, normal and shiny
    // each); before them, whole groups.
    let mut tail = narc.len();
    while tail > 0 {
        let r = narc.member(tail - 1)?;
        let mut magic = [0u8; 4];
        if r.len() < 4 {
            break;
        }
        rom.source().read_at(r.start, &mut magic)?;
        if &magic != b"RLCN" {
            break;
        }
        tail -= 1;
    }
    let groups = tail / GROUP5;
    let forms_start = groups
        .checked_sub(last_form)
        .ok_or(Error::Format("sprite NARC"))?;
    let egg = forms_start
        .checked_sub(2)
        .ok_or(Error::Format("sprite NARC"))?;
    for species in 1..650u16 {
        let (offset, count, palette_only) = forms[usize::from(species)];
        let base = group5(
            rom,
            &narc,
            w,
            usize::from(species),
            key(species, 0, false, false),
        )?;
        if count < 2 {
            continue;
        }
        if palette_only {
            // Arceus: the base sprite with a palette pair per form from the archive's tail.
            let Some((_, tiles_at, len)) = base else {
                continue;
            };
            let pals = narc.len() - tail;
            if pals < 2 * (usize::from(count) - 1) {
                continue;
            }
            for form in 1..count {
                let i = narc.len() - 2 * (usize::from(count) - 1) + 2 * usize::from(form - 1);
                let (bytes, at) = read_members(rom, &narc, i, i + 1)?;
                let normal = nclr(slice(&bytes, at, narc.member(i)?)?)?;
                let shiny = nclr(slice(&bytes, at, narc.member(i + 1)?)?)?;
                let pal_at = w.write(&pal_bytes(normal, shiny))?;
                w.sprite(key(species, form, false, false), pal_at, tiles_at, len);
            }
            continue;
        }
        for form in 1..count {
            let g = forms_start + usize::from(offset) + usize::from(form) - 1;
            if g < groups {
                group5(rom, &narc, w, g, key(species, form, false, false))?;
            }
        }
    }
    group5(rom, &narc, w, egg, key(0, 0, false, true))?;
    group5(rom, &narc, w, egg + 1, key(490, 0, false, true))?;
    Ok(())
}

// ---- type and category icons ----------------------------------------------------------------

/// Gen 5 (`app_menu_common`, pokebw2 `AppMenuCommon_*`): palettes in member 0x21, type t in
/// 0x22 + t, category c (status, physical, special) in 0x38 + c; the palette of each is fixed.
const TYPE_PALETTES5: [u8; TYPES] = [0, 0, 1, 1, 0, 0, 2, 1, 0, 0, 1, 2, 0, 1, 1, 2, 0];
const CATEGORY_PALETTES5: [u8; 3] = [0, 0, 1];
/// Gen 4 category palettes (pokeplatinum `sMoveCategoryIconPaletteIndex`): physical, special,
/// status.
const CATEGORY_PALETTES4: [u8; 3] = [0, 1, 0];
/// Start of the Gen 4 type palette table (`sMoveTypeIconPaletteIndex`, by Gen 4 type).
const TYPE_PALETTES4_START: [u8; 10] = [0, 0, 1, 1, 0, 0, 2, 1, 0, 2];

/// The 4 bpp tiles of a 32x16 icon (an NCGR, maybe LZ-compressed).
fn icon_tiles(b: &[u8]) -> Result<Vec<u8>> {
    let d = if matches!(b.first(), Some(0x10 | 0x11)) {
        lz::decompress(b)?
    } else {
        b.to_vec()
    };
    if d.get(..4) != Some(b"RGCN") {
        return Err(Error::Format("icon NCGR"));
    }
    d.get(NCGR_DATA..NCGR_DATA + ICON_BYTES)
        .map(<[u8]>::to_vec)
        .ok_or(Error::Format("icon NCGR"))
}

fn icon_block(
    palettes: &[[u16; 16]; 3],
    types: &[(u8, Vec<u8>)],
    categories: &[(u8, Vec<u8>)],
) -> Vec<u8> {
    let mut out = Vec::with_capacity(ICON_BLOCK);
    for p in palettes {
        for c in p {
            out.extend_from_slice(&c.to_le_bytes());
        }
    }
    for (pal, tiles) in types.iter().chain(categories) {
        out.push(*pal);
        out.extend_from_slice(tiles);
    }
    out
}

fn nclr3(b: &[u8]) -> Result<[[u16; 16]; 3]> {
    let d = if matches!(b.first(), Some(0x10 | 0x11)) {
        lz::decompress(b)?
    } else {
        b.to_vec()
    };
    Ok([palette_n(&d, 0)?, palette_n(&d, 1)?, palette_n(&d, 2)?])
}

/// Gen 4: the battle object archive, named by two tables in the ARM9 code: category members
/// (physical, special, status, u32 each) then type members (u32 by Gen 4 type, 18 types and 5
/// contest categories), then the type palette bytes. Found by the palette bytes.
fn icons4<S: Source>(rom: &mut Rom<S>, game: Game) -> Result<Vec<u8>> {
    let path = match game {
        Game::Platinum => "battle/graphic/pl_batt_obj.narc",
        Game::HeartGold | Game::SoulSilver => "a/0/0/8",
        _ => "battle/graphic/batt_obj.narc",
    };
    let code = rom.arm9()?;
    let bad = Error::Format("type icon table");
    let file = rom.find(path)?;
    let narc = Narc::open(rom.source(), file)?;
    let n = narc.len() as u32;
    let at = (0..code.len().saturating_sub(23))
        .find(|&i| {
            code[i..].starts_with(&TYPE_PALETTES4_START)
                && i >= 26 * 4
                && (0..26).all(|k| u32_at(&code, i - 4 * (26 - k)).is_ok_and(|v| v > 0 && v < n))
        })
        .ok_or(bad)?;
    let table = |k: usize| -> Result<usize> { Ok(u32_at(&code, at - 4 * (26 - k))? as usize) };
    // The icons sit together in the archive: one read.
    let members: Vec<usize> = (0..26).map(table).collect::<Result<_>>()?;
    let (lo, hi) = (
        *members.iter().min().ok_or(bad)?,
        *members.iter().max().ok_or(bad)?,
    );
    let (bytes, base) = read_members(rom, &narc, lo, hi)?;
    let icon = |m: usize| icon_tiles(slice(&bytes, base, narc.member(m)?)?);
    let mut types = Vec::with_capacity(TYPES);
    for t in 0..TYPES {
        let g4 = if t >= 9 { t + 1 } else { t };
        types.push((code[at + g4], icon(members[3 + g4])?));
    }
    let mut categories = Vec::with_capacity(CATEGORIES);
    for (c, pal) in CATEGORY_PALETTES4.iter().enumerate() {
        categories.push((*pal, icon(members[c])?));
    }
    let palettes = shared_palette4(rom, &narc, lo)?;
    Ok(icon_block(&palettes, &types, &categories))
}

/// The three-palette NCLR the Gen 4 type icons share (`type_icons_shared_NCLR`, member 74 in
/// Platinum and HeartGold): the nearest such member before the icons.
fn shared_palette4<S: Source>(
    rom: &mut Rom<S>,
    narc: &Narc,
    near: usize,
) -> Result<[[u16; 16]; 3]> {
    // Sized like one (header, TTLP of 0x78, maybe a PMCP; LZ-compressed ones are smaller), so
    // only a few members are read.
    for i in (0..near).rev() {
        if !narc
            .member(i)
            .is_ok_and(|r| (0x60..=0xC0).contains(&r.len()))
        {
            continue;
        }
        let b = narc.read_member(rom.source(), i)?;
        let d = if matches!(b.first(), Some(0x10 | 0x11)) {
            match lz::decompress(&b) {
                Ok(d) => d,
                Err(_) => continue,
            }
        } else {
            b
        };
        // A TTLP section of 0x78 bytes: exactly three 16-colour palettes.
        if d.get(..4) == Some(b"RLCN") && u32_at(&d, 0x14).is_ok_and(|l| l == 0x78) {
            return nclr3(&d);
        }
    }
    Err(Error::Format("type icon palette"))
}

fn icons5<S: Source>(rom: &mut Rom<S>, game: Game) -> Result<Vec<u8>> {
    let path = match game {
        Game::Black2 | Game::White2 => "a/0/8/2",
        _ => "a/0/8/3",
    };
    let file = rom.find(path)?;
    let narc = Narc::open(rom.source(), file)?;
    let (bytes, base) = read_members(rom, &narc, 0x21, 0x3A)?;
    let m = |i: usize| slice(&bytes, base, narc.member(i)?);
    let palettes = nclr3(m(0x21)?)?;
    let mut types = Vec::with_capacity(TYPES);
    for (t, pal) in TYPE_PALETTES5.iter().enumerate() {
        types.push((*pal, icon_tiles(m(0x22 + t)?)?));
    }
    // verity's order (physical, special, status) from Gen 5's (status, physical, special).
    let mut categories = Vec::with_capacity(CATEGORIES);
    for g5 in [1usize, 2, 0] {
        categories.push((CATEGORY_PALETTES5[g5], icon_tiles(m(0x38 + g5)?)?));
    }
    Ok(icon_block(&palettes, &types, &categories))
}

// ---- cache reader ---------------------------------------------------------------------------

/// The tail of a sprite cache (icon block, index and footer), which the DS keeps in memory;
/// sprite data is read from the file by offset.
pub struct CachedSprites<'a> {
    pub generation: u8,
    /// File offset of `tail[0]`.
    start: u32,
    tail: &'a [u8],
    index_at: u32,
    icons_at: u32,
    count: usize,
}

/// Where a sprite is in the cache file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct SpriteRef {
    /// 32 bytes of normal palette, then 32 of shiny.
    pub palette: u32,
    pub tiles: u32,
    pub tiles_len: u32,
}

impl<'a> CachedSprites<'a> {
    /// From the footer (the file's last `FOOTER` bytes) and the file length: the file offset
    /// where the tail starts, or None if this isn't a sprite cache.
    pub fn tail_start(footer: &[u8], file_len: u32) -> Option<u32> {
        if footer.len() != FOOTER || &footer[..4] != CACHE_MAGIC || footer[4] != CACHE_VERSION {
            return None;
        }
        let index_at = u32_at(footer, 8).ok()?;
        let icons_at = u32_at(footer, 12).ok()?;
        let start = if icons_at != 0 { icons_at } else { index_at };
        (start <= index_at && index_at < file_len).then_some(start)
    }

    pub fn parse(tail: &'a [u8], start: u32) -> Result<CachedSprites<'a>> {
        let bad = Error::Format("sprite cache");
        let f = tail
            .len()
            .checked_sub(FOOTER)
            .and_then(|o| tail.get(o..))
            .ok_or(bad)?;
        if Self::tail_start(f, start + tail.len() as u32) != Some(start) {
            return Err(bad);
        }
        let count = usize::from(u16_at(f, 6)?);
        let index_at = u32_at(f, 8)?;
        let icons_at = u32_at(f, 12)?;
        if (index_at - start) as usize + count * ENTRY + FOOTER != tail.len()
            || (icons_at != 0 && index_at - icons_at != ICON_BLOCK as u32)
        {
            return Err(bad);
        }
        Ok(CachedSprites {
            generation: f[5],
            start,
            tail,
            index_at,
            icons_at,
            count,
        })
    }

    pub fn len(&self) -> usize {
        self.count
    }

    pub fn is_empty(&self) -> bool {
        self.count == 0
    }

    fn entry(&self, i: usize) -> (IconKey, SpriteRef) {
        let o = (self.index_at - self.start) as usize + i * ENTRY;
        let e = &self.tail[o..o + ENTRY];
        let w = |k: usize| u32::from_le_bytes([e[k], e[k + 1], e[k + 2], e[k + 3]]);
        (
            key(
                u16::from_le_bytes([e[0], e[1]]),
                e[2],
                e[3] & FLAG_FEMALE != 0,
                e[3] & FLAG_EGG != 0,
            ),
            SpriteRef {
                palette: w(4),
                tiles: w(8),
                tiles_len: w(12),
            },
        )
    }

    fn exact(&self, k: IconKey) -> Option<SpriteRef> {
        let (mut lo, mut hi) = (0, self.count);
        while lo < hi {
            let mid = (lo + hi) / 2;
            let (mk, r) = self.entry(mid);
            match mk.cmp(&k) {
                core::cmp::Ordering::Less => lo = mid + 1,
                core::cmp::Ordering::Greater => hi = mid,
                core::cmp::Ordering::Equal => return Some(r),
            }
        }
        None
    }

    /// The sprite for a Pokémon: its form and gender's, else the male's, else the base form's.
    /// Eggs: `species` is 490 for Manaphy's egg, anything else for the egg.
    pub fn find(&self, k: IconKey) -> Option<SpriteRef> {
        if k.egg {
            let s = if k.species == 490 { 490 } else { 0 };
            return self
                .exact(key(s, 0, false, true))
                .or_else(|| self.exact(key(0, 0, false, true)));
        }
        self.exact(k)
            .or_else(|| self.exact(IconKey { female: false, ..k }))
            .or_else(|| self.exact(key(k.species, 0, k.female, false)))
            .or_else(|| self.exact(key(k.species, 0, false, false)))
    }

    /// A type icon (`kind` 0, verity's type number) or a category icon (`kind` 1: physical,
    /// special, status): tiles and colours.
    pub fn icon(&self, kind: u8, i: usize) -> Option<([u8; ICON_BYTES], [u16; 16])> {
        if self.icons_at == 0 {
            return None;
        }
        let slot = match kind {
            0 if i < TYPES => i,
            1 if i < CATEGORIES => TYPES + i,
            _ => return None,
        };
        let b = &self.tail[(self.icons_at - self.start) as usize..];
        let rec = 96 + slot * (1 + ICON_BYTES);
        let pal = usize::from(*b.get(rec)?);
        if pal > 2 {
            return None;
        }
        let mut tiles = [0u8; ICON_BYTES];
        tiles.copy_from_slice(b.get(rec + 1..rec + 1 + ICON_BYTES)?);
        let mut colors = [0u16; 16];
        for (k, c) in colors.iter_mut().enumerate() {
            *c = u16_at(b, 32 * pal + 2 * k).ok()?;
        }
        Some((tiles, colors))
    }
}

/// Decodes a sprite's tile data as read from the cache (LZ-compressed, with or without an NCGR
/// header) into the 96x96 block layout.
pub fn decode(blob: &[u8]) -> Result<[u8; SPRITE_BYTES]> {
    let d = lz::decompress(blob)?;
    let data = if d.get(..4) == Some(b"RGCN") {
        d.get(NCGR_DATA..)
    } else {
        Some(&d[..])
    };
    let data = data
        .and_then(|d| d.get(..SPRITE_BYTES))
        .ok_or(Error::Format("sprite"))?;
    let mut out = [0u8; SPRITE_BYTES];
    out.copy_from_slice(data);
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn runs_round_trip() {
        let mut src = vec![0u8; 4608];
        for (i, b) in src.iter_mut().enumerate().skip(1000).take(300) {
            *b = (i * 7) as u8;
        }
        let packed = compress_runs(&src);
        assert!(packed.len() < 1200, "{}", packed.len());
        assert_eq!(lz::decompress(&packed).unwrap(), src);
    }

    #[test]
    fn block_layout() {
        assert_eq!(tile_row(0, 0), 0);
        assert_eq!(tile_row(64, 0), 64 * 32); // block 2 starts at tile 64
        assert_eq!(tile_row(0, 64), 96 * 32); // block 3 at tile 96
        assert_eq!(tile_row(88, 95), SPRITE_BYTES - 4); // the last tile's last row
    }
}
