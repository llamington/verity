//! Current-move verification for Gen 4 origins in the Gen 4 format. Ported from PKHeX
//! `LearnSource/Verify/LearnVerifier.cs`, `LearnVerifierHistory.cs`, `Group/LearnGroup4.cs` and
//! `Sources/LearnSource4HGSS.cs`, `LearnSource4Pt.cs`, `LearnSource4DP.cs`.

use super::evo::EvoCriteria;
use crate::data::{move_table as mt, GameData, MoveData};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MoveResult {
    Unparsed,
    Empty,
    EmptyInvalid,
    Duplicate,
    Unobtainable,
    LevelUp {
        level: u8,
    },
    Machine,
    Tutor,
    Initial,
    /// A level-up move inherited from both parents (PKHeX `LearnMethod.InheritLevelUp`).
    InheritLevelUp,
    Sketch,
}

impl MoveResult {
    pub fn valid(self) -> bool {
        !matches!(
            self,
            Self::Unparsed | Self::EmptyInvalid | Self::Duplicate | Self::Unobtainable
        )
    }

    fn parsed(self) -> bool {
        self != Self::Unparsed
    }
}

pub struct Learn4<'a> {
    pub hgss: GameData<'a>,
    pub pt: GameData<'a>,
    pub dp: GameData<'a>,
    pub moves: MoveData<'a>,
}

const ROTOM: u16 = 479;
const SMEARGLE: u16 = 235;
const MOVE_WHIRLPOOL: u16 = 250;
const MOVE_DEFOG: u16 = 432;

/// PKHeX `MoveSourceType` subset used by Gen 4.
#[derive(Clone, Copy)]
struct Sources {
    level_up: bool,
    machine: bool,
    tutors: bool,
}

const ALL: Sources = Sources {
    level_up: true,
    machine: true,
    tutors: true,
};
const LEVEL_MACHINE: Sources = Sources {
    level_up: true,
    machine: true,
    tutors: false,
};
const LEVEL: Sources = Sources {
    level_up: true,
    machine: false,
    tutors: false,
};

#[derive(Clone, Copy, PartialEq, Eq)]
enum Game {
    Hgss,
    Pt,
    Dp,
}

