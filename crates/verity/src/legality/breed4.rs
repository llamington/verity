//! Move order on a Gen 4 bred egg. Ported from PKHeX `Legality/Moves/Breeding/MoveBreed4.cs`.
//!
//! The game fills an egg's moves in a fixed order (base moves, then level-up moves both parents
//! know, the father's TMs, the father's egg moves), so the order of the four moves says where
//! each came from, and an order no sequence of those steps produces is illegal.

use super::moves4::Learn4;
use crate::data::{move_table as mt, GameData};

// PKHeX `EggSource34`.
const BASE: u8 = 1;
const FATHER_EGG: u8 = 2;
const FATHER_TM: u8 = 3;
const PARENT_LEVEL_UP: u8 = 4;
const VOLT_TACKLE_SOURCE: u8 = 6;
/// The highest source value, 4 in both `EggSource34` and `EggSource5`.
const MAX_SOURCE: u8 = 4;

const PICHU: u16 = 172;
const VOLT_TACKLE: u16 = 344;
const EGG_LEVEL: u8 = 1;

struct Info {
    moves: [u16; 4],
    actual: [u8; 4],
    possible: [u8; 4],
    base: [u16; 4],
    base_len: usize,
}

/// `Learnset.GetBaseEggMoves`: the last four moves at or below `level`, in learnset order.
pub fn base_egg_moves(data: &GameData, index: usize, level: u8) -> ([u16; 4], usize) {
    let (mut out, mut n) = ([0u16; 4], 0);
    for (l, m) in data.levelup(index) {
        if l > level {
            break;
        }
        if n == 4 {
            out.copy_within(1..4, 0);
            n = 3;
        }
        out[n] = m;
        n += 1;
    }
    (out, n)
}

/// `Breeding.GetCanInheritMoves`: a species with no female line takes no level-up moves.
pub(super) fn can_inherit(data: &GameData, species: u16) -> bool {
    let ratio = data.personal(species as usize).gender_ratio();
    (ratio != 0 && ratio != 255) || matches!(species, 29 | 32 | 313 | 314)
}

fn verify_base(info: &Info) -> bool {
    let count = info.actual.iter().take_while(|&&a| a == BASE).count();
    let moves = &info.moves;
    let base = &info.base[..info.base_len];
    if base.len() < count || (moves[3] == 0 && count != base.len()) {
        return false;
    }
    for i in 0..count {
        if moves[count - 1 - i] != base[base.len() - 1 - i] {
            return false;
        }
    }
    if base.len() == count {
        return true;
    }
    // A low-index base move may be nudged out, but can only come back after enough other moves.
    for i in count..4 {
        if info.possible[i] & (1 << BASE) == 0 {
            continue;
        }
        let Some(index) = base.iter().position(|&b| b == moves[i]) else {
            continue;
        };
        if i < 4 - base.len() + index + count {
            return false;
        }
    }
    true
}

fn recurse(info: &mut Info, start: usize, source: u8) -> bool {
    let mut i = start;
    loop {
        if source - 1 != 0 && recurse(info, i, source - 1) {
            return true;
        }
        if info.possible[i] & (1 << source) == 0 {
            return false;
        }
        info.actual[i] = source;
        if i == 0 {
            break;
        }
        i -= 1;
    }
    verify_base(info)
}

/// What one generation's egg rules know about a species.
pub(super) trait Breed {
    fn species(&self) -> u16;
    fn base_moves(&self) -> ([u16; 4], usize);
    /// The `EggSource` value for moves inherited from level-up, which differs by generation.
    fn parent_level_up(&self) -> u8;
    fn father_tm(&self) -> u8;
    /// Level-up moves both parents can pass on.
    fn is_level_up(&self, mv: u16) -> bool;
    fn is_egg_move(&self, mv: u16) -> bool;
    fn is_tm(&self, mv: u16) -> bool;
}

struct Breed4<'c, 'a> {
    learn: &'c Learn4<'a>,
    species: u16,
    version: u8,
}

impl Breed for Breed4<'_, '_> {
    fn species(&self) -> u16 {
        self.species
    }
    fn base_moves(&self) -> ([u16; 4], usize) {
        base_egg_moves(
            game(self.learn, self.version),
            self.species as usize,
            EGG_LEVEL,
        )
    }
    fn parent_level_up(&self) -> u8 {
        PARENT_LEVEL_UP
    }
    fn father_tm(&self) -> u8 {
        FATHER_TM
    }
    fn is_level_up(&self, mv: u16) -> bool {
        let table = game(self.learn, self.version);
        can_inherit(table, self.species)
            && table.levelup(self.species as usize).any(|(_, m)| m == mv)
    }
    fn is_egg_move(&self, mv: u16) -> bool {
        let table = if matches!(self.version, 7 | 8) {
            &self.learn.hgss
        } else {
            &self.learn.dp
        };
        table
            .egg_moves(self.species as usize)
            .iter()
            .any(|m| m == mv)
    }
    fn is_tm(&self, mv: u16) -> bool {
        let learn = self.learn;
        let pi = game(learn, self.version).personal(self.species as usize);
        let hm_list = if matches!(self.version, 7 | 8) {
            mt::G4_HM_HGSS
        } else {
            mt::G4_HM_DPPT
        };
        let tm = learn.moves.list(mt::G4_TM).iter().position(|m| m == mv);
        let hm = learn.moves.list(hm_list).iter().position(|m| m == mv);
        tm.is_some_and(|t| t < 92 && pi.tmhm(t)) || hm.is_some_and(|h| h < 8 && pi.tmhm(92 + h))
    }
}

