//! Box icons (32x32, 4 bpp, three or four 16-colour palettes) and verity's icon cache. Research and
//! file layouts: `docs/research/rom-icons.md`.
//!
//! The palette of each icon is a byte table in the ARM9 code. It is found by its start: species 0,
//! then Bulbasaur to Blastoise (palettes 1 1 1 0 0 0 0 2 2), which Gen 5 packs as male | female << 4.

use alloc::vec::Vec;

use crate::narc::Narc;
use crate::nds::{FileRange, Game, Rom};
use crate::{Error, Result, Source, u16_at, u32_at};

const ANCHOR4: [u8; 10] = [0, 1, 1, 1, 0, 0, 0, 0, 2, 2];
const ANCHOR5: [u8; 10] = [0, 0x11, 0x11, 0x11, 0, 0, 0, 0, 0x22, 0x22];
/// One 32x32 frame at 4 bpp, tiles in 1D order (the first half of each 32x64 NCGR).
pub const TILES: usize = 512;
const NCGR_DATA: usize = 0x30;
const SPECIES_MANAPHY: u16 = 490;

/// What an icon is for. `form` 0 is the base form.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct IconKey {
    pub species: u16,
    pub form: u8,
    pub female: bool,
    pub egg: bool,
}

impl IconKey {
    fn packed(self) -> u32 {
        u32::from(self.species)
            | u32::from(self.form) << 16
            | u32::from(self.female) << 24
            | u32::from(self.egg) << 25
    }
}

/// The icons of one ROM: palettes as BGR555 and, per key, a palette and a tile block.
pub struct IconSet {
    pub palettes: Vec<[u16; 16]>,
    entries: Vec<(u32, u8, u16)>,
    tiles: Vec<[u8; TILES]>,
    /// FNV-1a of each tile block, so de-duplication stays fast on the DS.
    hashes: Vec<u32>,
}

fn fnv(b: &[u8]) -> u32 {
    b.iter().fold(0x811C_9DC5u32, |h, &x| {
        (h ^ u32::from(x)).wrapping_mul(0x0100_0193)
    })
}

/// Gen 4 icon index for a form (`pokeplatinum res/pokemon/meson.build`, `pokeheartgold
/// src/pokemon_icon_idx.c`): after the 494 species come the egg, Manaphy's egg, then forms.
fn gen4_form_base(species: u16) -> Option<(u16, u8)> {
    // (index of form 1, form count)
    Some(match species {
        386 => (496, 4),  // Deoxys
        201 => (500, 28), // Unown (form 0 is A, the species' own icon)
        412 => (527, 3),  // Burmy
        413 => (529, 3),  // Wormadam
        422 => (531, 2),  // Shellos
        423 => (532, 2),  // Gastrodon
        487 => (533, 2),  // Giratina
        492 => (534, 2),  // Shaymin
        479 => (535, 6),  // Rotom
        _ => return None,
    })
}

fn find_table<'a>(code: &'a [u8], anchor: &[u8], len: usize) -> Result<&'a [u8]> {
    let at = code
        .windows(anchor.len())
        .position(|w| w == anchor)
        .ok_or(Error::Format("icon palette table"))?;
    code.get(at..at + len)
        .ok_or(Error::Format("icon palette table"))
}

fn palettes(nclr: &[u8], count: usize) -> Result<Vec<[u16; 16]>> {
    let at = nclr
        .windows(4)
        .position(|w| w == b"TTLP")
        .ok_or(Error::Format("NCLR"))?;
    let data = at + 0x18;
    let mut out = Vec::with_capacity(count);
    for p in 0..count {
        let mut pal = [0u16; 16];
        for (i, c) in pal.iter_mut().enumerate() {
            *c = u16_at(nclr, data + 32 * p + 2 * i)?;
        }
        out.push(pal);
    }
    Ok(out)
}

fn frame(ncgr: &[u8]) -> Result<[u8; TILES]> {
    if ncgr.get(..4) != Some(b"RGCN") {
        return Err(Error::Format("NCGR"));
    }
    let src = ncgr
        .get(NCGR_DATA..NCGR_DATA + TILES)
        .ok_or(Error::Format("NCGR"))?;
    let mut out = [0u8; TILES];
    out.copy_from_slice(src);
    Ok(out)
}

