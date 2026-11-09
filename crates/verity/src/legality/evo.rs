//! Gen 4 evolution chains. Ported from PKHeX `Legality/Evolutions/EvolutionChain.cs`,
//! `EncounterOrigin.cs`, `EvolutionGroup/EvolutionGroup4.cs`, `EvolutionGroup/EvolutionUtil.cs`,
//! `Methods/EvoCriteria.cs`, `Methods/EvolutionMethod.cs`, `Reversal/*` and
//! `Forward/EvolutionForwardSpecies.cs`, for Gen 4 origins in the Gen 4 format.

use alloc::vec::Vec;

use crate::data::{Evolution, GameData};

pub const MAX_EVOLUTIONS: usize = 3;
pub const SENTINEL_NOT_REACHED: u8 = 255;
const ANY_FORM: u8 = 255;

/// PKHeX `EvoCriteria`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct EvoCriteria {
    pub species: u16,
    pub form: u8,
    pub level_up_required: u8,
    pub level_max: u8,
    pub level_min: u8,
    pub method: u8,
}

impl EvoCriteria {
    fn is_better_devolution(&self, r: &EvoCriteria) -> bool {
        r.species == 0
            || self.level_min as u32 + (self.level_up_required as u32)
                < r.level_min as u32 + r.level_up_required as u32
    }

    fn is_better_evolution(&self, r: &EvoCriteria) -> bool {
        if r.method == SENTINEL_NOT_REACHED {
            return true;
        }
        if self.level_min as u32 + self.level_up_required as u32
            > r.level_min as u32 + r.level_up_required as u32
        {
            return false;
        }
        self.level_max > r.level_max
    }
}

/// The PKM fields evolution checks read.
#[derive(Clone, Copy, Debug)]
pub struct EvoPkm {
    pub species: u16,
    pub form: u8,
    pub gender: u8,
    pub contest_beauty: u8,
    pub current_level: u8,
    pub met_level: u8,
    pub is_egg: bool,
    pub egg_location: u16,
}

/// PKHeX `EvolutionType.IsLevelUpRequired`.
pub fn is_level_up_required(t: u8) -> bool {
    !matches!(
        t,
        0 | 5
            | 6
            | 7
            | 8
            | 17
            | 18
            | 35
            | 42
            | 43
            | 44
            | 45
            | 48
            | 49
            | 61
            | 62
            | 90
            | 91
            | 92
            | 255
    )
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum Check {
    Valid,
    Invalid,
}

/// PKHeX `EvolutionMethod.Check` (Gen 4 relevant branches; PK4 is never "untraded").
fn check(m: &Evolution, pk: &EvoPkm, lvl: u8, level_min: u8, skip: bool) -> Check {
    let t = m.method;
    if !is_level_up_required(t) {
        return match t {
            17 | 59 if pk.gender != 0 => Check::Invalid,
            18 | 60 if pk.gender != 1 => Check::Invalid,
            _ => Check::Valid,
        };
    }
    let secondary_ok = match t {
        23 => pk.gender == 0,
        24 => pk.gender == 1,
        34 => pk.gender == 1 && pk.form == 1,
        16 => skip || pk.contest_beauty as u16 >= m.argument,
        _ => true,
    };
    if !secondary_ok {
        return Check::Invalid;
    }
    if lvl < m.level {
        return Check::Invalid;
    }
    if m.level_up == 0 {
        return Check::Valid;
    }
    if m.level == 0 && lvl < 2 {
        return Check::Invalid;
    }
    if (lvl as u32) < level_min as u32 + m.level_up as u32 && !skip {
        return Check::Invalid;
    }
    Check::Valid
}

#[derive(Clone, Copy)]
struct Link {
    method: Evolution,
    species: u16,
    form: u8,
}

/// Evolution data for Gen 4: forward tree from D/P (`EvolutionTree.Evolves4`), presence from HG/SS.
pub struct Evo4<'a> {
    tree: GameData<'a>,
    presence: GameData<'a>,
    /// (dest species | dest form << 11, link), in PKHeX registration order.
    reverse: Vec<(u32, Link)>,
}