impl<'a> Learn4<'a> {
    fn data(&self, g: Game) -> &GameData<'a> {
        match g {
            Game::Hgss => &self.hgss,
            Game::Pt => &self.pt,
            Game::Dp => &self.dp,
        }
    }

    fn level_learn(&self, g: Game, species: u16, form: u8, mv: u16) -> Option<u8> {
        let d = self.data(g);
        d.levelup(d.form_index(species, form))
            .find(|&(_, m)| m == mv)
            .map(|(l, _)| l)
    }

    fn list_index(&self, id: u16, mv: u16) -> Option<usize> {
        self.moves.list(id).iter().position(|m| m == mv)
    }

    /// `GetCanLearn` for one game; `pi_game` selects the personal table used for TM/HM and tutors.
    fn can_learn(
        &self,
        g: Game,
        pi_game: Game,
        evo: &EvoCriteria,
        current_form: u8,
        mv: u16,
        src: Sources,
    ) -> Option<MoveResult> {
        let Sources {
            level_up,
            machine,
            tutors,
        } = src;
        if level_up {
            if let Some(level) = self.level_learn(g, evo.species, evo.form, mv) {
                if level <= evo.level_max {
                    return Some(MoveResult::LevelUp { level });
                }
            }
        }
        let pd = self.data(pi_game);
        let pi = pd.species_form(evo.species, evo.form);
        if machine {
            if let Some(i) = self.list_index(mt::G4_TM, mv) {
                if i < 92 && pi.tmhm(i) {
                    return Some(MoveResult::Machine);
                }
            }
            let hm = match g {
                Game::Hgss => self
                    .list_index(mt::G4_HM_HGSS, mv)
                    .map(|i| (i, MOVE_WHIRLPOOL)),
                _ => self.list_index(mt::G4_HM_DPPT, mv).map(|i| (i, MOVE_DEFOG)),
            };
            // `pk.Format == Generation` holds for every PK4, so the special-case move is moot.
            if let Some((i, _)) = hm {
                let ok = match g {
                    // LearnSource4DP.GetIsHM reads GetIsLearnTM(92 + i), which is always false.
                    Game::Dp => false,
                    _ => i < 8 && pi.tmhm(92 + i),
                };
                if ok {
                    return Some(MoveResult::Machine);
                }
            }
        }
        if tutors {
            let special = |id: u16| self.moves.list(id).contains(evo.species);
            let type_tutor = match mv {
                307 => special(mt::G4_SPECIAL_BLAST_BURN),
                308 => special(mt::G4_SPECIAL_HYDRO_CANNON),
                338 => special(mt::G4_SPECIAL_FRENZY_PLANT),
                434 => special(mt::G4_SPECIAL_DRACO_METEOR),
                _ => false,
            };
            if type_tutor {
                return Some(MoveResult::Tutor);
            }
            if let Some(i) = self.list_index(mt::G4_TUTOR, mv) {
                if pd.type_tutor(pd.form_index(evo.species, evo.form), i) {
                    return Some(MoveResult::Tutor);
                }
            }
            if g != Game::Dp && evo.species == ROTOM {
                let form = match mv {
                    315 => Some(1),
                    56 => Some(2),
                    59 => Some(3),
                    403 => Some(4),
                    437 => Some(5),
                    _ => None,
                };
                if form == Some(current_form) {
                    return Some(MoveResult::Tutor);
                }
            }
        }
        None
    }

    fn check_internal(
        &self,
        result: &mut [MoveResult; 4],
        current: &[u16; 4],
        evo: &EvoCriteria,
        current_form: u8,
    ) {
        for i in (0..4).rev() {
            if result[i].valid() {
                continue;
            }
            let mv = current[i];
            if let Some(r) = self.can_learn(Game::Hgss, Game::Hgss, evo, current_form, mv, ALL) {
                result[i] = r;
                continue;
            }
            if let Some(r) =
                self.can_learn(Game::Pt, Game::Pt, evo, current_form, mv, LEVEL_MACHINE)
            {
                result[i] = r;
                continue;
            }
            if let Some(r) = self.can_learn(Game::Dp, Game::Pt, evo, current_form, mv, LEVEL) {
                result[i] = r;
            }
        }
    }

    /// `LearnGroup4.CheckNincadaMoves`.
    fn check_nincada4(
        &self,
        result: &mut [MoveResult; 4],
        current: &[u16; 4],
        nincada: EvoCriteria,
    ) {
        let idx = self.pt.form_index(NINJASK, 0);
        for i in 0..4 {
            if result[i].valid() {
                continue;
            }
            let mv = current[i];
            if mv == 0 {
                break;
            }
            let Some(level) = self.pt.levelup(idx).find(|&(_, m)| m == mv).map(|(l, _)| l) else {
                continue;
            };
            if !(nincada.level_min..=nincada.level_max).contains(&level) {
                continue;
            }
            result[i] = MoveResult::LevelUp { level };
            break;
        }
    }

    /// `LearnVerifier.Verify` for a Gen 3 origin in the Gen 4 format: Gen 4 sources first, then
    /// Gen 3. `egg` applies `CheckEncounterMoves` for a hatched egg of that species.
    pub fn verify3(
        &self,
        g3: &Learn3,
        current: [u16; 4],
        current_form: u8,
        enc_species: u16,
        (gen4, gen3): (&[EvoCriteria], &[EvoCriteria]),
        start: Start3,
    ) -> [MoveResult; 4] {
        let initial_moves = match start {
            Start3::Fixed(m) => Some(m),
            _ => None,
        };
        let mut result = [MoveResult::Unparsed; 4];
        if enc_species == SMEARGLE {
            return self.verify(current, current_form, enc_species, gen4, initial_moves);
        }
        for i in (0..4).rev() {
            if current[i] != 0 {
                break;
            }
            result[i] = MoveResult::Empty;
        }
        if let Some(init) = initial_moves {
            for m in init {
                if m == 0 {
                    break;
                }
                if let Some(i) = current.iter().position(|&c| c == m) {
                    result[i] = MoveResult::Initial;
                }
            }
        }
        for evo in gen4 {
            if matches!(evo.species, 386 | 487 | 492) {
                let fc = self.hgss.species_form(evo.species, evo.form).form_count();
                for f in 0..fc {
                    self.check_internal(
                        &mut result,
                        &current,
                        &EvoCriteria { form: f, ..*evo },
                        current_form,
                    );
                }
            } else {
                self.check_internal(&mut result, &current, evo, current_form);
            }
        }
        if enc_species == NINCADA && gen4.len() == 2 && gen4[0].species == SHEDINJA {
            self.check_nincada4(&mut result, &current, gen4[gen4.len() - 1]);
        }
        if !result.iter().all(|r| r.parsed()) {
            for evo in gen3 {
                g3.check(&mut result, &current, evo);
            }
            if let Start3::Egg { species, version } = start {
                g3.mark_egg_moves(&mut result, &current, species, version);
            }
            if enc_species == NINCADA && gen3.len() == 2 && gen3[0].species == SHEDINJA {
                g3.nincada_moves(&mut result, &current, gen3[gen3.len() - 1]);
            }
        }
        for r in result.iter_mut() {
            if !r.parsed() {
                *r = MoveResult::Unobtainable;
            }
        }
        finalize(&mut result, &current);
        result
    }

    /// PKHeX `LearnVerifier.Verify` for a non-egg Gen 4 origin (no initial-move encounters).
    pub fn verify(
        &self,
        current: [u16; 4],
        current_form: u8,
        enc_species: u16,
        history: &[EvoCriteria],
        initial_moves: Option<[u16; 4]>,
    ) -> [MoveResult; 4] {
        let mut result = [MoveResult::Unparsed; 4];
        if enc_species == SMEARGLE {
            // `VerifySmeargle`: any move Sketch can copy (not Struggle or Chatter).
            for i in 0..4 {
                result[i] = match current[i] {
                    0 => MoveResult::Empty,
                    165 | 448 => MoveResult::Unobtainable,
                    m if m <= self.moves.max_move() => MoveResult::Sketch,
                    _ => MoveResult::Unobtainable,
                };
            }
            finalize(&mut result, &current);
            return result;
        }
        for i in (0..4).rev() {
            if current[i] != 0 {
                break;
            }
            result[i] = MoveResult::Empty;
        }
        if let Some(init) = initial_moves {
            for m in init {
                if m == 0 {
                    break;
                }
                if let Some(i) = current.iter().position(|&c| c == m) {
                    result[i] = MoveResult::Initial;
                }
            }
        }
        for evo in history {
            if matches!(evo.species, 386 | 487 | 492) {
                let fc = self.hgss.species_form(evo.species, evo.form).form_count();
                for f in 0..fc {
                    self.check_internal(
                        &mut result,
                        &current,
                        &EvoCriteria { form: f, ..*evo },
                        current_form,
                    );
                }
            } else {
                self.check_internal(&mut result, &current, evo, current_form);
            }
        }
        // LearnGroup4.CheckNincadaMoves: a Shedinja from a Nincada encounter may keep one
        // Ninjask level-up move learned within Nincada's level range.
        if enc_species == NINCADA && history.len() == 2 && history[0].species == SHEDINJA {
            self.check_nincada4(&mut result, &current, history[history.len() - 1]);
        }
        for r in result.iter_mut() {
            if !r.parsed() {
                *r = MoveResult::Unobtainable;
            }
        }
        finalize(&mut result, &current);
        result
    }
}