impl IconSet {
    fn push(&mut self, key: IconKey, palette: u8, tiles: [u8; TILES]) {
        let h = fnv(&tiles);
        let found = (0..self.tiles.len()).find(|&i| self.hashes[i] == h && self.tiles[i] == tiles);
        let slot = found.unwrap_or_else(|| {
            self.tiles.push(tiles);
            self.hashes.push(h);
            self.tiles.len() - 1
        });
        self.entries.push((key.packed(), palette, slot as u16));
    }

    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn tile_blocks(&self) -> usize {
        self.tiles.len()
    }

    /// Serializes as an icon cache (see [`CachedIcons`]).
    pub fn to_cache(&self, source_game_code: [u8; 4]) -> Vec<u8> {
        let mut entries = self.entries.clone();
        entries.sort_unstable_by_key(|e| e.0);
        entries.dedup_by_key(|e| e.0);
        let n = entries.len();
        let tiles_at = CACHE_HEADER + 32 * self.palettes.len() + 7 * n;
        let mut v = Vec::with_capacity(tiles_at + TILES * self.tiles.len());
        v.extend_from_slice(CACHE_MAGIC);
        v.extend_from_slice(&[CACHE_VERSION, self.palettes.len() as u8]);
        v.extend_from_slice(&(n as u16).to_le_bytes());
        v.extend_from_slice(&(tiles_at as u32).to_le_bytes());
        v.extend_from_slice(&source_game_code);
        for p in &self.palettes {
            for c in p {
                v.extend_from_slice(&c.to_le_bytes());
            }
        }
        for e in &entries {
            v.extend_from_slice(&e.0.to_le_bytes());
        }
        for e in &entries {
            v.push(e.1);
            v.extend_from_slice(&e.2.to_le_bytes());
        }
        for t in &self.tiles {
            v.extend_from_slice(t);
        }
        v
    }
}

/// Extracts every icon of a supported game.
pub fn load<S: Source>(rom: &mut Rom<S>) -> Result<IconSet> {
    let game = rom.game().ok_or(Error::NotFound)?;
    if game.generation() == 4 {
        load4(rom, game)
    } else {
        load5(rom)
    }
}

/// Reads a whole file into memory: one seek per file, which matters on a FAT card holding a
/// 512 MB ROM.
fn read_narc<S: Source>(rom: &mut Rom<S>, path: &str) -> Result<(Vec<u8>, Narc)> {
    let file = rom.find(path)?;
    let bytes = rom.read(file)?;
    let narc = Narc::open(
        &mut bytes.as_slice(),
        FileRange {
            start: 0,
            end: file.len(),
        },
    )?;
    Ok((bytes, narc))
}

/// The palette table, copied out so the ARM9 code is freed before the icons are read.
fn table<S: Source>(rom: &mut Rom<S>, anchor: &[u8], len: usize) -> Result<Vec<u8>> {
    let code = rom.arm9()?;
    Ok(find_table(&code, anchor, len)?.to_vec())
}

fn load4<S: Source>(rom: &mut Rom<S>, game: Game) -> Result<IconSet> {
    let path = match game {
        Game::Platinum => "poketool/icongra/pl_poke_icon.narc",
        Game::HeartGold | Game::SoulSilver => "a/0/2/0",
        _ => "poketool/icongra/poke_icon.narc",
    };
    let shared = 7;
    let count = {
        let file = rom.find(path)?;
        Narc::open(rom.source(), file)?
            .len()
            .checked_sub(shared)
            .ok_or(Error::Format("icon NARC"))?
    };
    let table = table(rom, &ANCHOR4, count)?;
    let (bytes, narc) = read_narc(rom, path)?;
    let member = |i: usize| -> Result<&[u8]> {
        let r = narc.member(i)?;
        Ok(&bytes[r.start as usize..r.end as usize])
    };
    let mut set = IconSet {
        palettes: palettes(member(0)?, 3)?,
        entries: Vec::new(),
        tiles: Vec::new(),
        hashes: Vec::new(),
    };
    let mut add = |key: IconKey, index: usize| -> Result<()> {
        if index < count {
            set.push(key, table[index] & 3, frame(member(shared + index)?)?);
        }
        Ok(())
    };
    let none = |species: u16| IconKey {
        species,
        form: 0,
        female: false,
        egg: false,
    };
    for species in 1..=493u16 {
        add(none(species), usize::from(species))?;
        if let Some((base, forms)) = gen4_form_base(species) {
            for form in 1..forms {
                add(
                    IconKey {
                        form,
                        ..none(species)
                    },
                    usize::from(base) + usize::from(form) - 1,
                )?;
            }
        }
    }
    add(
        IconKey {
            egg: true,
            ..none(0)
        },
        494,
    )?;
    add(
        IconKey {
            egg: true,
            ..none(SPECIES_MANAPHY)
        },
        495,
    )?;
    Ok(set)
}

