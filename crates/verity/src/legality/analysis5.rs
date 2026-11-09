//! Legality of a Pokémon that originated in Black/White or Black 2/White 2, in the Gen 5 format.
//! Ported from PKHeX `EncounterGenerator5`, `EncounterEnumerator5`, the `Templates/Gen5` encounter
//! types, `LearnGroup5` and the verifiers' Gen 5 branches. Wonder Card events and Pokémon sent from
//! Gen 4 are not modelled yet, so they come back as [`Matched::Invalid`].

use alloc::vec::Vec;

use super::analysis4::{evolution_move_of, CheckId, Matched, Report};
use super::breed4::{self, Breed, EggMove};
use super::evo::{Evo4, EvoCriteria, EvoPkm};
use super::moves4::{finalize, MoveResult};
use crate::data::encounters::{flag, Encounter, EncounterData, EncounterKind};
use crate::data::{move_table as mt, GameData, MoveData, Strings};
use crate::pkm::derived::{exp_for_level, gender_from_pid};
use crate::pkm::Pkm;

const VERSION_W: u8 = 20;
const VERSION_B: u8 = 21;
const VERSION_W2: u8 = 22;
const VERSION_B2: u8 = 23;

const SMEARGLE: u16 = 235;
const PICHU: u16 = 172;
const VOLT_TACKLE: u16 = 344;
const COMBEE: u16 = 415;
const VESPIQUEN: u16 = 416;
const DEERLING: u16 = 585;
const SAWSBUCK: u16 = 586;
const ROTOM: u16 = 479;
const KELDEO: u16 = 647;
const MELOETTA: u16 = 648;
const WURMPLE: u16 = 265;
const DITTO: u16 = 132;

const LINK_TRADE5: u16 = 30003;
const LINK_TRADE5_NPC: u16 = 30002;
const DAYCARE5: u16 = 60002;

const ABILITY_ANY12: u8 = 0;
const ABILITY_HIDDEN: u8 = 4;
const GENDER_RANDOM: u8 = 255;
const BALL_POKE: u8 = 4;

/// Gen 5 data for legality.
pub struct Context5<'a> {
    pub bw: GameData<'a>,
    pub b2w2: GameData<'a>,
    pub moves: MoveData<'a>,
    pub enc_bw: EncounterData<'a>,
    pub enc_b2w2: EncounterData<'a>,
    pub enc_dr: EncounterData<'a>,
    pub evo: Evo4<'a>,
    /// Gen 5 species names by language ID (`names5.bin`); index 0 is the Egg name.
    pub names: Strings<'a>,
}

/// Whether a version value is one of the four Gen 5 games.
pub fn is_gen5_version(v: u8) -> bool {
    matches!(v, VERSION_W | VERSION_B | VERSION_W2 | VERSION_B2)
}

#[derive(Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum Rating {
    Match,
    Deferred,
    DeferredErrors,
    Partial,
    None,
}

#[derive(Clone, Copy)]
enum Enc5<'a> {
    Data(Encounter<'a>),
    Egg { species: u16, form: u8 },
}

fn hidden(pk: &Pkm) -> bool {
    pk.hidden_ability()
}

/// `IEncounterMatch.IsPartialMatchHidden`.
fn no_hidden_stage(species: u16) -> bool {
    matches!(species, 11 | 14 | 247 | 266 | 268 | 329 | 330)
}

fn partial_hidden(current: u16, original: u16) -> bool {
    current != original && no_hidden_stage(original) && no_hidden_stage(current)
}

/// `FormInfo.IsFormChangeable` for a Gen 5 origin.
fn form_changeable(species: u16) -> bool {
    matches!(
        species,
        386 | 412
            | 479
            | 483
            | 484
            | 487
            | 492
            | 493
            | 585
            | 586
            | 641
            | 642
            | 645
            | 646
            | 647
            | 649
    )
}

fn slot_exact(e: &Encounter, pk: &Pkm, evo: &EvoCriteria) -> bool {
    (e.level_min()..=e.level_max()).contains(&pk.met_level())
        && (evo.form == e.form() || matches!(e.species(), DEERLING | SAWSBUCK))
}

fn slot_rating(e: &Encounter, pk: &Pkm) -> Rating {
    let is_hidden = hidden(pk);
    if is_hidden && partial_hidden(pk.species(), e.species()) {
        return Rating::Partial;
    }
    if e.species() == COMBEE && pk.species() == VESPIQUEN && pk.pid() & 0xFF >= 0x1F {
        return Rating::DeferredErrors;
    }
    if is_hidden != e.is_hidden_grotto() {
        return Rating::Deferred;
    }
    Rating::Match
}

/// `EncounterStatic5.IsRoamerMet`.
fn roamer_met(location: u16) -> bool {
    location < 32 && 0b1011_1111_1111_1111_1000_0000_0000_0000u32 & (1 << location) != 0
}

fn static_exact(e: &Encounter, pk: &Pkm, evo: &EvoCriteria) -> bool {
    let Some(t) = e.template() else {
        return false;
    };
    let is_egg = t.has(flag::EGG);
    let egg_ok = if !is_egg {
        pk.egg_location() == t.egg_location()
    } else if !pk.is_egg() {
        pk.egg_location() == t.egg_location()
            || matches!(pk.egg_location(), LINK_TRADE5 | LINK_TRADE5_NPC)
    } else {
        pk.egg_location() == t.egg_location()
            && matches!(pk.met_location(), 0 | LINK_TRADE5 | LINK_TRADE5_NPC)
    };
    let met = pk.met_location();
    let location_ok = is_egg
        || if t.has(flag::ROAMING) {
            roamer_met(met)
        } else {
            met == t.location()
        };
    egg_ok
        && location_ok
        && pk.met_level() == t.level_min()
        && (t.gender() == GENDER_RANDOM || pk.gender() == t.gender())
        && (t.form() == evo.form || form_changeable(t.species()))
}

fn static_rating(e: &Encounter, pk: &Pkm) -> Rating {
    let Some(t) = e.template() else {
        return Rating::Match;
    };
    let is_hidden = hidden(pk);
    if (t.ability() == ABILITY_HIDDEN && !is_hidden)
        || (is_hidden && partial_hidden(pk.species(), t.species()))
    {
        Rating::Partial
    } else {
        Rating::Match
    }
}