/// `MoveBreed.Validate`: whether `moves` is an order the game can give an egg of this species,
/// and the source found for each move (best effort when it isn't).
fn validate_origins<B: Breed>(b: &B, moves: [u16; 4], previous: [u8; 4]) -> (bool, [u8; 4]) {
    let mut count = moves.iter().position(|&m| m == 0).unwrap_or(4);
    if count == 0 {
        return (false, previous);
    }
    let marked = count;
    let (base, base_len) = b.base_moves();
    let mut info = Info {
        moves,
        actual: previous,
        possible: [0; 4],
        base,
        base_len,
    };
    if b.species() == PICHU && moves[count - 1] == VOLT_TACKLE {
        count -= 1;
        info.actual[count] = VOLT_TACKLE_SOURCE;
    }
    let valid = if count == 0 {
        verify_base(&info)
    } else {
        for (i, &mv) in moves.iter().enumerate().take(count) {
            let mut p = 0u8;
            if base[..base_len].contains(&mv) {
                p |= 1 << BASE;
            }
            if b.is_level_up(mv) {
                p |= 1 << b.parent_level_up();
            }
            if b.is_egg_move(mv) {
                p |= 1 << FATHER_EGG;
            }
            if b.is_tm(mv) {
                p |= 1 << b.father_tm();
            }
            info.possible[i] = p;
        }
        recurse(&mut info, count - 1, MAX_SOURCE)
    };
    if !valid {
        // CleanResult: name a source for each move the search didn't reach.
        for i in 0..marked {
            if info.possible[i] != 0 && info.actual[i] == 0 {
                info.actual[i] = info.possible[i].trailing_zeros() as u8;
            }
        }
    }
    (valid, info.actual)
}

/// The learnset table PKHeX's `GameData.GetLearnSource` picks for a Gen 4 version.
pub fn game<'a, 'b>(learn: &'b Learn4<'a>, version: u8) -> &'b GameData<'a> {
    match version {
        7 | 8 | 66 => &learn.hgss, // HG, SS, the HGSS group
        10 | 11 | 64 => &learn.dp, // D, P, the DP group
        _ => &learn.pt,            // Pt and the DPPt group
    }
}

/// `MoveBreed.Validate`.
pub fn validate(learn: &Learn4, species: u16, version: u8, moves: [u16; 4]) -> bool {
    let b = Breed4 {
        learn,
        species,
        version,
    };
    validate_origins(&b, moves, [0; 4]).0
}

/// One move of an unhatched egg, from [`verify_moves`].
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum EggMove {
    Invalid,
    Empty,
    Inherited,
    Other,
}

/// `LearnVerifierRelearn.VerifyEggMoveset`. A move with no source found is invalid even when the
/// moveset as a whole passes (a move after an empty slot). The source buffer is reused on the
/// second pass, as PKHeX does.
pub fn verify_moves(learn: &Learn4, species: u16, version: u8, moves: [u16; 4]) -> [EggMove; 4] {
    let b = Breed4 {
        learn,
        species,
        version,
    };
    verify_moves_with(&b, moves)
}

pub(super) fn verify_moves_with<B: Breed>(b: &B, moves: [u16; 4]) -> [EggMove; 4] {
    let (valid, origins) = validate_origins(b, moves, [0; 4]);
    let (expected, origins) = if valid {
        (moves, origins)
    } else {
        let expected = expected_with(b, moves);
        let (_, o) = validate_origins(b, expected, origins);
        (expected, o)
    };
    core::array::from_fn(|i| match (moves[i], origins[i]) {
        (m, _) if m != expected[i] => EggMove::Invalid,
        (0, _) => EggMove::Empty,
        (_, 0) => EggMove::Invalid,
        (_, o) if o == b.parent_level_up() => EggMove::Inherited,
        _ => EggMove::Other,
    })
}

/// `MoveBreed.GetExpectedMoves`: `moves` when legal, else the nearest legal order: sorted by
/// source, then with fresh base moves, then just the base moves.
pub fn expected_moves(learn: &Learn4, species: u16, version: u8, moves: [u16; 4]) -> [u16; 4] {
    let b = Breed4 {
        learn,
        species,
        version,
    };
    expected_with(&b, moves)
}

fn expected_with<B: Breed>(b: &B, moves: [u16; 4]) -> [u16; 4] {
    let (valid, origins) = validate_origins(b, moves, [0; 4]);
    if valid {
        return moves;
    }
    // GetSortedMoveOrder: insertion by source, last move first.
    let mut sorted = [(0u16, 0u8); 4];
    let mut count = 0;
    for i in (0..4).rev() {
        let origin = origins[i];
        if origin == 0 {
            continue;
        }
        let at = sorted[..count]
            .iter()
            .position(|&(_, o)| origin < o)
            .unwrap_or(count);
        if at < count {
            sorted.copy_within(at..3, at + 1);
        }
        sorted[at] = (moves[i], origin);
        count += 1;
    }
    let result = sorted.map(|(m, _)| m);
    if validate_origins(b, result, [0; 4]).0 {
        return result;
    }
    let (base, base_len) = b.base_moves();
    let base = &base[..base_len];
    // RebuildMoves.
    let (mut not_base, mut n) = ([0u16; 4], 0);
    for (mv, origin) in sorted {
        if origin != 0 && !base.contains(&mv) {
            not_base[n] = mv;
            n += 1;
        }
    }
    let keep = (4 - n).min(base.len());
    let mut result = [0u16; 4];
    result[..keep].copy_from_slice(&base[base.len() - keep..]);
    result[keep..keep + n].copy_from_slice(&not_base[..n]);
    if validate_origins(b, result, [0; 4]).0 {
        return result;
    }
    let mut result = [0u16; 4];
    result[..base.len()].copy_from_slice(base);
    result
}