fn load5<S: Source>(rom: &mut Rom<S>) -> Result<IconSet> {
    let (personal, pnarc) = read_narc(rom, "a/0/1/6")?;
    // (sprite offset, form count, palette-only forms) for species 0..=649.
    let mut forms = Vec::with_capacity(650);
    let mut form_icons = 0usize;
    for s in 0..650 {
        let m = pnarc.member(s)?;
        let r = &personal[m.start as usize..m.end as usize];
        let offset = u16_at(r, 0x1E)?;
        let count = *r.get(0x20).ok_or(Error::Format("personal"))?;
        let palette_only = r.get(0x21).is_some_and(|b| b >> 7 != 0);
        if count > 1 && !palette_only {
            form_icons = form_icons.max(usize::from(offset) + usize::from(count) - 1);
        }
        forms.push((offset, count, palette_only));
    }
    drop(personal);

    let path = "a/0/0/7";
    let members = {
        let file = rom.find(path)?;
        Narc::open(rom.source(), file)?.len()
    };
    let (bytes, narc) = read_narc(rom, path)?;
    let member = |i: usize| -> Result<&[u8]> {
        let r = narc.member(i)?;
        Ok(&bytes[r.start as usize..r.end as usize])
    };
    // Shared files come first (one or two palettes, then cells and animations).
    let shared = (0..members.min(16))
        .find(|&i| member(i).is_ok_and(|m| m.starts_with(b"RGCN")))
        .ok_or(Error::Format("icon NARC"))?;
    let groups = (members - shared) / 2;
    let forms_start = groups
        .checked_sub(form_icons)
        .ok_or(Error::Format("icon NARC"))?;
    let egg = forms_start
        .checked_sub(2)
        .ok_or(Error::Format("icon NARC"))?;
    let pal = palettes(member(0)?, 3)?;
    drop(bytes);
    let table = table(rom, &ANCHOR5, groups + 2)?;
    // From the eggs on, Black 2/White 2's table has two more entries than icon groups (Black/White
    // has none): find the shift by the eggs' palettes, 1 for the egg and 2 for Manaphy's.
    let shift = (0..=2)
        .find(|&k| table[egg + k] & 0xF == 1 && table[egg + k + 1] & 0xF == 2)
        .ok_or(Error::Format("icon palette table"))?;
    let pal_of = |group: usize, female: bool| {
        let b = table[group + if group >= egg { shift } else { 0 }];
        if female { b >> 4 } else { b & 0xF }
    };
    let (bytes, narc) = read_narc(rom, path)?;
    let member = |i: usize| -> Result<&[u8]> {
        let r = narc.member(i)?;
        Ok(&bytes[r.start as usize..r.end as usize])
    };
    let mut set = IconSet {
        palettes: pal,
        entries: Vec::new(),
        tiles: Vec::new(),
        hashes: Vec::new(),
    };
    let mut add = |key: IconKey, group: usize| -> Result<()> {
        let index = shared + 2 * group + usize::from(key.female);
        if group >= groups || member(index)?.is_empty() {
            return Ok(());
        }
        set.push(key, pal_of(group, key.female) & 3, frame(member(index)?)?);
        Ok(())
    };
    for species in 1..650u16 {
        let (offset, count, palette_only) = forms[usize::from(species)];
        for female in [false, true] {
            let key = IconKey {
                species,
                form: 0,
                female,
                egg: false,
            };
            add(key, usize::from(species))?;
            if !palette_only {
                for form in 1..count {
                    add(
                        IconKey { form, ..key },
                        forms_start + usize::from(offset) + usize::from(form) - 1,
                    )?;
                }
            }
        }
    }
    let egg_key = IconKey {
        species: 0,
        form: 0,
        female: false,
        egg: true,
    };
    add(egg_key, egg)?;
    add(
        IconKey {
            species: SPECIES_MANAPHY,
            ..egg_key
        },
        egg + 1,
    )?;
    Ok(set)
}

// ---- cache ---------------------------------------------------------------------------------