const NINCADA: u16 = 290;
const NINJASK: u16 = 291;
const SHEDINJA: u16 = 292;
const MEW: u16 = 151;

/// PKHeX `PersonalInfo3.MachineMovesTechnical`.
const TM3: [u16; 50] = [
    264, 337, 352, 347, 46, 92, 258, 339, 331, 237, 241, 269, 58, 59, 63, 113, 182, 240, 202, 219,
    218, 76, 231, 85, 87, 89, 216, 91, 94, 247, 280, 104, 115, 351, 53, 188, 201, 126, 317, 332,
    259, 263, 290, 156, 213, 168, 211, 285, 289, 315,
];
/// PKHeX `LearnSource3E.Tutor_E`.
const TUTOR_E: [u16; 30] = [
    5, 14, 25, 34, 38, 68, 69, 102, 118, 135, 138, 86, 153, 157, 164, 223, 205, 244, 173, 196, 203,
    189, 8, 207, 214, 129, 111, 9, 7, 210,
];
/// PKHeX `LearnSource3RS` XD tutors.
const TUTOR_MEW: [u16; 6] = [185, 252, 95, 101, 272, 192];
const XD_SELFDESTRUCT: [u16; 48] = [
    74, 75, 76, 88, 89, 90, 91, 92, 93, 94, 95, 100, 101, 102, 103, 109, 110, 143, 150, 151, 185,
    204, 205, 208, 211, 218, 219, 222, 273, 274, 275, 299, 316, 317, 320, 321, 323, 324, 337, 338,
    343, 344, 362, 375, 376, 377, 378, 379,
];
const XD_SKY_ATTACK: [u16; 27] = [
    16, 17, 18, 21, 22, 84, 85, 142, 144, 145, 146, 151, 163, 164, 176, 177, 178, 198, 225, 227,
    250, 276, 277, 278, 279, 333, 334,
];
const XD_NIGHTMARE: [u16; 69] = [
    12, 35, 36, 39, 40, 52, 53, 63, 64, 65, 79, 80, 92, 93, 94, 96, 97, 102, 103, 108, 121, 122,
    124, 131, 137, 150, 151, 163, 164, 173, 174, 177, 178, 190, 196, 197, 198, 199, 200, 203, 206,
    215, 228, 229, 233, 234, 238, 248, 249, 250, 251, 280, 281, 282, 284, 292, 302, 315, 316, 317,
    327, 353, 354, 355, 356, 358, 359, 385, 386,
];

