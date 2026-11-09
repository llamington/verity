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
            let nincada = history[history.len() - 1];
            let idx = self.pt.form_index(NINJASK, 0);
            for i in 0..4 {
                if result[i].valid() {
                    continue;
                }
                let mv = current[i];
                if mv == 0 {
                    break;
                }
                let Some(level) = self.pt.levelup(idx).find(|&(_, m)| m == mv).map(|(l, _)| l)
                else {
                    continue;
                };
                if !(nincada.level_min..=nincada.level_max).contains(&level) {
                    continue;
                }
                result[i] = MoveResult::LevelUp { level };
                break;
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
}

const NINCADA: u16 = 290;
const NINJASK: u16 = 291;
const SHEDINJA: u16 = 292;

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
