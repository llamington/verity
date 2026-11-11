//! Held item icons (32x32, 4 bpp, one 16-colour palette each) and verity's item icon cache.
//! Research: `docs/research/rom-icons.md` (Item icons).
//!
//! Each item's icon and palette are members of one NARC, named by a table in the ARM9 code
//! (pokeplatinum `sItemArchiveIDs`, pokebw2 `ITEM_GRAPHICS_CHARS_AND_PALETTES`). verity finds the
//! table by its start: Gen 4 rows are `{data, icon, palette, gen3 item}` with item 0 pointing past
//! the item icons (`{0, n, n + 1, 0}`), then `{1, 2, 3, 1}`, `{2, 4, 5, 2}`; Gen 5 rows are
//! `{icon, palette}`, `{n, n + 1}` then `{2, 3}`, `{4, 5}`, `{6, 7}`.

use alloc::vec;
use alloc::vec::Vec;

use crate::icons::TILES;
use crate::narc::Narc;
use crate::nds::{FileRange, Game, Rom};
use crate::{Error, Result, Source, u16_at};

const NCGR_DATA: usize = 0x30;
const NONE: u16 = 0xFFFF;

/// The item icons of one ROM. Item IDs differ between Gen 4 and Gen 5, so a set serves one
/// generation.
pub struct ItemIcons {
    pub generation: u8,
    /// Per item ID from 0: (palette, tile block), `NONE` when the item has no icon.
    entries: Vec<(u16, u16)>,
    palettes: Vec<[u16; 16]>,
    tiles: Vec<[u8; TILES]>,
}

/// The last item ID of each game (PKHeX `MaxItemID_*`); the table is read no further.
fn last_item(game: Game) -> usize {
    match game {
        Game::Diamond | Game::Pearl => 464,
        Game::Platinum => 467,
        Game::HeartGold | Game::SoulSilver => 536,
        Game::Black | Game::White => 632,
        Game::Black2 | Game::White2 => 638,
    }
}

/// Finds the table and returns (icon member, palette member) per item, stopping at the first row
/// that points outside the archive (White's table ends at item 626).
fn table(code: &[u8], gen4: bool, members: usize, last: usize) -> Result<Vec<(u16, u16)>> {
    let bad = Error::Format("item icon table");
    let row = if gen4 { 8 } else { 4 };
    let n = members as u16;
    let at16 = |o: usize| u16_at(code, o).ok();
    let matches = |at: usize| -> bool {
        let f = |item: usize, field: usize| at16(at + row * item + 2 * field);
        if gen4 {
            (0..3).all(|i| f(i, 0) == Some(i as u16))
                && f(0, 1).is_some_and(|v| v >= 0x100 && v < n)
                && f(0, 2) == f(0, 1).map(|v| v + 1)
                && (1..3).all(|i| {
                    f(i, 1) == Some(2 * i as u16)
                        && f(i, 2) == Some(2 * i as u16 + 1)
                        && f(i, 3) == Some(i as u16)
                })
        } else {
            f(0, 0).is_some_and(|v| v >= 0x100 && v < n)
                && f(0, 1) == f(0, 0).map(|v| v + 1)
                && (1..4)
                    .all(|i| f(i, 0) == Some(2 * i as u16) && f(i, 1) == Some(2 * i as u16 + 1))
        }
    };
    let start = (0..code.len().saturating_sub(4 * row))
        .step_by(2)
        .find(|&at| matches(at))
        .ok_or(bad)?;
    let (icon, pal) = if gen4 { (2, 4) } else { (0, 2) };
    let mut out = Vec::with_capacity(last + 1);
    for item in 0..=last {
        let o = start + row * item;
        match (at16(o + icon), at16(o + pal)) {
            (Some(i), Some(p)) if i < n && p < n => out.push((i, p)),
            _ => break,
        }
    }
    Ok(out)
}

