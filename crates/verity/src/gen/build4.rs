//! Builds a PK4 for a generated D/P/Pt wild encounter. Mirrors PKHeX
//! `EncounterSlot4.ConvertToPKM`, `G4PKM.MetLocation`/`Ball` setters, `Learnset.SetEncounterMoves`
//! and `PKM.SetMoves`; PID/IVs/level come from [`super::wild4`].

use super::wild4::Generated;
use crate::data::encounters::Encounter;
use crate::data::{GameData, MoveData, Strings};
use crate::pkm::{Format, Pkm, SIZE_4STORED};
use crate::text::{encode4, Clear};

/// The trainer receiving the Pokémon (the save's player).
#[derive(Clone, Copy, Debug)]
pub struct Trainer4 {
    /// Gen 4-encoded OT name (terminated or 7 characters).
    pub name_raw: [u16; 8],
    pub tid: u16,
    pub sid: u16,
    pub gender: u8,
    pub language: u8,
}

/// Data the builder reads.
pub struct BuildData<'a> {
    /// HG/SS personal table (PKHeX uses `PersonalTable.HGSS` for Gen 4 conversions).
    pub hgss: GameData<'a>,
    /// Level-up source for the encounter's game (D/P or Pt).
    pub learn: GameData<'a>,
    pub moves: MoveData<'a>,
    pub names4: Strings<'a>,
}

const FARAWAY4: u16 = 3002;
const BALL_POKE: u8 = 4;
const BALL_CHERISH: u8 = 16;

/// PKHeX `Language.GetSafeLanguage456`.
pub fn safe_language(lang: u8) -> u8 {
    if matches!(lang, 1..=5 | 7 | 8) {
        lang
    } else {
        2
    }
}

/// PKHeX `Learnset.SetEncounterMoves`: the last four distinct level-up moves at or below `level`.
pub fn encounter_moves(learn: &GameData, species: u16, form: u8, level: u8) -> [u16; 4] {
    let mut moves = [0u16; 4];
    let mut ctr = 0usize;
    for (req, mv) in learn.levelup(learn.form_index(species, form)) {
        if req < 1 {
            continue;
        }
        if req > level {
            break;
        }
        if !moves.contains(&mv) {
            moves[ctr & 3] = mv;
            ctr += 1;
        }
    }
    if ctr > 4 {
        moves.rotate_left(ctr & 3);
    }
    moves
}

/// Builds the stored (box) PK4 for a generated wild slot encounter, met on `date` (years since
/// 2000, month, day).
pub fn build_wild4(
    d: &BuildData,
    enc: &Encounter,
    g: &Generated,
    tr: &Trainer4,
    date: [u8; 3],
) -> Pkm {
    let mut raw = [0u8; SIZE_4STORED];
    let species = enc.species();
    let form = if enc.has(crate::data::encounters::flag::RANDOM_UNSPECIFIC_FORM) {
        g.form
    } else {
        enc.form()
    };
    let pi = d.hgss.species_form(species, form);
    let language = safe_language(tr.language);
    let version = enc.version();

    let put16 = |raw: &mut [u8; SIZE_4STORED], o: usize, v: u16| {
        raw[o..o + 2].copy_from_slice(&v.to_le_bytes())
    };
    raw[0..4].copy_from_slice(&g.pid.to_le_bytes());
    put16(&mut raw, 0x08, species);
    put16(&mut raw, 0x0C, tr.tid);
    put16(&mut raw, 0x0E, tr.sid);
    raw[0x10..0x14]
        .copy_from_slice(&crate::pkm::derived::exp_for_level(g.level, pi.growth()).to_le_bytes());
    raw[0x14] = pi.base_friendship();
    let [a1, a2, _] = pi.abilities();
    raw[0x15] = if g.pid & 1 == 0 { a1 } else { a2 };
    raw[0x17] = language;
    raw[0x38..0x3C].copy_from_slice(&(g.iv32 & 0x3FFF_FFFF).to_le_bytes());
    raw[0x40] = (g.gender & 3) << 1 | form << 3;
    raw[0x5F] = version;
    raw[0x7B] = date[0];
    raw[0x7C] = date[1];
    raw[0x7D] = date[2];

    let loc = enc.location();
    let pt_or_hgss = matches!(version, 7 | 8 | 12);
    let (dp_loc, ext_loc) = if (112..2000).contains(&loc) || (2011..3000).contains(&loc) {
        (FARAWAY4, loc)
    } else {
        (loc, if pt_or_hgss { loc } else { 0 })
    };
    put16(&mut raw, 0x80, dp_loc);
    put16(&mut raw, 0x46, ext_loc);

    let ball = match enc.ball() {
        0 => BALL_POKE,
        b => b,
    };
    raw[0x83] = if ball <= BALL_CHERISH {
        ball
    } else {
        BALL_POKE
    };
    raw[0x84] = (g.level & 0x7F) | (tr.gender & 1) << 7;
    let tiles = enc.ground_tile();
    raw[0x85] = if tiles == 0 {
        0
    } else {
        tiles.trailing_zeros() as u8
    };

    let mut pk = Pkm::from_decrypted(Format::Pk4, &raw).expect("stored length");

    let name = d.names4.get(language as u16, species as usize);
    let units: alloc::vec::Vec<u16> = name.iter().collect();
    let mut nick = [0u16; 11];
    encode4(&units, 10, language, &mut nick, Clear::Zero);
    pk.set_nickname_raw(&nick);

    let mut ot = [0u16; 8];
    for (i, &c) in tr.name_raw.iter().take(7).enumerate() {
        if c == crate::text::TERMINATOR {
            ot[i] = c;
            break;
        }
        ot[i] = c;
        if i == 6 {
            ot[7] = crate::text::TERMINATOR;
        }
    }
    pk.set_ot_name_raw(&ot);

    let moves = encounter_moves(&d.learn, species, form, g.level);
    pk.set_moves_with_pp(moves, &d.moves);
    pk.refresh_checksum();
    pk
}