const CACHE_MAGIC: &[u8; 4] = b"VICN";
const CACHE_VERSION: u8 = 1;
const CACHE_HEADER: usize = 16;

/// The head of an icon cache file (everything before the tiles, which stay on the SD card and are
/// read per icon). Layout, little-endian: `"VICN"`, version `u8` (1), palette count `u8`, entry
/// count `u16`, tiles offset `u32`, source game code `[u8; 4]`; palettes (16 BGR555 each); sorted
/// keys (`u32`: species | form << 16 | female << 24 | egg << 25); per entry palette `u8` and tile
/// block `u16`; then 512-byte tile blocks.
pub struct CachedIcons<'a> {
    pub palette_count: u8,
    pub tiles_offset: u32,
    pub source_game_code: [u8; 4],
    b: &'a [u8],
    n: usize,
}

/// Where to find one icon: its palette and the file offset of its 512 tile bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IconRef {
    pub palette: u8,
    pub tiles_at: u32,
}

impl<'a> CachedIcons<'a> {
    /// Bytes needed from the start of the file to call [`CachedIcons::parse`].
    pub fn head_len(header: &[u8]) -> Option<usize> {
        (header.len() >= CACHE_HEADER && &header[..4] == CACHE_MAGIC)
            .then(|| u32_at(header, 8).ok())
            .flatten()
            .map(|v| v as usize)
    }

    /// Parses the head (at least `tiles_offset` bytes).
    pub fn parse(b: &'a [u8]) -> Result<CachedIcons<'a>> {
        let bad = Error::Format("icon cache");
        if b.len() < CACHE_HEADER || &b[..4] != CACHE_MAGIC || b[4] != CACHE_VERSION {
            return Err(bad);
        }
        let (pals, n, tiles_offset) = (b[5], usize::from(u16_at(b, 6)?), u32_at(b, 8)?);
        if pals == 0
            || pals > 16
            || tiles_offset as usize != CACHE_HEADER + 32 * usize::from(pals) + 7 * n
            || b.len() < tiles_offset as usize
        {
            return Err(bad);
        }
        Ok(CachedIcons {
            palette_count: pals,
            tiles_offset,
            source_game_code: [b[12], b[13], b[14], b[15]],
            b,
            n,
        })
    }

    pub fn len(&self) -> usize {
        self.n
    }

    pub fn is_empty(&self) -> bool {
        self.n == 0
    }

    pub fn palette(&self, p: u8) -> Option<[u16; 16]> {
        if p >= self.palette_count {
            return None;
        }
        let mut out = [0u16; 16];
        for (i, c) in out.iter_mut().enumerate() {
            *c = u16_at(self.b, CACHE_HEADER + 32 * usize::from(p) + 2 * i).ok()?;
        }
        Some(out)
    }

    fn exact(&self, key: u32) -> Option<IconRef> {
        let keys = CACHE_HEADER + 32 * usize::from(self.palette_count);
        let (mut lo, mut hi) = (0, self.n);
        while lo < hi {
            let mid = (lo + hi) / 2;
            let k = u32_at(self.b, keys + 4 * mid).ok()?;
            if k == key {
                let e = keys + 4 * self.n + 3 * mid;
                let block = u16_at(self.b, e + 1).ok()?;
                return Some(IconRef {
                    palette: self.b[e],
                    tiles_at: self.tiles_offset + TILES as u32 * u32::from(block),
                });
            }
            if k < key {
                lo = mid + 1;
            } else {
                hi = mid;
            }
        }
        None
    }

    /// The icon for a Pokémon, falling back to the male icon, then the base form; eggs use the
    /// egg icon (Manaphy's own).
    pub fn find(&self, key: IconKey) -> Option<IconRef> {
        if key.egg {
            let species = if key.species == SPECIES_MANAPHY {
                SPECIES_MANAPHY
            } else {
                0
            };
            let egg = IconKey {
                species,
                form: 0,
                female: false,
                egg: true,
            };
            return self
                .exact(egg.packed())
                .or_else(|| self.exact(IconKey { species: 0, ..egg }.packed()));
        }
        let plain = IconKey { egg: false, ..key };
        [
            plain,
            IconKey {
                female: false,
                ..plain
            },
            IconKey { form: 0, ..plain },
            IconKey {
                form: 0,
                female: false,
                ..plain
            },
        ]
        .iter()
        .find_map(|k| self.exact(k.packed()))
    }
}