impl<'a> Evo4<'a> {
    pub fn new(dp: GameData<'a>, hgss: GameData<'a>) -> Self {
        let mut reverse = Vec::new();
        for s in 1..=dp.max_species() {
            let fc = dp.personal(s as usize).form_count();
            for f in 0..fc {
                for evo in dp.evolutions(s as usize) {
                    if evo.species == 0 {
                        break;
                    }
                    let df = if evo.form == ANY_FORM { f } else { evo.form };
                    reverse.push((
                        evo.species as u32 | (df as u32) << 11,
                        Link {
                            method: evo,
                            species: s,
                            form: f,
                        },
                    ));
                }
            }
        }
        Self {
            tree: dp,
            presence: hgss,
            reverse,
        }
    }

    fn links(&self, species: u16, form: u8) -> impl Iterator<Item = &Link> {
        let key = species as u32 | (form as u32) << 11;
        self.reverse
            .iter()
            .filter(move |(k, _)| *k == key)
            .map(|(_, l)| l)
            .take(2)
    }

    /// PKHeX `IEvolutionNetwork.GetBaseSpeciesForm`: the deepest pre-evolution (first links).
    pub fn base_species_form(&self, mut species: u16, mut form: u8) -> (u16, u8) {
        for _ in 0..8 {
            match self.links(species, form).next() {
                Some(l) => {
                    species = l.species;
                    form = l.form;
                }
                None => break,
            }
        }
        (species, form)
    }

    fn try_devolve(
        &self,
        head: &EvoCriteria,
        pk: &EvoPkm,
        max: u8,
        min: u8,
        skip: bool,
    ) -> Option<EvoCriteria> {
        if head.species > self.tree.max_species() {
            return None;
        }
        self.links(head.species, head.form).find_map(|l| {
            (check(&l.method, pk, max, min, skip) == Check::Valid).then_some(EvoCriteria {
                species: l.species,
                form: l.form,
                level_max: max,
                method: l.method.method,
                level_min: l.method.level,
                level_up_required: l.method.level_up,
            })
        })
    }

    fn try_evolve(
        &self,
        head: &EvoCriteria,
        next: &EvoCriteria,
        pk: &EvoPkm,
        max: u8,
        min: u8,
        skip: bool,
    ) -> Option<EvoCriteria> {
        if head.species > self.tree.max_species() {
            return None;
        }
        self.tree.evolutions(head.species as usize).find_map(|m| {
            let form = if m.form == ANY_FORM {
                head.form
            } else {
                m.form
            };
            (m.species == next.species
                && form == next.form
                && check(&m, pk, max, min, skip) == Check::Valid)
                .then(|| EvoCriteria {
                    species: next.species,
                    form: next.form,
                    level_max: max,
                    method: m.method,
                    level_min: (min.wrapping_add(m.level_up)).max(m.level),
                    level_up_required: m.level_up,
                })
        })
    }

    /// PKHeX `PersonalTable4.IsPresentInGame` (HG/SS table; `PersonalTable3` for a Gen 3 tree).
    pub fn is_present(&self, species: u16, form: u8) -> bool {
        if species > self.presence.max_species() {
            return false;
        }
        if self.presence.generation() == 3 {
            return form == 0
                || match species {
                    201 => form < 28,
                    351 | 386 => form < 4,
                    _ => false,
                };
        }
        if form == 0 {
            return true;
        }
        let p = self.presence.personal(species as usize);
        if p.form_stats_index() != 0 && form < p.form_count() {
            return true;
        }
        match species {
            172 => form == 1,
            201 => form < 28,
            351 => form < 4,
            412 | 414 => form < 3,
            421 => form == 1,
            422 | 423 => form == 1,
            493 => form < 18 && form != 9,
            _ => false,
        }
    }

    fn devolve(
        &self,
        result: &mut [EvoCriteria; MAX_EVOLUTIONS],
        pk: &EvoPkm,
        level_min: u8,
        skip: bool,
    ) {
        for i in 1..MAX_EVOLUTIONS {
            let prev = result[i - 1];
            let Some(evo) = self.try_devolve(&prev, pk, prev.level_max, level_min, skip) else {
                continue;
            };
            if evo.is_better_devolution(&result[i]) {
                result[i] = evo;
            }
        }
    }

    fn discard(&self, result: &mut [EvoCriteria; MAX_EVOLUTIONS]) {
        let mut i = 0;
        while i < MAX_EVOLUTIONS {
            let e = result[i];
            if e.species == 0 {
                break;
            }
            if self.is_present(e.species, e.form) {
                i += 1;
                continue;
            }
            for j in i + 1..MAX_EVOLUTIONS {
                result[j - 1] = result[j];
            }
            result[MAX_EVOLUTIONS - 1] = EvoCriteria::default();
        }
    }