fn entree_exact(e: &Encounter, pk: &Pkm, evo: &EvoCriteria) -> bool {
    let Some(t) = e.template() else {
        return false;
    };
    if pk.egg_location() != t.egg_location()
        || pk.met_location() != t.location()
        || pk.met_level() != t.level_min()
    {
        return false;
    }
    // PKHeX returns true here on a gender mismatch, skipping the form check.
    if t.gender() != GENDER_RANDOM && pk.gender() != t.gender() {
        return true;
    }
    t.form() == evo.form || form_changeable(t.species())
}

fn radar_exact(e: &Encounter, pk: &Pkm, evo: &EvoCriteria) -> bool {
    let Some(t) = e.template() else {
        return false;
    };
    let lv = pk.met_level();
    pk.egg_location() == t.egg_location()
        && pk.met_location() == t.location()
        && lv.is_multiple_of(5)
        && (5..=40).contains(&lv)
        && (t.form() == evo.form || form_changeable(t.species()))
}

fn n_exact(ctx: &Context5, e: &Encounter, pk: &Pkm, evo: &EvoCriteria) -> bool {
    let Some(t) = e.template() else {
        return false;
    };
    let ratio = ctx.b2w2.species_form(t.species(), t.form()).gender_ratio();
    const N_IV: u16 = 30;
    t.pid() == pk.pid()
        && pk.egg_location() == t.egg_location()
        && pk.met_location() == t.location()
        && pk.met_level() == t.level_min()
        && (t.form() == evo.form || form_changeable(t.species()))
        && pk.gender() == if ratio == 255 { 2 } else { 0 }
        && pk.ot_gender() == 0
        && pk.ivs().iter().all(|&v| v == N_IV)
        && pk.tid() == t.tid()
        && pk.sid() == t.sid()
}

fn trade_exact(e: &Encounter, pk: &Pkm, evo: &EvoCriteria, bw: bool) -> bool {
    let Some(t) = e.template() else {
        return false;
    };
    let ivs_ok = if bw || t.has(flag::HAS_IVS) {
        pk.ivs()
            .iter()
            .zip(t.ivs().iter())
            .all(|(&a, &b)| a == b as u16)
    } else {
        true
    };
    let identity_ok = if bw {
        t.pid() == pk.pid() && t.nature() == pk.nature()
    } else {
        shiny_valid(t.shiny(), pk.is_shiny())
            && (t.gender() == GENDER_RANDOM || t.gender() == pk.gender())
            && (t.nature() == 25 || t.nature() == pk.nature())
    };
    pk.met_level() == t.level_min()
        && ivs_ok
        && identity_ok
        && pk.tid() == t.tid()
        && pk.sid() == t.sid()
        && (evo.form == t.form() || form_changeable(t.species()))
        && pk.ot_gender() == t.ot_gender()
        && pk.egg_location() == t.egg_location()
}

fn shiny_valid(shiny: u8, is_shiny: bool) -> bool {
    match shiny {
        1 => !is_shiny,
        2 => is_shiny,
        _ => true,
    }
}

struct Search<'a, 'p> {
    pk: &'p Pkm,
    chain: &'p [EvoCriteria],
    out: Vec<Enc5<'a>>,
    deferred: Option<(Enc5<'a>, Rating)>,
    best: Rating,
    yielded: bool,
}

impl<'a> Search<'a, '_> {
    fn scan(
        &mut self,
        list: impl Iterator<Item = Encounter<'a>>,
        exact: impl Fn(&Encounter<'a>, &Pkm, &EvoCriteria) -> bool,
        rate: impl Fn(&Encounter<'a>, &Pkm) -> Rating,
    ) {
        for e in list {
            let Some(evo) = self.chain.iter().find(|c| c.species == e.species()) else {
                continue;
            };
            if !exact(&e, self.pk, evo) {
                continue;
            }
            let r = rate(&e, self.pk);
            if r == Rating::Match {
                self.out.push(Enc5::Data(e));
                self.yielded = true;
            } else if r < self.best {
                self.deferred = Some((Enc5::Data(e), r));
                self.best = r;
            }
        }
    }
}

fn split_breed(species: u16) -> bool {
    matches!(species, 183 | 202 | 113 | 122 | 143 | 185 | 226 | 315 | 358)
}

fn can_hatch(species: u16) -> bool {
    !matches!(
        species,
        132 | 144..=146 | 150 | 151 | 201 | 243..=245 | 249..=251 | 377..=386 | 480..=488
            | 490..=494 | 638..=649
    )
}

/// `Breeding.IsBreedableForm`.
fn breedable_form(species: u16, form: u8) -> bool {
    form == 0 || !matches!(species, 25 | 133 | 172)
}

fn add_eggs<'a>(ctx: &Context5<'a>, s: &mut Search<'a, '_>) -> bool {
    let chain = s.chain;
    let Some(devolved) = chain.last() else {
        return false;
    };
    if !(devolved.level_min..=devolved.level_max).contains(&1) {
        return false;
    }
    let (species, form) = ctx.evo.base_species_form(devolved.species, devolved.form);
    if species != devolved.species && !split_breed(devolved.species) {
        return false;
    }
    if !can_hatch(species) || !breedable_form(species, form) || !ctx.evo.is_present(species, form) {
        return false;
    }
    s.out.push(Enc5::Egg { species, form });
    s.yielded = true;
    let mut d = *devolved;
    if species == d.species {
        if chain.len() < 2 {
            return true;
        }
        d = chain[chain.len() - 2];
    }
    if split_breed(d.species) {
        s.out.push(Enc5::Egg {
            species: d.species,
            form,
        });
    }
    true
}

/// Candidates in `EncounterEnumerator5` order.
fn enumerate<'a>(ctx: &Context5<'a>, pk: &Pkm, chain: &[EvoCriteria]) -> Vec<Enc5<'a>> {
    let mut s = Search {
        pk,
        chain,
        out: Vec::new(),
        deferred: None,
        best: Rating::None,
        yielded: false,
    };
    if chain.is_empty() {
        return s.out;
    }
    let version = pk.version();
    let bw = matches!(version, VERSION_W | VERSION_B);
    let ed = if bw { &ctx.enc_bw } else { &ctx.enc_b2w2 };
    let match_any = |_: &Encounter<'a>, _: &Pkm| Rating::Match;
    if pk.met_location() == LINK_TRADE5_NPC && !pk.is_egg() {
        let (own, shared): (&[u8], &[u8]) = match version {
            VERSION_W => (b"TradeGift_W", b"TradeGift_BW"),
            VERSION_B => (b"TradeGift_B", b"TradeGift_BW"),
            VERSION_W2 => (b"TradeGift_W2", b"TradeGift_B2W2"),
            _ => (b"TradeGift_B2", b"TradeGift_B2W2"),
        };
        let exact = |e: &Encounter<'a>, p: &Pkm, c: &EvoCriteria| trade_exact(e, p, c, bw);
        s.scan(ed.group(own), exact, match_any);
        if s.yielded {
            return finish(s);
        }
        s.scan(ed.group(shared), exact, match_any);
        if s.yielded {
            return finish(s);
        }
    } else if matches!(pk.egg_location(), DAYCARE5 | LINK_TRADE5 | LINK_TRADE5_NPC)
        && add_eggs(ctx, &mut s)
        && pk.egg_location() == DAYCARE5
    {
        return s.out;
    }
    captures(ctx, ed, version, bw, &mut s);
    finish(s)
}

