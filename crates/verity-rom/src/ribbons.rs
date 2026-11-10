//! Ribbon images (32x32, 4 bpp) from the summary screen, and verity's ribbon cache. Research:
//! `docs/research/rom-icons.md` (Ribbons).
//!
//! The summary screen's ribbon table (pokeplatinum `src/ribbon.c` `sRibbonDataTable`, pokebw2
//! `src/ov207/ribbon.c` `sRibbons`) gives each of the 80 ribbons a Pokémon data parameter, an NCGR
//! in the summary screen's archive and a palette of one shared NCLR. Rows are `{u32 param, u16
//! sprite, u16 palette, u16 name, u16 description}` (Gen 5 adds a category byte: 16 bytes); verity
//! finds the table by row `k` naming text line `k` and description `80 + k`. A parameter is a
//! base plus a bit of one of the save's three ribbon sets, which gives verity's list position.

use alloc::vec::Vec;

use crate::icons::TILES;
use crate::items::{frame, palette_n};
use crate::narc::Narc;
use crate::nds::{FileRange, Game, Rom};
use crate::{u16_at, u32_at, Error, Result, Source};

/// Ribbons in verity's list: the Sinnoh set (28 bits), the Hoenn set (32), Sinnoh contests (20).
pub const COUNT: usize = 80;
const PALETTES: usize = 5;

struct Layout {
    /// None: the ARM9; Some(n): overlay n.
    overlay: Option<u32>,
    row: usize,
    archive: &'static str,
    palette_member: usize,
    /// Parameters of bit 0 of the Sinnoh, Hoenn and contest sets.
    bases: [u32; 3],
}

fn layout(game: Game) -> Option<Layout> {
    let gen4 = |archive| Layout {
        overlay: None,
        row: 12,
        archive,
        palette_member: 136,
        bases: [25, 78, 123],
    };
    let gen5 = |overlay, archive, palette_member| Layout {
        overlay: Some(overlay),
        row: 16,
        archive,
        palette_member,
        bases: [25, 77, 120],
    };
    Some(match game {
        Game::Platinum => gen4("graphic/pl_pst_gra.narc"),
        Game::HeartGold | Game::SoulSilver => gen4("a/0/3/9"),
        Game::Black | Game::White => gen5(131, "a/0/7/8", 6),
        Game::Black2 | Game::White2 => gen5(207, "a/0/7/7", 8),
        // Not checked (no ROM): the archive and table may differ.
        Game::Diamond | Game::Pearl => return None,
    })
}

/// verity's list position of a ribbon parameter.
fn position(param: u32, bases: [u32; 3]) -> Option<usize> {
    let [s, h, c] = bases;
    let p = param;
    if (s..s + 28).contains(&p) {
        Some((p - s) as usize)
    } else if (h..h + 32).contains(&p) {
        Some(28 + (p - h) as usize)
    } else if (c..c + 20).contains(&p) {
        Some(60 + (p - c) as usize)
    } else {
        None
    }
}

fn find_table(code: &[u8], row: usize) -> Option<usize> {
    (0..code.len().saturating_sub(row * COUNT))
        .step_by(4)
        .find(|&at| {
            (0..5).all(|k| {
                u16_at(code, at + row * k + 8).ok() == Some(k as u16)
                    && u16_at(code, at + row * k + 10).ok() == Some(80 + k as u16)
            })
        })
}

/// The ribbon images of one ROM, by verity's list position.
pub struct RibbonIcons {
    palettes: Vec<[u16; 16]>,
    /// (palette, tile block) per list position.
    entries: [(u8, u16); COUNT],
    tiles: Vec<[u8; TILES]>,
}

/// Extracts the 80 ribbon images of a supported game.
pub fn load<S: Source>(rom: &mut Rom<S>) -> Result<RibbonIcons> {
    let game = rom.game().ok_or(Error::NotFound)?;
    let l = layout(game).ok_or(Error::NotFound)?;
    let bad = Error::Format("ribbon table");
    // (position, sprite member, palette) per ribbon.
    let mut rows = Vec::with_capacity(COUNT);
    {
        let code = match l.overlay {
            None => rom.arm9()?,
            Some(n) => rom.overlay(n)?,
        };
        let at = find_table(&code, l.row).ok_or(bad)?;
        for k in 0..COUNT {
            let o = at + l.row * k;
            let pos = position(u32_at(&code, o)?, l.bases).ok_or(bad)?;
            rows.push((pos, u16_at(&code, o + 4)?, u16_at(&code, o + 6)?));
        }
    }
    let file = rom.find(l.archive)?;
    let bytes = rom.read(file)?;
    let narc = Narc::open(
        &mut bytes.as_slice(),
        FileRange {
            start: 0,
            end: file.len(),
        },
    )?;
    let member = |i: usize| -> Result<&[u8]> {
        let r = narc.member(i)?;
        Ok(&bytes[r.start as usize..r.end as usize])
    };
    let nclr = member(l.palette_member)?;
    let palettes = (0..PALETTES)
        .map(|p| palette_n(nclr, p))
        .collect::<Result<Vec<_>>>()?;
    let mut set = RibbonIcons {
        palettes,
        entries: [(0xFF, 0); COUNT],
        tiles: Vec::new(),
    };
    let mut blocks: Vec<(u16, u16)> = Vec::new(); // (sprite member, block)
    for (pos, sprite, pal) in rows {
        if usize::from(pal) >= PALETTES || set.entries[pos].0 != 0xFF {
            return Err(bad);
        }
        let block = match blocks.iter().find(|b| b.0 == sprite) {
            Some(b) => b.1,
            None => {
                set.tiles.push(frame(member(usize::from(sprite))?)?);
                let b = (set.tiles.len() - 1) as u16;
                blocks.push((sprite, b));
                b
            }
        };
        set.entries[pos] = (pal as u8, block);
    }
    Ok(set)
}