fn palette(nclr: &[u8]) -> Result<[u16; 16]> {
    palette_n(nclr, 0)
}

/// Palette `p` (16 BGR555 colours) of an NCLR.
pub(crate) fn palette_n(nclr: &[u8], p: usize) -> Result<[u16; 16]> {
    let at = nclr
        .windows(4)
        .position(|w| w == b"TTLP")
        .ok_or(Error::Format("NCLR"))?;
    let mut pal = [0u16; 16];
    for (i, c) in pal.iter_mut().enumerate() {
        *c = u16_at(nclr, at + 0x18 + 32 * p + 2 * i)?;
    }
    Ok(pal)
}

/// The first 32x32 4 bpp frame of an NCGR.
pub(crate) fn frame(ncgr: &[u8]) -> Result<[u8; TILES]> {
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

/// Extracts every held item icon of a supported game.
pub fn load<S: Source>(rom: &mut Rom<S>) -> Result<ItemIcons> {
    let game = rom.game().ok_or(Error::NotFound)?;
    let gen4 = game.generation() == 4;
    let path = match game {
        Game::HeartGold | Game::SoulSilver => "a/0/1/8",
        Game::Diamond | Game::Pearl | Game::Platinum => "itemtool/itemdata/item_icon.narc",
        _ => "a/0/2/5",
    };
    let file = rom.find(path)?;
    let members = Narc::open(rom.source(), file)?.len();
    let rows = {
        let code = rom.arm9()?;
        table(&code, gen4, members, last_item(game))?
    };
    let bytes = rom.read(file)?;
    let narc = Narc::open(
        &mut bytes.as_slice(),
        FileRange {
            start: 0,
            end: file.len(),
        },
    )?;
    let member = |i: u16| -> Result<&[u8]> {
        let r = narc.member(usize::from(i))?;
        Ok(&bytes[r.start as usize..r.end as usize])
    };
    // Archive member -> block, so shared icons (TMs, Plates) and palettes are stored once.
    let mut tile_of = vec![NONE; members];
    let mut pal_of = vec![NONE; members];
    let mut set = ItemIcons {
        generation: game.generation(),
        entries: Vec::with_capacity(rows.len()),
        palettes: Vec::new(),
        tiles: Vec::new(),
    };
    for (item, &(icon, pal)) in rows.iter().enumerate() {
        // Item 0 shows the empty icon; verity draws nothing for no item.
        let parsed = (item > 0)
            .then(|| -> Result<()> {
                if tile_of[usize::from(icon)] == NONE {
                    set.tiles.push(frame(member(icon)?)?);
                    tile_of[usize::from(icon)] = (set.tiles.len() - 1) as u16;
                }
                if pal_of[usize::from(pal)] == NONE {
                    set.palettes.push(palette(member(pal)?)?);
                    pal_of[usize::from(pal)] = (set.palettes.len() - 1) as u16;
                }
                Ok(())
            })
            .transpose()?;
        set.entries.push(match parsed {
            Some(()) => (pal_of[usize::from(pal)], tile_of[usize::from(icon)]),
            None => (NONE, NONE),
        });
    }
    if set.tiles.is_empty() {
        return Err(Error::Format("item icons"));
    }
    Ok(set)
}

impl ItemIcons {
    /// Item IDs covered (0 to `len() - 1`).
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    pub fn tile_blocks(&self) -> usize {
        self.tiles.len()
    }

    /// Serializes as an item icon cache (see [`CachedItems`]).
    pub fn to_cache(&self, source_game_code: [u8; 4]) -> Vec<u8> {
        let n = self.entries.len();
        let pals_at = CACHE_HEADER + 4 * n;
        let mut v =
            Vec::with_capacity(pals_at + 32 * self.palettes.len() + TILES * self.tiles.len());
        v.extend_from_slice(CACHE_MAGIC);
        v.extend_from_slice(&[CACHE_VERSION, self.generation]);
        v.extend_from_slice(&(n as u16).to_le_bytes());
        v.extend_from_slice(&(self.palettes.len() as u16).to_le_bytes());
        v.extend_from_slice(&(self.tiles.len() as u16).to_le_bytes());
        v.extend_from_slice(&source_game_code);
        for &(p, t) in &self.entries {
            v.extend_from_slice(&p.to_le_bytes());
            v.extend_from_slice(&t.to_le_bytes());
        }
        for p in &self.palettes {
            for c in p {
                v.extend_from_slice(&c.to_le_bytes());
            }
        }
        for t in &self.tiles {
            v.extend_from_slice(t);
        }
        v
    }
}

// ---- cache ---------------------------------------------------------------------------------

const CACHE_MAGIC: &[u8; 4] = b"VITM";
const CACHE_VERSION: u8 = 1;
const CACHE_HEADER: usize = 16;

/// The head of an item icon cache (header and index; palettes and tiles stay on the SD card).
/// Layout, little-endian: `"VITM"`, version `u8` (1), generation `u8`, item count `u16`, palette
/// count `u16`, tile block count `u16`, source game code `[u8; 4]`; per item ID from 0 its palette
/// `u16` and tile block `u16` (0xFFFF: no icon); palettes (16 BGR555 each); 512-byte tile blocks
/// (4 bpp, 1D 32x32).
pub struct CachedItems<'a> {
    pub generation: u8,
    pub source_game_code: [u8; 4],
    b: &'a [u8],
    n: usize,
    palettes: usize,
    tiles: usize,
}