fn finish<'a>(mut s: Search<'a, '_>) -> Vec<Enc5<'a>> {
    if let Some((e, _)) = s.deferred.take() {
        s.out.push(e);
    }
    s.out
}

fn captures<'a>(
    ctx: &Context5<'a>,
    ed: &EncounterData<'a>,
    version: u8,
    bw: bool,
    s: &mut Search<'a, '_>,
) {
    let match_any = |_: &Encounter<'a>, _: &Pkm| Rating::Match;
    let own: &[u8] = match version {
        VERSION_W => b"StaticW",
        VERSION_B => b"StaticB",
        VERSION_W2 => b"StaticW2",
        _ => b"StaticB2",
    };
    s.scan(ed.group(own), static_exact, static_rating);
    if bw {
        s.scan(ed.group(b"Encounter_BW"), static_exact, static_rating);
        s.scan(ed.group(b"DreamWorld_BW"), entree_exact, match_any);
    } else {
        s.scan(
            ed.group(b"Encounter_B2W2_Regular"),
            static_exact,
            static_rating,
        );
        s.scan(
            ed.group(b"Encounter_B2W2_N"),
            |e: &Encounter<'a>, p: &Pkm, c: &EvoCriteria| n_exact(ctx, e, p, c),
            match_any,
        );
        s.scan(ed.group(b"DreamWorld_B2W2"), entree_exact, match_any);
        s.scan(
            ctx.enc_dr.group(b"Encounter_DreamRadar"),
            radar_exact,
            match_any,
        );
    }
    s.scan(
        ctx.enc_dr.group(b"DreamWorld_Common"),
        entree_exact,
        match_any,
    );
    if !s.pk.is_egg() && s.pk.egg_location() == 0 {
        let slots: &[u8] = match version {
            VERSION_W => b"SlotsW",
            VERSION_B => b"SlotsB",
            VERSION_W2 => b"SlotsW2",
            _ => b"SlotsB2",
        };
        s.scan(
            ed.slots_at(slots, s.pk.met_location()),
            slot_exact,
            slot_rating,
        );
    }
}