/// How a Gen 3 encounter's moves started.
#[derive(Clone, Copy)]
pub enum Start3 {
    /// From the species' level-up moves.
    Wild,
    /// A template's fixed moves (`IMoveset`).
    Fixed([u16; 4]),
    /// Hatched from an egg of this species in this version.
    Egg { species: u16, version: u8 },
}

/// Gen 3 move sources (PKHeX `LearnGroup3`, `LearnSource3E/RS/FR/LG`), for a Pokémon that has
/// left Gen 3: HM moves can no longer be from Gen 3 (`pk.Format == Generation` fails).
#[derive(Clone, Copy)]
pub struct Learn3<'a> {
    pub data: GameData<'a>,
}

impl Learn3<'_> {
    fn level_in(&self, table: Option<u16>, species: u16, mv: u16) -> Option<u8> {
        let i = species as usize;
        let level = |(l, m): (u8, u16)| (m == mv).then_some(l);
        match table {
            None => self.data.levelup(i).find_map(level),
            Some(t) => self.data.levelup_from(t, i).find_map(level),
        }
    }

    /// `LearnGroup3.Check` over one history entry, marking moves still unexplained.
    fn check(&self, result: &mut [MoveResult; 4], current: &[u16; 4], evo: &EvoCriteria) {
        use crate::data::game_table as gt;
        let species = evo.species;
        if species as usize >= self.data.personal_count() {
            return;
        }
        let pi = self.data.personal(species as usize);
        let found = |t: Option<u16>, mv: u16| {
            self.level_in(t, species, mv)
                .filter(|&l| l <= evo.level_max)
                .map(|level| MoveResult::LevelUp { level })
        };
        for i in (0..4).rev() {
            if result[i].valid() {
                continue;
            }
            let mv = current[i];
            let r = found(None, mv)
                .or_else(|| {
                    let tm = TM3.iter().position(|&m| m == mv);
                    tm.filter(|&t| pi.tmhm(t)).map(|_| MoveResult::Machine)
                })
                .or_else(|| {
                    let tutor = TUTOR_E.iter().position(|&m| m == mv);
                    tutor
                        .filter(|&t| self.data.type_tutor(species as usize, t))
                        .map(|_| MoveResult::Tutor)
                })
                .or_else(|| found(Some(gt::LEVELUP_RS), mv))
                .or_else(|| self.tutor_rs(species, mv).then_some(MoveResult::Tutor))
                .or_else(|| found(Some(gt::LEVELUP_FR), mv))
                .or_else(|| self.tutor_fr(species, mv).then_some(MoveResult::Tutor))
                .or_else(|| found(Some(gt::LEVELUP_LG), mv));
            if let Some(r) = r {
                result[i] = r;
            }
        }
    }

    fn tutor_rs(&self, species: u16, mv: u16) -> bool {
        if species == MEW && TUTOR_MEW.contains(&mv) {
            return true;
        }
        match mv {
            120 => XD_SELFDESTRUCT.binary_search(&species).is_ok(),
            143 => XD_SKY_ATTACK.binary_search(&species).is_ok(),
            171 => XD_NIGHTMARE.binary_search(&species).is_ok(),
            _ => false,
        }
    }

    fn tutor_fr(&self, species: u16, mv: u16) -> bool {
        matches!((mv, species), (307, 6) | (308, 9) | (338, 3))
    }

    fn nincada_moves(
        &self,
        result: &mut [MoveResult; 4],
        current: &[u16; 4],
        nincada: EvoCriteria,
    ) {
        for i in 0..4 {
            if result[i].valid() {
                continue;
            }
            let mv = current[i];
            if mv == 0 {
                break;
            }
            let Some(level) = self.level_in(None, NINJASK, mv) else {
                continue;
            };
            if !(nincada.level_min..=nincada.level_max).contains(&level) {
                continue;
            }
            result[i] = MoveResult::LevelUp { level };
            break;
        }
    }

    /// `LearnGroup3.CheckEncounterMoves`: moves a hatched egg of this species started with.
    pub fn mark_egg_moves(
        &self,
        result: &mut [MoveResult; 4],
        current: &[u16; 4],
        species: u16,
        version: u8,
    ) {
        use crate::data::game_table as gt;
        let table = match version {
            3 => None,
            4 => Some(gt::LEVELUP_FR),
            5 => Some(gt::LEVELUP_LG),
            _ => Some(gt::LEVELUP_RS),
        };
        let eggs = self.data.egg_moves(species as usize);
        for i in (0..4).rev() {
            if result[i].valid() {
                continue;
            }
            let mv = current[i];
            if eggs.iter().any(|m| m == mv) {
                result[i] = MoveResult::Initial;
            } else if super::breed4::can_inherit(&self.data, species)
                && self.level_in(table, species, mv).is_some()
            {
                result[i] = MoveResult::InheritLevelUp;
            } else if mv == 344 && species == 172 && version == 3 {
                result[i] = MoveResult::Initial;
            }
        }
    }
}

pub(crate) fn finalize(result: &mut [MoveResult; 4], current: &[u16; 4]) {
    let mut empty = false;
    for i in 0..4 {
        let mv = current[i];
        if mv == 0 {
            empty = true;
            continue;
        }
        if empty {
            for j in (0..i).rev() {
                if current[j] != 0 {
                    break;
                }
                result[j] = MoveResult::EmptyInvalid;
            }
            empty = false;
            continue;
        }
        if (i + 1..4).rev().any(|j| current[j] == mv) {
            result[i] = MoveResult::Duplicate;
        }
    }
    if current[0] == 0 {
        result[0] = MoveResult::EmptyInvalid;
    }
}