    /// PKHeX `EvolutionChain.GetOriginChain` (span overload) for Gen 4 origin and format.
    fn origin_chain_inner(
        &self,
        pk: &EvoPkm,
        level_min: u8,
        level_max: u8,
        enc_species: u16,
        discard: bool,
    ) -> Vec<EvoCriteria> {
        let mut result = [EvoCriteria::default(); MAX_EVOLUTIONS];
        if pk.is_egg {
            result[0] = EvoCriteria {
                species: pk.species,
                form: pk.form,
                level_max,
                level_min: level_max,
                ..Default::default()
            };
            return result[..1].to_vec();
        }
        result[0] = EvoCriteria {
            species: pk.species,
            form: pk.form,
            level_max,
            ..Default::default()
        };
        self.devolve(&mut result, pk, level_min, false);
        if discard {
            self.discard(&mut result);
        }
        let count = if enc_species != 0 {
            result
                .iter()
                .position(|e| e.species == enc_species)
                .map_or(0, |i| i + 1)
        } else {
            result.iter().take_while(|e| e.species != 0).count()
        };
        let mut chain = result[..count].to_vec();
        clean_devolve(&mut chain, level_min);
        chain
    }

    /// PKHeX `EncounterOrigin.GetOriginChain(pk, 4, Gen4)` for a PK4.
    pub fn encounter_origin_chain(&self, pk: &EvoPkm) -> Vec<EvoCriteria> {
        let min = if pk.is_egg || pk.egg_location != 0 {
            1
        } else {
            pk.met_level
        };
        self.origin_chain_inner(pk, min, pk.current_level, 0, true)
    }

    /// PKHeX `EvolutionChain.GetEvolutionChainsAllGens(pk, enc).Gen4` for a Gen 4 encounter in
    /// the Gen 4 format.
    pub fn chain_for_encounter(
        &self,
        pk: &EvoPkm,
        enc_species: u16,
        enc_level_min: u8,
    ) -> Vec<EvoCriteria> {
        let min = pk.met_level.max(enc_level_min);
        let mut chain = self.origin_chain_inner(pk, min, pk.current_level, enc_species, false);
        if chain.is_empty() {
            return chain;
        }
        for e in chain.iter_mut() {
            if e.species == enc_species {
                break;
            }
            e.method = SENTINEL_NOT_REACHED;
        }
        // EvolutionGroup4.Evolve
        let len = chain.len();
        for i in (1..len).rev() {
            let dest = chain[i - 1];
            let devolved = chain[i];
            match self.try_evolve(
                &devolved,
                &dest,
                pk,
                pk.current_level,
                devolved.level_min,
                false,
            ) {
                Some(evo) => {
                    if evo.is_better_evolution(&dest) {
                        chain[i - 1] = evo;
                    }
                }
                None => {
                    if dest.method == SENTINEL_NOT_REACHED {
                        break;
                    }
                }
            }
        }
        self.set_history(&chain)
    }

    /// `GetOriginChain` (span overload) for a Gen 3 origin in the Gen 4 format: `self` is the
    /// Gen 4 tree, walked first, then `g3` (`EvolutionGroup3.Devolve` caps levels at the met
    /// level, which for a PK4 is the level when it left Pal Park).
    fn origin_chain3(
        &self,
        g3: &Evo4<'a>,
        pk: &EvoPkm,
        level_min: u8,
        level_max: u8,
        enc_species: u16,
        discard: bool,
    ) -> Vec<EvoCriteria> {
        let mut result = [EvoCriteria::default(); MAX_EVOLUTIONS];
        if pk.is_egg {
            result[0] = EvoCriteria {
                species: pk.species,
                form: pk.form,
                level_max,
                level_min: level_max,
                ..Default::default()
            };
            return result[..1].to_vec();
        }
        result[0] = EvoCriteria {
            species: pk.species,
            form: pk.form,
            level_max,
            ..Default::default()
        };
        self.devolve(&mut result, pk, level_min, false);
        for e in result.iter_mut() {
            if e.species == 0 {
                break;
            }
            e.level_max = e.level_max.min(pk.met_level);
        }
        g3.devolve(&mut result, pk, 1, false);
        if discard {
            g3.discard(&mut result);
        }
        let count = if enc_species != 0 {
            result
                .iter()
                .position(|e| e.species == enc_species)
                .map_or(0, |i| i + 1)
        } else {
            result.iter().take_while(|e| e.species != 0).count()
        };
        let mut chain = result[..count].to_vec();
        clean_devolve(&mut chain, level_min);
        chain
    }