/// Gen 5 move learning (`LearnGroup5`, `LearnSource5BW`, `LearnSource5B2W2`).
impl Context5<'_> {
    fn level_learn(g: &GameData, species: u16, form: u8, mv: u16) -> Option<u8> {
        g.levelup(g.form_index(species, form))
            .find(|&(_, m)| m == mv)
            .map(|(l, _)| l)
    }

    fn list_index(&self, id: u16, mv: u16) -> Option<usize> {
        self.moves.list(id).iter().position(|m| m == mv)
    }

    fn can_learn_b2w2(&self, evo: &EvoCriteria, current_form: u8, mv: u16) -> Option<MoveResult> {
        let g = &self.b2w2;
        if let Some(level) = Self::level_learn(g, evo.species, evo.form, mv) {
            if level <= evo.level_max {
                return Some(MoveResult::LevelUp { level });
            }
        }
        let pi = g.species_form(evo.species, evo.form);
        let raw = pi.raw();
        if self.list_index(mt::G5_TMHM, mv).is_some_and(|i| pi.tmhm(i)) {
            return Some(MoveResult::Machine);
        }
        let bit = |base: usize, i: usize| raw[base + (i >> 3)] & (1 << (i & 7)) != 0;
        if self
            .list_index(mt::G5_TYPE_TUTOR, mv)
            .is_some_and(|i| i < 8 && bit(0x38, i))
        {
            return Some(MoveResult::Tutor);
        }
        for (id, base, count) in [
            (mt::B2W2_TUTOR_1, 0x3C, 15),
            (mt::B2W2_TUTOR_2, 0x40, 17),
            (mt::B2W2_TUTOR_3, 0x44, 13),
            (mt::B2W2_TUTOR_4, 0x48, 15),
        ] {
            if self
                .list_index(id, mv)
                .is_some_and(|i| i < count && bit(base, i))
            {
                return Some(MoveResult::Tutor);
            }
        }
        self.enhanced_tutor(evo.species, current_form, mv)
            .then_some(MoveResult::Tutor)
    }

    fn enhanced_tutor(&self, species: u16, current_form: u8, mv: u16) -> bool {
        match species {
            KELDEO => mv == 548,
            MELOETTA => mv == 547,
            ROTOM => matches!(
                (mv, current_form),
                (315, 1) | (56, 2) | (59, 3) | (403, 4) | (437, 5)
            ),
            _ => false,
        }
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
            if let Some(r) = self.can_learn_b2w2(evo, current_form, mv) {
                result[i] = r;
            }
            if evo.species as usize >= self.bw.personal_count() {
                continue;
            }
            if let Some(level) = Self::level_learn(&self.bw, evo.species, evo.form, mv) {
                if level <= evo.level_max {
                    result[i] = MoveResult::LevelUp { level };
                }
            }
        }
    }

    fn egg_table(&self, version: u8) -> &GameData<'_> {
        if matches!(version, VERSION_W | VERSION_B) {
            &self.bw
        } else {
            &self.b2w2
        }
    }

    fn verify_moves(
        &self,
        pk: &Pkm,
        enc_species: u16,
        history: &[EvoCriteria],
        initial: Option<[u16; 4]>,
        egg: Option<(u16, u8)>,
    ) -> [MoveResult; 4] {
        let current = pk.moves();
        let mut result = [MoveResult::Unparsed; 4];
        if enc_species == SMEARGLE {
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
        if let Some(init) = initial {
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
                let fc = self.b2w2.species_form(evo.species, evo.form).form_count();
                for f in 0..fc {
                    self.check_internal(
                        &mut result,
                        &current,
                        &EvoCriteria { form: f, ..*evo },
                        pk.form(),
                    );
                }
            } else {
                self.check_internal(&mut result, &current, evo, pk.form());
            }
        }
        if let Some((species, form)) = egg {
            let table = self.egg_table(pk.version());
            let idx = table.form_index(species, form);
            let eggs = self.bw.egg_moves(self.bw.form_index(species, form));
            let inherit = breed4::can_inherit(table, species);
            for i in (0..4).rev() {
                if result[i].valid() {
                    continue;
                }
                let mv = current[i];
                if eggs.iter().any(|m| m == mv) {
                    result[i] = MoveResult::Initial;
                } else if inherit && table.levelup(idx).any(|(_, m)| m == mv) {
                    result[i] = MoveResult::InheritLevelUp;
                } else if mv == VOLT_TACKLE && species == PICHU {
                    result[i] = MoveResult::Initial;
                }
            }
        }
        for r in result.iter_mut() {
            if *r == MoveResult::Unparsed {
                *r = MoveResult::Unobtainable;
            }
        }
        // LearnVerifier.FlagFormExclusiveKyurem: fused forms swap Scary Face for their Fusion move.
        if pk.species() == 646 {
            let form = pk.form();
            for (mv, allowed) in [(558, form == 1), (559, form == 2), (184, form == 0)] {
                if !allowed {
                    if let Some(i) = current.iter().position(|&c| c == mv) {
                        result[i] = MoveResult::Unobtainable;
                    }
                }
            }
        }
        finalize(&mut result, &current);
        result
    }
}

/// `MoveBreed5`: the order of an unhatched egg's moves.
struct Breed5<'c, 'a> {
    ctx: &'c Context5<'a>,
    species: u16,
    version: u8,
}

impl Breed for Breed5<'_, '_> {
    fn species(&self) -> u16 {
        self.species
    }
    fn base_moves(&self) -> ([u16; 4], usize) {
        let t = self.ctx.egg_table(self.version);
        breed4::base_egg_moves(t, self.species as usize, 1)
    }
    fn parent_level_up(&self) -> u8 {
        3
    }
    fn father_tm(&self) -> u8 {
        4
    }
    fn is_level_up(&self, mv: u16) -> bool {
        let t = self.ctx.egg_table(self.version);
        breed4::can_inherit(t, self.species)
            && t.levelup(self.species as usize).any(|(_, m)| m == mv)
    }
    fn is_egg_move(&self, mv: u16) -> bool {
        self.ctx
            .bw
            .egg_moves(self.species as usize)
            .iter()
            .any(|m| m == mv)
    }
    fn is_tm(&self, mv: u16) -> bool {
        let t = self.ctx.egg_table(self.version);
        // The list's last entry, TM96 Snarl's slot, can't be used.
        let n = self.ctx.moves.list(mt::G5_TMHM).len().saturating_sub(1);
        self.ctx
            .list_index(mt::G5_TMHM, mv)
            .is_some_and(|i| i < n && t.personal(self.species as usize).tmhm(i))
    }
}

fn egg_moves_unhatched(ctx: &Context5, pk: &Pkm, species: u16) -> [MoveResult; 4] {
    let current = pk.moves();
    let b = Breed5 {
        ctx,
        species,
        version: pk.version(),
    };
    let mut out = breed4::verify_moves_with(&b, current).map(|m| match m {
        EggMove::Invalid => MoveResult::Unobtainable,
        EggMove::Empty => MoveResult::Empty,
        EggMove::Inherited => MoveResult::InheritLevelUp,
        EggMove::Other => MoveResult::Initial,
    });
    let dupe = (1..4).find(|&i| current[i] != 0 && current[..i].contains(&current[i]));
    if let Some(i) = dupe {
        out[i] = MoveResult::Duplicate;
    }
    finalize(&mut out, &current);
    out
}

fn evo_input(pk: &Pkm, level: u8) -> EvoPkm {
    EvoPkm {
        species: pk.species(),
        form: pk.form(),
        gender: pk.gender(),
        contest_beauty: pk.contest()[1],
        current_level: level,
        met_level: pk.met_level(),
        is_egg: pk.is_egg(),
        egg_location: pk.egg_location(),
    }
}

fn enc_species(e: &Enc5) -> u16 {
    match e {
        Enc5::Data(d) => d.species(),
        Enc5::Egg { species, .. } => *species,
    }
}

fn enc_level_min(e: &Enc5) -> u8 {
    match e {
        Enc5::Data(d) => d.level_min(),
        Enc5::Egg { .. } => 1,
    }
}

fn moves_for(ctx: &Context5, pk: &Pkm, e: &Enc5, history: &[EvoCriteria]) -> [MoveResult; 4] {
    if pk.is_egg() {
        return match e {
            Enc5::Egg { species, .. } => egg_moves_unhatched(ctx, pk, *species),
            Enc5::Data(d) => base_initial(ctx, pk, d),
        };
    }
    match e {
        Enc5::Egg { species, form } => {
            ctx.verify_moves(pk, *species, history, None, Some((*species, *form)))
        }
        Enc5::Data(d) => {
            let initial = d
                .template()
                .filter(|t| t.has(flag::HAS_MOVES))
                .map(|t| t.moves());
            ctx.verify_moves(pk, d.species(), history, initial, None)
        }
    }
}