/// Where one item's icon is in the cache file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ItemRef {
    /// File offset of 16 BGR555 colours.
    pub palette_at: u32,
    /// File offset of 512 bytes of tiles.
    pub tiles_at: u32,
}

impl<'a> CachedItems<'a> {
    /// Bytes needed from the start of the file to call [`CachedItems::parse`] (the header and
    /// index), from its first 16 bytes.
    pub fn head_len(header: &[u8]) -> Option<usize> {
        (header.len() >= CACHE_HEADER && &header[..4] == CACHE_MAGIC)
            .then(|| u16_at(header, 6).ok())
            .flatten()
            .map(|n| CACHE_HEADER + 4 * usize::from(n))
    }

    /// Parses the head (at least [`CachedItems::head_len`] bytes).
    pub fn parse(b: &'a [u8]) -> Result<CachedItems<'a>> {
        let bad = Error::Format("item icon cache");
        if b.len() < CACHE_HEADER
            || &b[..4] != CACHE_MAGIC
            || b[4] != CACHE_VERSION
            || !(4..=5).contains(&b[5])
        {
            return Err(bad);
        }
        let n = usize::from(u16_at(b, 6)?);
        if b.len() < CACHE_HEADER + 4 * n {
            return Err(bad);
        }
        Ok(CachedItems {
            generation: b[5],
            source_game_code: [b[12], b[13], b[14], b[15]],
            b,
            n,
            palettes: usize::from(u16_at(b, 8)?),
            tiles: usize::from(u16_at(b, 10)?),
        })
    }

    pub fn len(&self) -> usize {
        self.n
    }

    pub fn is_empty(&self) -> bool {
        self.n == 0
    }

    pub fn find(&self, item: u16) -> Option<ItemRef> {
        let i = usize::from(item);
        if i >= self.n {
            return None;
        }
        let p = usize::from(u16_at(self.b, CACHE_HEADER + 4 * i).ok()?);
        let t = usize::from(u16_at(self.b, CACHE_HEADER + 4 * i + 2).ok()?);
        if p >= self.palettes || t >= self.tiles {
            return None;
        }
        let pals_at = CACHE_HEADER + 4 * self.n;
        let tiles_at = pals_at + 32 * self.palettes;
        Some(ItemRef {
            palette_at: (pals_at + 32 * p) as u32,
            tiles_at: (tiles_at + TILES * t) as u32,
        })
    }
}