impl RibbonIcons {
    pub fn tile_blocks(&self) -> usize {
        self.tiles.len()
    }

    /// Serializes as a ribbon cache (see [`CachedRibbons`]).
    pub fn to_cache(&self, source_game_code: [u8; 4]) -> Vec<u8> {
        let mut v =
            Vec::with_capacity(CACHE_HEADER + 32 * PALETTES + 4 * COUNT + TILES * self.tiles.len());
        v.extend_from_slice(CACHE_MAGIC);
        v.extend_from_slice(&[CACHE_VERSION, COUNT as u8, PALETTES as u8, 0]);
        v.extend_from_slice(&(self.tiles.len() as u16).to_le_bytes());
        v.extend_from_slice(&[0; 2]);
        v.extend_from_slice(&source_game_code);
        for p in &self.palettes {
            for c in p {
                v.extend_from_slice(&c.to_le_bytes());
            }
        }
        for &(p, t) in &self.entries {
            v.extend_from_slice(&[p, 0]);
            v.extend_from_slice(&t.to_le_bytes());
        }
        for t in &self.tiles {
            v.extend_from_slice(t);
        }
        v
    }
}

// ---- cache ---------------------------------------------------------------------------------

const CACHE_MAGIC: &[u8; 4] = b"VRIB";
const CACHE_VERSION: u8 = 1;
const CACHE_HEADER: usize = 16;

/// A ribbon cache read in place (about 28 KB, kept whole in memory). Layout, little-endian:
/// `"VRIB"`, version `u8` (1), ribbon count `u8` (80), palette count `u8`, reserved `u8`, tile
/// block count `u16`, reserved `u16`, source game code `[u8; 4]`; palettes (16 BGR555 each); per
/// ribbon in verity's list order its palette `u8`, reserved `u8` and tile block `u16`; 512-byte
/// tile blocks (4 bpp, 1D 32x32).
pub struct CachedRibbons<'a> {
    pub source_game_code: [u8; 4],
    b: &'a [u8],
    palettes: usize,
    tiles: usize,
}

impl<'a> CachedRibbons<'a> {
    pub fn parse(b: &'a [u8]) -> Result<CachedRibbons<'a>> {
        let bad = Error::Format("ribbon cache");
        if b.len() < CACHE_HEADER
            || &b[..4] != CACHE_MAGIC
            || b[4] != CACHE_VERSION
            || usize::from(b[5]) != COUNT
        {
            return Err(bad);
        }
        let (palettes, tiles) = (usize::from(b[6]), usize::from(u16_at(b, 8)?));
        if b.len() != CACHE_HEADER + 32 * palettes + 4 * COUNT + TILES * tiles {
            return Err(bad);
        }
        Ok(CachedRibbons {
            source_game_code: [b[12], b[13], b[14], b[15]],
            b,
            palettes,
            tiles,
        })
    }

    /// The image of the ribbon at list position `i`: 512 tile bytes and its 16 colours.
    pub fn icon(&self, i: usize) -> Option<(&'a [u8], [u16; 16])> {
        if i >= COUNT {
            return None;
        }
        let e = CACHE_HEADER + 32 * self.palettes + 4 * i;
        let (p, t) = (
            usize::from(self.b[e]),
            usize::from(u16_at(self.b, e + 2).ok()?),
        );
        if p >= self.palettes || t >= self.tiles {
            return None;
        }
        let mut pal = [0u16; 16];
        for (k, c) in pal.iter_mut().enumerate() {
            *c = u16_at(self.b, CACHE_HEADER + 32 * p + 2 * k).ok()?;
        }
        let at = CACHE_HEADER + 32 * self.palettes + 4 * COUNT + TILES * t;
        Some((&self.b[at..at + TILES], pal))
    }
}