/// `LearnVerifierEgg` for a gift egg: exactly its fixed or base moves, in order.
fn base_initial(ctx: &Context5, pk: &Pkm, d: &Encounter) -> [MoveResult; 4] {
    let current = pk.moves();
    let fixed = d
        .template()
        .filter(|t| t.has(flag::HAS_MOVES))
        .map(|t| t.moves());
    let expected = fixed.unwrap_or_else(|| {
        let t = ctx.egg_table(pk.version());
        let (b, n) = breed4::base_egg_moves(t, t.form_index(d.species(), d.form()), d.level_min());
        let mut out = [0u16; 4];
        out[..n].copy_from_slice(&b[..n]);
        out
    });
    let mut out: [MoveResult; 4] = core::array::from_fn(|i| {
        if current[i] != expected[i] {
            MoveResult::Unobtainable
        } else if expected[i] == 0 {
            MoveResult::Empty
        } else {
            MoveResult::Initial
        }
    });
    finalize(&mut out, &current);
    out
}

fn evolution_ok(
    ctx: &Context5,
    pk: &Pkm,
    species: u16,
    history: &[EvoCriteria],
    moves: &[MoveResult; 4],
) -> bool {
    let cur = pk.species();
    if cur == species {
        return true;
    }
    if !history.iter().any(|e| e.species == cur) || !history.iter().any(|e| e.species == species) {
        return false;
    }
    let Some(mv) = evolution_move_of(cur) else {
        return true;
    };
    // IsMoveSlotAvailable: a slot could have held the move unless all four are egg moves.
    if moves
        .iter()
        .all(|m| matches!(m, MoveResult::Initial | MoveResult::InheritLevelUp))
    {
        return false;
    }
    let i = history
        .iter()
        .position(|e| e.species == cur)
        .map_or(0, |i| i + 1);
    let mut probe = pk.clone();
    probe.set_moves([mv, 0, 0, 0]);
    ctx.verify_moves(&probe, species, &history[i..], None, None)[0].valid()
}

fn is_partial(e: &Enc5, pk: &Pkm) -> bool {
    let Enc5::Data(d) = e else {
        return false;
    };
    let r = match d.kind() {
        Some(EncounterKind::Slot5) => slot_rating(d, pk),
        Some(EncounterKind::Static5) => static_rating(d, pk),
        _ => Rating::Match,
    };
    r >= Rating::Partial
}

fn valid_date(year: u8, month: u8, day: u8) -> bool {
    let y = 2000 + year as u32;
    let leap = y.is_multiple_of(4) && (!y.is_multiple_of(100) || y.is_multiple_of(400));
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 if leap => 29,
        2 => 28,
        _ => return false,
    };
    (1..=days).contains(&day)
}

fn is_number(c: u16) -> bool {
    matches!(c, 0x30..=0x39 | 0xFF10..=0xFF19 | 0xB2 | 0xB3 | 0xB9 | 0xBC..=0xBE)
}

/// `EncounterVerifier` for a hatched or unhatched Gen 5 egg.
fn egg_encounter_ok(ctx: &Context5, pk: &Pkm) -> bool {
    if pk.is_egg() {
        return pk.met_level() == 1
            && !matches!(pk.egg_location(), LINK_TRADE5 | LINK_TRADE5_NPC)
            && matches!(pk.met_location(), 0 | LINK_TRADE5 | LINK_TRADE5_NPC);
    }
    if pk.met_level() != 1 {
        return false;
    }
    let bit = 1u16 << (pk.version() as u16 - VERSION_W as u16).min(3);
    ctx.moves
        .list(mt::G5_HATCH)
        .get(pk.met_location() as usize)
        .is_some_and(|mask| mask & bit != 0)
}

fn encounter_ok(ctx: &Context5, pk: &Pkm, e: &Enc5) -> bool {
    match e {
        Enc5::Egg { .. } => egg_encounter_ok(ctx, pk),
        Enc5::Data(d) if d.has(flag::EGG) && !d.is_slot() => {
            !pk.is_egg() && egg_encounter_ok(ctx, pk) || pk.is_egg()
        }
        _ => true,
    }
}

/// Analyses a PK5 whose origin is Black, White, Black 2 or White 2.
pub fn analyze5<'a>(ctx: &'a Context5<'a>, pk: &Pkm) -> Report<'a> {
    let pi = ctx.b2w2.species_form(pk.species(), pk.form());
    let level = pk.level(&pi.base_info());
    let input = evo_input(pk, level);
    let chain = ctx.evo.encounter_origin_chain5(&input);
    let list = enumerate(ctx, pk, &chain);
    let mut report = Report {
        matched: Matched::Invalid,
        moves: [MoveResult::Unparsed; 4],
        invalid: Vec::new(),
    };
    let n = list.len();
    let mut chosen = None;
    for (i, e) in list.iter().enumerate() {
        let last = i + 1 == n;
        if !encounter_ok(ctx, pk, e) && !last {
            continue;
        }
        let species = enc_species(e);
        let history = ctx
            .evo
            .chain_for_encounter(&input, species, enc_level_min(e));
        let moves = moves_for(ctx, pk, e, &history);
        if !moves.iter().all(|m| m.valid()) && !last {
            continue;
        }
        let ev = evolution_ok(ctx, pk, species, &history, &moves);
        if !ev && !last {
            continue;
        }
        if is_partial(e, pk) && !last {
            continue;
        }
        report.moves = moves;
        if !ev {
            report.flag(CheckId::Evolution);
        }
        chosen = Some(*e);
        break;
    }
    let Some(e) = chosen else {
        report.flag(CheckId::Encounter);
        let history = ctx
            .evo
            .chain_for_encounter(&input, pk.species(), pk.met_level());
        report.moves = ctx.verify_moves(pk, pk.species(), &history, None, None);
        verify_common(ctx, pk, level, None, &mut report);
        return report;
    };
    match e {
        Enc5::Data(encounter) => {
            report.matched = Matched::Slot {
                encounter,
                lead: Default::default(),
            }
        }
        Enc5::Egg { species, .. } => report.matched = Matched::Egg { species },
    }
    if is_partial(&e, pk) || !encounter_ok(ctx, pk, &e) {
        report.flag(CheckId::Encounter);
    }
    verify_common(ctx, pk, level, Some(&e), &mut report);
    report
}