    /// `EncounterOrigin.GetOriginChain(pk, 3, Gen3)` for a PK4 (the original met level is lost).
    pub fn encounter_origin_chain3(&self, g3: &Evo4<'a>, pk: &EvoPkm) -> Vec<EvoCriteria> {
        const MIN_WILD_LEVEL: u8 = 2;
        let mut chain = self.origin_chain3(g3, pk, MIN_WILD_LEVEL, pk.current_level, 0, true);
        // EvolutionUtil.ConditionEncounterNoMet
        for i in 0..chain.len().saturating_sub(1) {
            if is_level_up_required(chain[i].method) {
                chain[i].level_min = chain[i + 1].level_min + chain[i].level_up_required;
            }
        }
        chain
    }

    /// `EvolutionChain.GetEvolutionChainsAllGens` for a Gen 3 encounter in the Gen 4 format:
    /// the chain while in Gen 3, then while in Gen 4.
    pub fn chain_for_encounter3(
        &self,
        g3: &Evo4<'a>,
        pk: &EvoPkm,
        enc_species: u16,
        enc_level_min: u8,
    ) -> (Vec<EvoCriteria>, Vec<EvoCriteria>) {
        let mut chain =
            self.origin_chain3(g3, pk, enc_level_min, pk.current_level, enc_species, false);
        if chain.is_empty() {
            return (Vec::new(), Vec::new());
        }
        for e in chain.iter_mut() {
            if e.species == enc_species {
                break;
            }
            e.method = SENTINEL_NOT_REACHED;
        }
        g3.evolve_stage(&mut chain, pk, pk.met_level);
        let gen3 = g3.set_history(&chain);
        // EvolutionGroup4.Evolve, EvolutionUtil.UpdateFloor
        chain[0].level_max = pk.current_level;
        for e in chain.iter_mut() {
            e.level_min = e.level_min.max(pk.met_level);
        }
        self.evolve_stage(&mut chain, pk, pk.current_level);
        (gen3, self.set_history(&chain))
    }

    /// The loop shared by `EvolutionGroup3.Evolve` and `EvolutionGroup4.Evolve`.
    fn evolve_stage(&self, chain: &mut [EvoCriteria], pk: &EvoPkm, max: u8) {
        for i in (1..chain.len()).rev() {
            let (dest, devolved) = (chain[i - 1], chain[i]);
            match self.try_evolve(&devolved, &dest, pk, max, devolved.level_min, false) {
                Some(evo) => {
                    if evo.is_better_evolution(&dest) {
                        chain[i - 1] = evo;
                    }
                }
                None => {
                    if dest.method == SENTINEL_NOT_REACHED {
                        break;
                    }
                }
            }
        }
    }

    /// `EvolutionUtil.SetHistory` with the HG/SS table.
    fn set_history(&self, result: &[EvoCriteria]) -> Vec<EvoCriteria> {
        let (mut start, mut count) = (0usize, 0usize);
        for e in result {
            if e.method == SENTINEL_NOT_REACHED {
                start += count + 1;
                count = 0;
            } else if self.is_present(e.species, e.form) {
                count += 1;
            } else if count == 0 {
                start += 1;
            } else {
                break;
            }
        }
        let mut arr = result[start..start + count].to_vec();
        let len = clean_evolve(&mut arr);
        arr.truncate(len);
        arr
    }
}

fn clean_devolve(result: &mut [EvoCriteria], mut level_min: u8) {
    let (mut req, mut method) = (0u8, 0u8);
    for evo in result.iter_mut().rev() {
        let (next_min, next_req, next_method) = (evo.level_min, evo.level_up_required, evo.method);
        evo.level_min = evo.level_max.min(level_min);
        evo.level_up_required = req;
        evo.method = method;
        level_min = next_min.max(level_min);
        req = next_req;
        method = next_method;
    }
}

fn clean_evolve(result: &mut [EvoCriteria]) -> usize {
    let mut i = 1;
    while i < result.len() {
        let next = result[i - 1];
        let new_max = next.level_max;
        if (result[i].level_min as i32) > new_max as i32 - next.level_up_required as i32 {
            break;
        }
        result[i].level_max = new_max;
        i += 1;
    }
    i.min(result.len())
}
