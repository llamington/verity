//! Move stats the save data doesn't carry (category, power, accuracy), from the ROM's move table,
//! and verity's move cache. Type and PP come from PKHeX's tables in the core crate instead.
//!
//! The values differ between generations (Tackle is 35/95 in Gen 4, 50/100 in Gen 5), so a cache
//! records the generation it came from and is only used for saves of that generation.
//!
//! Gen 4 (`pokeplatinum` `MoveTable`, 16 bytes): effect `u16`, class, power, type, accuracy, PP, ...
//! Class: 0 physical, 1 special, 2 status. Gen 5 (`a/0/2/1`, 36 bytes): type, quality, category,
//! power, accuracy, PP, ...; category: 0 status, 1 physical, 2 special.

use alloc::vec::Vec;

use crate::narc::Narc;
use crate::nds::{Game, Rom};
use crate::{u16_at, Error, Result, Source};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Category {
    Physical = 0,
    Special = 1,
    Status = 2,
}

/// One move's stats as the games store them: power 0 means none, 1 varies; accuracy 0 (Gen 4)
/// or 101 (Gen 5) never misses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MoveStats {
    pub category: Category,
    pub power: u8,
    pub accuracy: u8,
    pub pp: u8,
}

pub struct MoveTable {
    pub generation: u8,
    moves: Vec<MoveStats>,
}

fn location(game: Game) -> &'static str {
    match game {
        Game::Diamond | Game::Pearl => "poketool/waza/waza_tbl.narc",
        Game::Platinum => "poketool/waza/pl_waza_tbl.narc",
        Game::HeartGold | Game::SoulSilver => "a/0/1/1",
        _ => "a/0/2/1",
    }
}

/// Reads the move table of a supported game.
pub fn load<S: Source>(rom: &mut Rom<S>) -> Result<MoveTable> {
    let game = rom.game().ok_or(Error::NotFound)?;
    let file = rom.find(location(game))?;
    let bytes = rom.read(file)?;
    let narc = Narc::open(
        &mut bytes.as_slice(),
        crate::nds::FileRange {
            start: 0,
            end: file.len(),
        },
    )?;
    let gen4 = game.generation() == 4;
    let mut moves = Vec::with_capacity(narc.len());
    for i in 0..narc.len() {
        let r = narc.member(i)?;
        let m = &bytes[r.start as usize..r.end as usize];
        let bad = Error::Format("move record");
        let s = if gen4 {
            if m.len() < 16 {
                return Err(bad);
            }
            let category = match m[2] {
                0 => Category::Physical,
                1 => Category::Special,
                _ => Category::Status,
            };
            MoveStats {
                category,
                power: m[3],
                accuracy: m[5],
                pp: m[6],
            }
        } else {
            if m.len() < 36 {
                return Err(bad);
            }
            let category = match m[2] {
                1 => Category::Physical,
                2 => Category::Special,
                _ => Category::Status,
            };
            MoveStats {
                category,
                power: m[3],
                accuracy: m[4],
                pp: m[5],
            }
        };
        moves.push(s);
    }
    // Pound (move 1) is 40 power, 35 PP in every Gen 4/5 game: a cheap check of the layout.
    if moves.get(1).is_none_or(|p| p.power != 40 || p.pp != 35) {
        return Err(Error::Format("move table"));
    }
    Ok(MoveTable {
        generation: game.generation(),
        moves,
    })
}

impl MoveTable {
    pub fn len(&self) -> usize {
        self.moves.len()
    }

    pub fn is_empty(&self) -> bool {
        self.moves.is_empty()
    }

    pub fn get(&self, mv: u16) -> Option<MoveStats> {
        self.moves.get(usize::from(mv)).copied()
    }

    /// Serializes as a move cache (see [`CachedMoves`]).
    pub fn to_cache(&self, source_game_code: [u8; 4]) -> Vec<u8> {
        let mut v = Vec::with_capacity(CACHE_HEADER + 4 * self.moves.len());
        v.extend_from_slice(CACHE_MAGIC);
        v.extend_from_slice(&[CACHE_VERSION, self.generation]);
        v.extend_from_slice(&(self.moves.len() as u16).to_le_bytes());
        v.extend_from_slice(&source_game_code);
        v.extend_from_slice(&[0; 4]);
        for m in &self.moves {
            v.extend_from_slice(&[m.category as u8, m.power, m.accuracy, m.pp]);
        }
        v
    }
}

const CACHE_MAGIC: &[u8; 4] = b"VMOV";
const CACHE_VERSION: u8 = 1;
const CACHE_HEADER: usize = 16;

/// A move cache read in place. Layout: `"VMOV"`, version `u8` (1), generation `u8`, count `u16`,
/// source game code `[u8; 4]`, 4 reserved bytes, then per move: category, power, accuracy, PP.
pub struct CachedMoves<'a> {
    pub generation: u8,
    pub source_game_code: [u8; 4],
    b: &'a [u8],
    n: usize,
}

impl<'a> CachedMoves<'a> {
    pub fn parse(b: &'a [u8]) -> Result<CachedMoves<'a>> {
        let bad = Error::Format("move cache");
        if b.len() < CACHE_HEADER
            || &b[..4] != CACHE_MAGIC
            || b[4] != CACHE_VERSION
            || !(4..=5).contains(&b[5])
        {
            return Err(bad);
        }
        let n = usize::from(u16_at(b, 6)?);
        if b.len() != CACHE_HEADER + 4 * n {
            return Err(bad);
        }
        Ok(CachedMoves {
            generation: b[5],
            source_game_code: [b[8], b[9], b[10], b[11]],
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

    pub fn get(&self, mv: u16) -> Option<MoveStats> {
        let i = usize::from(mv);
        if i >= self.n {
            return None;
        }
        let r = &self.b[CACHE_HEADER + 4 * i..CACHE_HEADER + 4 * i + 4];
        let category = match r[0] {
            0 => Category::Physical,
            1 => Category::Special,
            _ => Category::Status,
        };
        Some(MoveStats {
            category,
            power: r[1],
            accuracy: r[2],
            pp: r[3],
        })
    }
}