fn verify_common(ctx: &Context5, pk: &Pkm, lvl: u8, enc: Option<&Enc5>, report: &mut Report) {
    let data = match enc {
        Some(Enc5::Data(d)) => Some(d),
        _ => None,
    };
    let egg_enc = matches!(enc, Some(Enc5::Egg { .. }));
    let kind = data.and_then(|d| d.kind());
    let e_species = enc.map_or(pk.species(), enc_species);
    let pi = ctx.b2w2.species_form(pk.species(), pk.form());
    let growth = pi.growth();

    // PIDVerifier: Wurmple evolution.
    if e_species == WURMPLE && pk.species() != WURMPLE {
        let val = ((pk.pid() >> 16) % 10 / 5) as i32;
        let idx = pk.species() as i32 - 266;
        let group = if (idx & 3) != idx { -1 } else { idx >> 1 };
        if val != group {
            report.flag(CheckId::EC);
        }
    }

    // PIDVerifier.VerifyShiny and the Gen 5 PID correlations.
    if let Some(d) = data {
        if !shiny_valid(d.shiny(), pk.is_shiny()) {
            report.flag(CheckId::Shiny);
        }
        let ids = (pk.tid() ^ pk.sid()) as u32 & 1;
        let bit_ok = ids ^ (pk.pid() & 1) ^ (pk.pid() >> 31) == 0;
        let fixed_gender = d
            .template()
            .map(|t| t.gender())
            .filter(|g| matches!(g, 0 | 1))
            .filter(|_| kind != Some(EncounterKind::Trade5BW));
        if let Some(g) = fixed_gender {
            let low = pk.pid() & 0xFF;
            let ok = if g == 0 {
                !matches!(low, 254 | 255)
            } else {
                low != 0
            };
            if !ok {
                report.flag(CheckId::PID);
            }
        }
        let wild_pid = d.has(flag::WILD_CORRELATION_PID) && !d.is_slot();
        if d.is_slot() {
            if d.is_hidden_grotto() {
                if pk.is_shiny() {
                    report.flag(CheckId::Shiny);
                }
            } else if !bit_ok {
                report.flag(CheckId::PID);
            }
        } else if wild_pid && !bit_ok {
            report.flag(CheckId::PID);
        }
    }

    // LevelVerifier.
    if pk.is_egg() {
        let lmin = enc.map_or(pk.met_level(), enc_level_min);
        if lvl != lmin || pk.exp() != exp_for_level(lmin, growth) {
            report.flag(CheckId::Level);
        }
    } else {
        if lvl >= 100 && pk.exp() != exp_for_level(100, growth) {
            report.flag(CheckId::Level);
        }
        if lvl < pk.met_level() {
            report.flag(CheckId::Level);
        }
    }

    // EffortValueVerifier.
    let evs = pk.evs();
    if pk.is_egg() {
        if evs.iter().any(|&v| v != 0) {
            report.flag(CheckId::EVs);
        }
    } else if evs.iter().map(|&v| v as u32).sum::<u32>() > 510 {
        report.flag(CheckId::EVs);
    }

    // AbilityVerifier.
    verify_ability(ctx, pk, enc, report);

    // BallVerifier.
    let ball = pk.ball();
    let ball_ok = match enc {
        Some(Enc5::Egg { .. }) => ball == BALL_POKE,
        Some(Enc5::Data(d)) if d.ball() != 0 => ball == d.ball(),
        Some(Enc5::Data(_)) => {
            const BALLS5: u64 = (1 << 1)
                | (1 << 2)
                | (1 << 3)
                | (1 << 4)
                | (0x7F << 6)
                | (1 << 13)
                | (1 << 14)
                | (1 << 15);
            let dream = if kind == Some(EncounterKind::Entree5) {
                1u64 << 25
            } else {
                0
            };
            ball < 64 && (BALLS5 | dream) & (1u64 << ball) != 0
        }
        None => true,
    };
    if !ball_ok {
        report.flag(CheckId::Ball);
    }

    // MiscVerifier: dates, Pokérus, fateful, Gen 5 fields.
    let pkrs = pk.pokerus();
    if (pkrs & 0xF) as u32 > ((pkrs >> 4) & 3) as u32 + 1 {
        report.flag(CheckId::Misc);
    }
    let [my, mm, md] = pk.met_date();
    if pk.met_location() != 0 {
        if !valid_date(my, mm, md) {
            report.flag(CheckId::Memory);
        }
    } else if enc.is_some() && (my, mm, md) != (0, 0, 0) {
        report.flag(CheckId::Memory);
    }
    let [ey, em, ed] = pk.egg_date();
    if pk.egg_location() != 0 {
        if !valid_date(ey, em, ed) {
            report.flag(CheckId::Memory);
        }
    } else if (ey, em, ed) != (0, 0, 0) {
        report.flag(CheckId::Memory);
    }
    let fateful_expected = data.is_some_and(|d| d.has(flag::FATEFUL));
    if pk.fateful() != fateful_expected {
        report.flag(CheckId::Fateful);
    }
    let fame = pk.decrypted()[0x87];
    if fame != 0 {
        if pk.is_egg() {
            report.flag(CheckId::Egg);
        } else if e_species == DITTO || !matches!(fame % 25, 0 | 5) {
            report.flag(CheckId::Misc);
        }
    }
    let is_n = kind == Some(EncounterKind::Static5N);
    if is_n != pk.n_sparkle() {
        report.flag(CheckId::Fateful);
    }

    // FormVerifier.
    verify_form(ctx, pk, e_species, enc, report);

    // GroundTileVerifier.
    if pk.ground_tile() != 0 {
        report.flag(CheckId::Encounter);
    }

    // ItemVerifier.
    let item = pk.held_item();
    if pk.is_egg() && item != 0 {
        report.flag(CheckId::HeldItem);
    }
    if item != 0 && !ctx.moves.list(mt::RELEASED_HELD).contains(item) {
        report.flag(CheckId::HeldItem);
    }

    // Eggs and MovePPVerifier.
    let (moves, ups, pp) = (pk.moves(), pk.pp_ups(), pk.pp());
    if pk.is_egg() {
        if pk.friendship() > pi.hatch_cycles() {
            report.flag(CheckId::Egg);
        }
        if pk.contest().iter().any(|&c| c != 0) {
            report.flag(CheckId::Egg);
        }
        if ups.iter().any(|&u| u != 0) {
            report.flag(CheckId::Egg);
        }
        if (0..4).any(|i| pp[i] != ctx.moves.max_pp(moves[i], 0)) {
            report.flag(CheckId::Egg);
        }
    } else {
        for i in 0..4 {
            if ups[i] != 0 && matches!(moves[i], 0 | 166) {
                report.flag(CheckId::CurrentMove);
            }
            if pp[i] > ctx.moves.max_pp(moves[i], ups[i]) {
                report.flag(CheckId::CurrentMove);
            }
        }
    }

    // ContestStatVerifier: no way to raise them in a Gen 5 game.
    if pk.contest().iter().any(|&c| c != 0) {
        report.flag(CheckId::Memory);
    }

    // LanguageVerifier.
    let lang = pk.language();
    let missing_ok = kind == Some(EncounterKind::Trade5BW);
    if lang == 6 || lang > 8 || (lang == 0 && !missing_ok) {
        report.flag(CheckId::Language);
    }

    verify_names(ctx, pk, e_species, enc, report);

    // RibbonVerifier.
    let raw = pk.decrypted();
    let args = super::ribbons4::RibbonArgs {
        sinnoh: [raw[0x24], raw[0x25], raw[0x26], raw[0x27]],
        hoenn: [raw[0x3C], raw[0x3D], raw[0x3E], raw[0x3F]],
        contest4: [raw[0x60], raw[0x61], raw[0x62]],
        is_egg: pk.is_egg(),
        event: [0; 3],
        gen4: false,
        species: pk.species(),
        form: pk.form(),
        spiky_static: false,
        gen3: None,
        gen3_origin: false,
    };
    if super::ribbons4::ribbons_invalid(&args) {
        report.flag(CheckId::Ribbon);
    }

    // GenderVerifier.
    let ratio = pi.gender_ratio();
    let gender = pk.gender();
    if (ratio == 255) != (gender == 2) {
        report.flag(CheckId::Gender);
    } else {
        let pid = pk.pid();
        let ok = match ratio {
            255 => gender == 2,
            254 => gender == 1,
            0 => gender == 0,
            r => gender < 2 && gender == gender_from_pid(pid, r),
        };
        let fixed_from_dual = matches!(pk.species(), 292 | 413 | 414 | 416 | 475 | 478 | 538 | 539);
        let ok = ok
            && (!fixed_from_dual
                || gender == 2
                || gender_from_pid(pid, ctx.b2w2.personal(e_species as usize).gender_ratio())
                    == gender);
        if !ok {
            report.flag(CheckId::Gender);
        }
    }

    if egg_enc {
        // PIDVerifier.VerifyEggPID has no Gen 5 branch; nothing further.
    }
}

fn verify_ability(ctx: &Context5, pk: &Pkm, enc: Option<&Enc5>, report: &mut Report) {
    let pi = ctx.b2w2.species_form(pk.species(), pk.form());
    let [a1, a2, ah] = pi.abilities();
    let ability = pk.ability();
    let index = if ability == a1 {
        Some(0u8)
    } else if ability == a2 {
        Some(1)
    } else if ability == ah {
        Some(2)
    } else {
        None
    };
    let Some(index) = index else {
        report.flag(CheckId::Ability);
        return;
    };
    let number = if pk.hidden_ability() {
        4u8
    } else {
        1 << ((pk.pid() >> 16) & 1)
    };
    let (permission, is_egg_enc, e_species) = match enc {
        Some(Enc5::Egg { species, .. }) => (
            if hidden_possible(*species) {
                255
            } else {
                ABILITY_ANY12
            },
            true,
            *species,
        ),
        Some(Enc5::Data(d)) => (d.ability(), false, d.species()),
        None => (ABILITY_ANY12, false, pk.species()),
    };
    let from_number = [a1, a2, ah][(number >> 1) as usize] == ability;
    if permission != 255 {
        if (number == 4) != (permission == ABILITY_HIDDEN) {
            report.flag(CheckId::Ability);
            return;
        }
        if permission != ABILITY_ANY12 {
            let single = match permission {
                1 => 0u8,
                2 => 1,
                _ => 2,
            };
            let ok = if e_species != pk.species() {
                index == single || from_number
            } else {
                index == single || number == 1 << single
            };
            if !ok {
                report.flag(CheckId::Ability);
            }
            return;
        }
    }
    let _ = is_egg_enc;
    if !from_number {
        report.flag(CheckId::Ability);
    }
}

/// `AbilityBreedLegality.IsHiddenPossible5`.
fn hidden_possible(species: u16) -> bool {
    const BAN: [u8; 80] = [
        0x92, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x00, 0x02, 0x10, 0x10, 0x20, 0x00,
        0x01, 0x11, 0x02, 0x00, 0x49, 0x00, 0x00, 0x00, 0x00, 0x00, 0x03, 0x00, 0x00, 0x00, 0x10,
        0x00, 0x90, 0x04, 0x00, 0x00, 0x80, 0x00, 0x00, 0x80, 0x00, 0x00, 0x00, 0x86, 0x80, 0x49,
        0x00, 0x40, 0x00, 0x48, 0x02, 0x00, 0x00, 0x10, 0x00, 0x12, 0x00, 0x00, 0x00, 0x00, 0x80,
        0x00, 0x82, 0x24, 0x80, 0x0A, 0x00, 0x00, 0x0C, 0x00, 0x00, 0x44, 0x44, 0x00, 0x00, 0xA0,
        0x84, 0x80, 0x40, 0x08, 0x12,
    ];
    BAN.get((species >> 3) as usize)
        .is_none_or(|b| b & (1 << (species & 7)) == 0)
}

fn verify_form(ctx: &Context5, pk: &Pkm, e_species: u16, enc: Option<&Enc5>, report: &mut Report) {
    let species = pk.species();
    let form = pk.form();
    let pi = ctx.b2w2.species_form(species, form);
    let fc = pi.form_count();
    if fc <= 1 && form == 0 {
        return;
    }
    let in_range = form < fc;
    let oob_ok = match species {
        201 => form < 28,
        414 => form < 3,
        _ => false,
    };
    if !in_range && !oob_ok {
        report.flag(CheckId::Form);
        return;
    }
    let _ = (e_species, enc);
    let item = pk.held_item();
    let bad = match species {
        487 => (form == 1) != (item == 112),
        493 => {
            const PLATES: [u16; 17] = [
                303, 306, 304, 305, 309, 308, 310, 313, 298, 299, 301, 300, 307, 302, 311, 312, 644,
            ];
            let expect = if (298..=313).contains(&item) || item == 644 {
                PLATES.iter().position(|&p| p == item).map_or(0, |i| i + 1) as u8
            } else {
                0
            };
            expect != form
        }
        649 => {
            let expect = if (116..=119).contains(&item) {
                (item - 115) as u8
            } else {
                0
            };
            expect != form
        }
        ROTOM => pk.is_egg() && form != 0,
        // FormInfo.IsBattleOnlyForm.
        351 | 421 | 555 | 648 => form != 0,
        _ => false,
    };
    if bad {
        report.flag(CheckId::Form);
    }
}

fn eq16(a: &[u16], b: crate::data::blob::U16s) -> bool {
    a.len() == b.len() && a.iter().zip(b.iter()).all(|(x, y)| *x == y)
}

fn verify_names(ctx: &Context5, pk: &Pkm, e_species: u16, enc: Option<&Enc5>, report: &mut Report) {
    let data = match enc {
        Some(Enc5::Data(d)) => Some(d),
        _ => None,
    };
    let mut ot_buf = [0u16; 8];
    let on = crate::text::decode5(&pk.ot_name_raw(), &mut ot_buf);
    let ot = &ot_buf[..on];
    let kind = data.and_then(|d| d.kind());
    let trade = matches!(
        kind,
        Some(EncounterKind::Trade5BW | EncounterKind::Trade5B2W2)
    );
    let is_n = kind == Some(EncounterKind::Static5N);
    let lang = pk.language();

    if is_n {
        let expect: &[u16] = if lang == 1 { &[0xFF2E] } else { &[0x4E] };
        if ot != expect {
            report.flag(CheckId::Trainer);
        }
    } else if !trade {
        if on == 0 || ot.contains(&0xFFFF) {
            report.flag(CheckId::Trainer);
        }
        let max = if matches!(lang, 1 | 8) { 6 } else { 12 };
        if on > max {
            let traded = if pk.is_egg() {
                pk.met_location() == LINK_TRADE5
            } else {
                pk.egg_location() == LINK_TRADE5
            };
            let enc_egg =
                matches!(enc, Some(Enc5::Egg { .. })) || data.is_some_and(|d| d.has(flag::EGG));
            if !(enc_egg && traded && on <= 12) {
                report.flag(CheckId::Trainer);
            }
        }
        if on > 4 && ot.iter().filter(|&&c| is_number(c)).count() > 4 {
            report.flag(CheckId::Trainer);
        }
    }

    let mut buf = [0u16; 11];
    let n = crate::text::decode5(&pk.nickname_raw(), &mut buf);
    let nick = &buf[..n];

    let mut fixed_nickname = false;
    if trade {
        let d = data.unwrap();
        let bw = kind == Some(EncounterKind::Trade5BW);
        if bw && lang == 1 {
            report.flag(CheckId::Language);
        }
        let use_lang = if bw { lang.max(1) } else { lang };
        let ed = if bw { &ctx.enc_bw } else { &ctx.enc_b2w2 };
        fixed_nickname = d.has(flag::FIXED_NICKNAME);
        match ed.trade_names(d, use_lang) {
            Some((name, trainer)) => {
                if !eq16(ot, trainer) {
                    report.flag(CheckId::Trainer);
                }
                if fixed_nickname && !eq16(nick, name) {
                    report.flag(CheckId::Nickname);
                }
            }
            None => {
                report.flag(CheckId::Trainer);
                if fixed_nickname {
                    report.flag(CheckId::Nickname);
                }
            }
        }
    }
    if fixed_nickname {
        return;
    }

    if n == 0 || nick.contains(&0xFFFF) {
        report.flag(CheckId::Nickname);
        return;
    }
    let name_is = |l: u8, sp: u16| {
        let name = ctx.names.get(l as u16, sp as usize);
        !name.is_empty() && eq16(nick, name)
    };
    if pk.is_egg() {
        if pk.is_nicknamed() || !name_is(lang, 0) {
            report.flag(CheckId::Egg);
        }
        return;
    }
    let species = pk.species();
    if pk.is_nicknamed() {
        if nick.iter().any(|&c| (0x4E00..=0x9FFF).contains(&c)) {
            report.flag(CheckId::Nickname);
            return;
        }
        if n > if matches!(lang, 1 | 8) { 5 } else { 10 } {
            report.flag(CheckId::Nickname);
            return;
        }
        if n > 4 && nick.iter().filter(|&&c| is_number(c)).count() > 4 {
            report.flag(CheckId::Nickname);
        }
        return;
    }
    if name_is(lang, species) {
        return;
    }
    let traded_egg = pk.egg_location() == LINK_TRADE5;
    let any_lang = [1u8, 2, 3, 4, 5, 7, 8].iter().any(|&l| name_is(l, species));
    if !((e_species != species || traded_egg) && any_lang) {
        report.flag(CheckId::Nickname);
    }
}
