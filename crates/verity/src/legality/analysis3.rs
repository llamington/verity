//! Legality of a Gen 3-origin Pokémon in the Gen 4 format (brought over through Pal Park).
//! Ported from PKHeX `EncounterGenerator3`, `EncounterEnumerator3`, `EncounterSlot3`,
//! `EncounterStatic3`, `EncounterTrade3`, `EncounterEgg3`, `LeadFinder`, `TransferVerifier` and
//! the verifiers' Gen 3 branches. Colosseum/XD and Wonder Card events are not modelled yet, so
//! those Pokémon come back as [`Matched::Invalid`].

use alloc::vec::Vec;

use super::analysis4::{
    inputs_for, invalid_moves3, transfer_evolution_valid, transfer_moves, verify_common_origin3,
    CheckId, Context4, Inputs, Matched, Origin3, Report,
};
use super::analysis5::Context5;
use super::enc4::{Deferral, Enc4, MatchPkm, Rating};
use super::evo::{Evo4, EvoCriteria};
use super::moves4::MoveResult;
use super::moves4::{Learn3, Start3};
use crate::data::blob::BlobError;
use crate::data::encounters::{Encounter, EncounterData, EncounterKind};
use crate::data::game_table as gt;
use crate::data::GameData;
use crate::pkm::Pkm;
use crate::rng::finder::{analyze, PidIv, PidType};
use crate::rng::method_h::{self, Bounds, Slot3};
use crate::rng::method_j::LeadSeed;

const VERSION_S: u8 = 1;
const VERSION_R: u8 = 2;
const VERSION_E: u8 = 3;
const VERSION_FR: u8 = 4;
const VERSION_LG: u8 = 5;
const EGG_LEVEL: u8 = 5;
const SAFARI_RSE: u16 = 57;
const SAFARI_FRLG: u16 = 136;
const BALL_POKE: u8 = 4;
const BALL_SAFARI: u8 = 5;
const WURMPLE: u16 = 265;
const UNOWN: u16 = 201;
const TOGEPI: u16 = 175;
const MARILL: u16 = 183;
const AZUMARILL: u16 = 184;
const WOBBUFFET: u16 = 202;

/// `Locations.Transfer3`: Pal Park.
pub const PAL_PARK: u16 = 55;

/// Gen 3 data for Pal Park legality (`g3_origin.bin`).
pub struct Context3<'a> {
    pub learn: Learn3<'a>,
    pub evo: Evo4<'a>,
    pub enc: EncounterData<'a>,
}

impl<'a> Context3<'a> {
    pub fn load(data: &'a [u8]) -> Result<Self, BlobError> {
        let game = GameData::load(data)?;
        Ok(Self {
            learn: Learn3 { data: game },
            evo: Evo4::new(game, game),
            enc: EncounterData::load(game.raw_table(gt::ENCOUNTERS))?,
        })
    }
}

/// Whether a version value is one of the five mainline Gen 3 games.
pub fn is_gba_version(version: u8) -> bool {
    matches!(
        version,
        VERSION_S | VERSION_R | VERSION_E | VERSION_FR | VERSION_LG
    )
}

/// Deferred-match state of `EncounterEnumerator3`.
struct Search<'a, 'p> {
    chain: &'p [EvoCriteria],
    out: Vec<(Enc4<'a>, bool)>,
    deferred: Option<Enc4<'a>>,
}

impl<'a> Search<'a, '_> {
    /// `TryGetNext`: an encounter matched against the first chain entry of its species.
    fn scan(
        &mut self,
        list: impl Iterator<Item = Encounter<'a>>,
        exact: impl Fn(&Encounter<'a>, &EvoCriteria) -> bool,
        rating: impl Fn(&Encounter<'a>) -> Rating,
    ) {
        for e in list {
            let Some(evo) = self.chain.iter().find(|c| c.species == e.species()) else {
                continue;
            };
            if !exact(&e, evo) {
                continue;
            }
            if rating(&e) == Rating::Match {
                self.out.push((Enc4::Data(e), false));
            } else if self.deferred.is_none() {
                self.deferred = Some(Enc4::Data(e));
            }
        }
    }
}

fn colo_rating(e: &Encounter, pk: &Pkm) -> Rating {
    match e.template() {
        Some(t) if super::gift3::colo_partial(&t, pk) => Rating::PartialMatch,
        _ => Rating::Match,
    }
}

fn wurmple_valid(species: u16, pid: u32) -> bool {
    let val = ((pid >> 16) % 10 / 5) as i32;
    let idx = species as i32 - 266;
    let group = if (idx & 3) != idx { -1 } else { idx >> 1 };
    val == group
}

fn is_safari3(location: u16) -> bool {
    matches!(location, SAFARI_RSE | SAFARI_FRLG)
}

fn slot_exact(e: &Encounter, evo: &EvoCriteria) -> bool {
    (e.form() == evo.form || e.species() == 412) && evo.level_max >= e.level_min()
}

fn slot_rating(e: &Encounter, pk: &MatchPkm) -> Rating {
    let species = pk.analyze.species;
    let wurmple_bad =
        e.species() == WURMPLE && species != WURMPLE && !wurmple_valid(species, pk.analyze.pid);
    if (pk.ball == BALL_SAFARI) != is_safari3(e.location()) || wurmple_bad {
        Rating::PartialMatch
    } else {
        Rating::Match
    }
}

/// `FormInfo.IsFormChangeable` for the Gen 3 species with forms that can change.
fn form_changeable(species: u16) -> bool {
    species == 386
}

fn static_exact(e: &Encounter, pk: &MatchPkm, evo: &EvoCriteria) -> bool {
    pk.egg_location == 0
        && evo.level_max >= e.level_min()
        && (e.form() == evo.form || form_changeable(e.species()))
}

fn static_rating(e: &Encounter, pk: &MatchPkm) -> Rating {
    let gift = e.ball() == BALL_POKE;
    let fateful = e.has(crate::data::encounters::flag::FATEFUL);
    if (pk.ball == BALL_SAFARI) != is_safari3(e.location())
        || (gift && pk.ball != e.ball())
        || fateful != pk.fateful
    {
        Rating::PartialMatch
    } else {
        Rating::Match
    }
}

/// `Legal.GetIsFixedIVSequenceValidSkipRand`: -1 (stored as 255) means any value.
fn ivs_match(expected: [u8; 6], pk: &Pkm) -> bool {
    let actual = pk.ivs();
    expected
        .iter()
        .zip(actual.iter())
        .all(|(&e, &a)| e == 255 || e as u16 == a)
}

fn trade_exact(e: &Encounter, pk: &Pkm, evo: &EvoCriteria) -> bool {
    let Some(t) = e.template() else {
        return false;
    };
    pk.pid() == t.pid()
        && ivs_match(t.ivs(), pk)
        && pk.tid() == t.tid()
        && pk.sid() == t.sid()
        && evo.level_max >= e.level_min()
        && (evo.form == e.form() || form_changeable(e.species()))
        && pk.gender() == t.gender()
        && pk.ot_gender() == t.ot_gender()
        && pk.egg_location() == 0
        && pk
            .contest()
            .iter()
            .zip(t.contest().iter())
            .all(|(a, b)| a >= b)
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum TypeCheck {
    Match,
    NotIdeal,
    Mismatch,
}

fn type_check(e: &Enc4, pk: &Pkm, pv: PidIv, mutated: &mut Option<PidType>) -> TypeCheck {
    use PidType::*;
    let kind = pv.kind;
    if let Enc4::Data(d) = e {
        if matches!(
            d.kind(),
            Some(
                EncounterKind::Gift3
                    | EncounterKind::Gift3NY
                    | EncounterKind::Gift3JPN
                    | EncounterKind::Gift3Colo
            )
        ) {
            return if super::gift3::compatible(d, pk, pv, mutated) {
                TypeCheck::Match
            } else {
                TypeCheck::Mismatch
            };
        }
    }
    let yes = |b: bool| {
        if b {
            TypeCheck::Match
        } else {
            TypeCheck::Mismatch
        }
    };
    match e {
        Enc4::Egg3 { .. } => yes(kind == None),
        Enc4::Data(d) => match d.kind() {
            Some(EncounterKind::Slot3 | EncounterKind::Slot3Swarm) => {
                if d.species() != UNOWN {
                    yes(matches!(kind, Method1 | Method2 | Method3 | Method4))
                } else {
                    yes(matches!(
                        kind,
                        Method1Unown | Method2Unown | Method3Unown | Method4Unown
                    ))
                }
            }
            Some(EncounterKind::Static3) => {
                let v = pk.version();
                if v == VERSION_E {
                    return yes(kind == Method1);
                }
                if d.has(crate::data::encounters::flag::ROAMING) {
                    let ivs_small = pk.iv32() & 0x3FFF_FFFF <= 0xFF;
                    return yes(kind == Method1Roamer || (kind == Method1 && ivs_small));
                }
                if kind == Method1 {
                    return TypeCheck::Match;
                }
                let m4 = kind == Method4;
                if matches!(v, VERSION_R | VERSION_S) {
                    return if m4 {
                        TypeCheck::NotIdeal
                    } else {
                        TypeCheck::Mismatch
                    };
                }
                if m4 && d.species() == TOGEPI {
                    TypeCheck::NotIdeal
                } else {
                    TypeCheck::Mismatch
                }
            }
            _ => yes(kind == None),
        },
        _ => TypeCheck::Mismatch,
    }
}

fn ball_compatible(e: &Enc4, pk: &Pkm) -> bool {
    let fixed_safari = matches!(e, Enc4::Data(d) if d.ball() == BALL_SAFARI);
    (fixed_safari && pk.ball() == BALL_SAFARI) || pk.ball() != BALL_SAFARI
}

/// `Breeding.CanHatchAsEgg` for Gen 3 species.
fn can_hatch(species: u16) -> bool {
    !matches!(
        species,
        132 | 144..=146 | 150 | 151 | 201 | 243..=245 | 249..=251 | 377..=386
    )
}

fn split_breed3(species: u16) -> bool {
    matches!(species, MARILL | WOBBUFFET)
}

fn add_eggs<'a>(ctx: &Context3<'a>, chain: &[EvoCriteria], s: &mut Search<'a, '_>, version: u8) {
    let Some(devolved) = chain.last() else {
        return;
    };
    if !(devolved.level_min..=devolved.level_max).contains(&EGG_LEVEL) {
        return;
    }
    let (species, form) = ctx.evo.base_species_form(devolved.species, devolved.form);
    if species != devolved.species && !split_breed3(devolved.species) {
        return;
    }
    if !can_hatch(species) || (form != 0 && matches!(species, 25 | 133 | 172)) {
        return;
    }
    if !ctx.evo.is_present(species, form) {
        return;
    }
    s.out.push((Enc4::Egg3 { species, version }, false));
    let mut d = *devolved;
    if species == d.species {
        if chain.len() < 2 {
            return;
        }
        d = chain[chain.len() - 2];
    }
    if split_breed3(d.species) {
        s.out.push((
            Enc4::Egg3 {
                species: d.species,
                version,
            },
            false,
        ));
    }
}

/// Candidates in `EncounterEnumerator3` order.
fn enumerate<'a>(g3: &Context3<'a>, inp: &Inputs, chain: &[EvoCriteria]) -> Vec<(Enc4<'a>, bool)> {
    let pk = &inp.pk;
    let version = pk.version();
    let m = &inp.matcher;
    let mut s = Search {
        chain,
        out: Vec::new(),
        deferred: None,
    };
    if chain.is_empty() {
        return s.out;
    }
    let ed = &g3.enc;
    // EncounterEnumerator3 Event states.
    let gift = |e: &Encounter<'a>, evo: &EvoCriteria| super::gift3::is_match_exact(ed, e, pk, evo);
    s.scan(ed.group(b"ColoGiftsR"), gift, |e| colo_rating(e, pk));
    s.scan(ed.group(b"ColoGiftsS"), gift, |e| colo_rating(e, pk));
    s.scan(ed.group(b"Encounter_WC3"), gift, |_| Rating::Match);
    if super::gift3::pcny_language(pk.language()) {
        s.scan(ed.group(b"PCNY"), gift, |_| Rating::Match);
    } else if super::gift3::pcjp_language(pk.language()) {
        s.scan(ed.group(b"PCJP"), gift, |_| Rating::Match);
    }
    let trade_groups: &[&[u8]] = match version {
        VERSION_E => &[b"TradeGift_E"],
        VERSION_FR => &[b"TradeGift_FR", b"TradeGift_FRLG"],
        VERSION_LG => &[b"TradeGift_LG", b"TradeGift_FRLG"],
        _ => &[b"TradeGift_RS", b"TradeGift_E"],
    };
    for name in trade_groups {
        s.scan(
            ed.group(name),
            |e, evo| trade_exact(e, pk, evo),
            |_| Rating::Match,
        );
    }
    let (slots, swarm, statics): (&[u8], bool, &[&[u8]]) = match version {
        VERSION_R => (b"SlotsR", true, &[b"StaticR", b"StaticRSE"]),
        VERSION_S => (b"SlotsS", true, &[b"StaticS", b"StaticRSE"]),
        VERSION_E => (b"SlotsE", true, &[b"StaticE", b"StaticRSE"]),
        VERSION_FR => (b"SlotsFR", false, &[b"StaticFR", b"StaticFRLG"]),
        _ => (b"SlotsLG", false, &[b"StaticLG", b"StaticFRLG"]),
    };
    let wild_ok = !pk.is_egg() && pk.egg_location() == 0;
    let scan_slots = |s: &mut Search<'a, '_>| {
        if !wild_ok {
            return;
        }
        let exact = |e: &Encounter<'a>, evo: &EvoCriteria| slot_exact(e, evo);
        let rate = |e: &Encounter<'a>| slot_rating(e, m);
        s.scan(ed.group(slots), exact, rate);
        if swarm {
            s.scan(ed.group(b"SlotsSwarmRSE"), exact, rate);
        }
    };
    let scan_statics = |s: &mut Search<'a, '_>| {
        for name in statics {
            s.scan(
                ed.group(name),
                |e, evo| static_exact(e, m, evo),
                |e| static_rating(e, m),
            );
        }
    };
    if pk.ball() == BALL_SAFARI {
        scan_slots(&mut s);
        scan_statics(&mut s);
    } else {
        scan_statics(&mut s);
        scan_slots(&mut s);
    }
    add_eggs(g3, chain, &mut s, version);
    if let Some(d) = s.deferred.take() {
        s.out.push((d, true));
    }
    s.out
}

/// `EncounterGenerator3.Deferral.Update`.
fn defer_update<'a>(cur: &mut Option<(Deferral, Enc4<'a>)>, d: Deferral, e: Enc4<'a>) {
    if cur.is_none_or(|(c, _)| d < c) {
        *cur = Some((d, e));
    }
}

/// Lists the candidates `EncounterGenerator3.GetEncounters` yields, then chooses one.
pub fn analyze3<'a>(ctx: &'a Context4<'a>, g3: &'a Context3<'a>, pk: &Pkm) -> Report<'a> {
    analyze3_inner(ctx, g3, None, pk)
}

/// A Gen 3 origin now in the Gen 5 format.
pub fn analyze3_transfer<'a>(
    ctx: &'a Context4<'a>,
    g3: &'a Context3<'a>,
    c5: &'a Context5<'a>,
    pk: &Pkm,
) -> Report<'a> {
    analyze3_inner(ctx, g3, Some(c5), pk)
}

fn analyze3_inner<'a>(
    ctx: &'a Context4<'a>,
    g3: &'a Context3<'a>,
    c5: Option<&'a Context5<'a>>,
    pk: &Pkm,
) -> Report<'a> {
    let inp = inputs_for(ctx, c5, pk);
    let chain = match c5 {
        Some(c) => c
            .evo
            .transfer_origin_chain((&ctx.evo, Some(&g3.evo)), &inp.evo),
        None => ctx.evo.encounter_origin_chain3(&g3.evo, &inp.evo),
    };
    let ratio = |s: u16| ctx.ratio(s);
    let pidiv = analyze(&inp.analyze, ratio);
    let mut direct: Vec<(Enc4<'a>, LeadSeed, bool)> = Vec::new();
    let mut queue: Vec<(Enc4<'a>, LeadSeed, bool)> = Vec::new();
    let mut defer: Option<(Deferral, Enc4<'a>)> = None;
    let gender = if matches!(pk.species(), MARILL | AZUMARILL) {
        crate::pkm::derived::gender_from_pid(pk.pid(), 127)
    } else {
        pk.gender()
    };
    let classic = matches!(
        pidiv.kind,
        PidType::Method1
            | PidType::Method2
            | PidType::Method3
            | PidType::Method4
            | PidType::Method1Unown
            | PidType::Method2Unown
            | PidType::Method3Unown
            | PidType::Method4Unown
    );
    let mut mutated = None;
    for (e, partial) in enumerate(g3, &inp, &chain) {
        match type_check(&e, pk, pidiv, &mut mutated) {
            TypeCheck::Mismatch => {
                defer_update(&mut defer, Deferral::PidIv, e);
                continue;
            }
            TypeCheck::NotIdeal => {
                defer_update(&mut defer, Deferral::PidIvDefer, e);
                continue;
            }
            TypeCheck::Match => {}
        }
        if !ball_compatible(&e, pk) {
            defer_update(&mut defer, Deferral::Ball, e);
            continue;
        }
        let slot = match &e {
            Enc4::Data(d) if d.is_slot() => *d,
            _ => {
                direct.push((e, LeadSeed::default(), partial));
                continue;
            }
        };
        let lead = if classic {
            let evo = chain.iter().find(|c| c.species == slot.species());
            let (min, max) = evo.map_or((0, 0), |c| {
                (
                    c.level_min.max(slot.level_min()),
                    c.level_max.min(slot.level_max()),
                )
            });
            method_h::get_seed(
                &Slot3::from_encounter(&slot),
                pidiv.seed,
                Bounds {
                    min,
                    max,
                    format: if c5.is_some() { 5 } else { 4 },
                },
                pk.version() == VERSION_E,
                gender,
                |s| g3.learn.data.personal(s as usize).gender_ratio(),
            )
        } else {
            LeadSeed::default()
        };
        if !lead.is_valid() {
            defer_update(&mut defer, Deferral::SlotNumber, e);
            continue;
        }
        let at = queue.iter().position(|(_, l, _)| lead.lead > l.lead);
        queue.insert(at.unwrap_or(queue.len()), (e, lead, partial));
    }
    direct.extend(queue);
    let mut list: Vec<Candidate<'a>> = direct
        .into_iter()
        .map(|(encounter, lead, _)| Candidate {
            encounter,
            lead,
            deferral: None,
        })
        .collect();
    if let Some((d, encounter)) = defer {
        list.push(Candidate {
            encounter,
            lead: LeadSeed::default(),
            deferral: Some(d),
        });
    }
    let mut report = Report {
        matched: Matched::Invalid,
        moves: [MoveResult::Unparsed; 4],
        invalid: Vec::new(),
    };
    choose3(ctx, g3, &inp, &list, &mut report);
    report
}

struct Candidate<'a> {
    encounter: Enc4<'a>,
    lead: LeadSeed,
    deferral: Option<Deferral>,
}

fn is_partial(e: &Enc4, inp: &Inputs) -> bool {
    let pk = &inp.matcher;
    match e {
        Enc4::Data(d) if d.is_slot() => slot_rating(d, pk) != Rating::Match,
        Enc4::Data(d) if d.kind() == Some(EncounterKind::Static3) => {
            static_rating(d, pk) != Rating::Match
        }
        Enc4::Data(d) if d.kind() == Some(EncounterKind::Gift3Colo) => d
            .template()
            .is_some_and(|t| super::gift3::colo_partial(&t, &inp.pk)),
        _ => false,
    }
}

fn enc_species3(e: &Enc4) -> u16 {
    match e {
        Enc4::Data(d) => d.species(),
        Enc4::Egg3 { species, .. } => *species,
        _ => 0,
    }
}

fn enc_level3(e: &Enc4) -> u8 {
    match e {
        Enc4::Data(d) => d.level_min(),
        _ => EGG_LEVEL,
    }
}

/// How a candidate's moves started.
fn start_of(g3: &Context3, e: &Enc4) -> Start3 {
    match e {
        Enc4::Egg3 { species, version } => Start3::Egg {
            species: *species,
            version: *version,
        },
        Enc4::Data(d) if d.is_slot() => g3.enc.swarm_moves(d).map_or(Start3::Wild, Start3::Fixed),
        Enc4::Data(d) if d.has(crate::data::encounters::flag::HAS_MOVES) => d
            .template()
            .map_or(Start3::Wild, |t| Start3::Fixed(t.moves())),
        _ => Start3::Wild,
    }
}

/// `LearnVerifier` for a candidate: the fixed moves of a static or swarm, a hatched egg's moves.
fn moves_for(
    ctx: &Context4,
    g3: &Context3,
    inp: &Inputs,
    e: &Enc4,
    history: (&[EvoCriteria], &[EvoCriteria]),
) -> [MoveResult; 4] {
    let pk = &inp.pk;
    let species = enc_species3(e);
    let start = start_of(g3, e);
    ctx.learn
        .verify3(&g3.learn, pk.moves(), pk.form(), species, history, start)
}

/// `EvolutionVerifier.VerifyEvolution` across Gen 3 and Gen 4.
fn evolution_ok(
    ctx: &Context4,
    g3: &Context3,
    inp: &Inputs,
    species: u16,
    (gen4, gen3): (&[EvoCriteria], &[EvoCriteria]),
) -> bool {
    let cur = inp.pk.species();
    if cur == species {
        return true;
    }
    if !gen4.iter().any(|c| c.species == cur) || !gen3.iter().any(|c| c.species == species) {
        return false;
    }
    let Some(mv) = super::analysis4::evolution_move_of(cur) else {
        return true;
    };
    let pruned = |h: &[EvoCriteria]| -> Vec<EvoCriteria> {
        let i = h.iter().position(|c| c.species == cur).map_or(0, |i| i + 1);
        h[i..].to_vec()
    };
    let (p4, p3) = (pruned(gen4), pruned(gen3));
    let r = ctx.learn.verify3(
        &g3.learn,
        [mv, 0, 0, 0],
        inp.pk.form(),
        species,
        (&p4, &p3),
        Start3::Wild,
    );
    r[0].valid()
}

/// `EncounterVerifier.VerifyEncounterEgg3Transfer`.
fn egg_transfer_ok(pk: &Pkm, f5: bool) -> bool {
    !pk.is_egg()
        && pk.met_level() >= EGG_LEVEL
        && pk.egg_location() == 0
        && pk.met_location() == transfer_location(f5)
}

/// Where a Gen 3 origin was "met": Pal Park (Gen 4 format) or Poké Transfer (Gen 5 format).
fn transfer_location(f5: bool) -> u16 {
    if f5 {
        30001
    } else {
        PAL_PARK
    }
}

/// `EncounterVerifier.VerifyEncounter` for a Gen 3 origin after leaving Gen 3.
fn encounter_ok(pk: &Pkm, e: &Enc4, f5: bool) -> bool {
    match e {
        Enc4::Egg3 { .. } => egg_transfer_ok(pk, f5),
        Enc4::Data(d)
            if d.kind() == Some(EncounterKind::Gift3)
                && d.has(crate::data::encounters::flag::EGG)
                && !pk.is_egg() =>
        {
            egg_transfer_ok(pk, f5)
        }
        Enc4::Data(d) if d.kind() == Some(EncounterKind::Static3) => {
            let japanese = pk.language() == 1;
            !((d.species() == 151 && !japanese)
                || (d.species() == 386 && d.location() == 200 && japanese))
                && (pk.is_egg()
                    || !d.has(crate::data::encounters::flag::EGG)
                    || egg_transfer_ok(pk, f5))
        }
        _ => true,
    }
}

/// `EncounterFinder.FindVerifiedEncounter`, then the encounter verifiers.
fn choose3<'a>(
    ctx: &'a Context4<'a>,
    g3: &'a Context3<'a>,
    inp: &Inputs,
    list: &[Candidate<'a>],
    report: &mut Report<'a>,
) {
    let pk = &inp.pk;
    let f5 = inp.fmt5.is_some();
    let mut chosen = None;
    let n = list.len();
    for (i, c) in list.iter().enumerate() {
        let last = i + 1 == n;
        if !encounter_ok(pk, &c.encounter, f5) && !last {
            continue;
        }
        let species = enc_species3(&c.encounter);
        let level = enc_level3(&c.encounter);
        let [gen3, gen4, gen5] = match inp.fmt5 {
            Some(c5) => {
                c5.evo
                    .transfer_history((&ctx.evo, Some(&g3.evo)), &inp.evo, species, level)
            }
            None => {
                let (a, b) = ctx
                    .evo
                    .chain_for_encounter3(&g3.evo, &inp.evo, species, level);
                [a, b, Vec::new()]
            }
        };
        let (moves, ev) = match inp.fmt5 {
            Some(c5) => {
                let (fixed, egg3) = match start_of(g3, &c.encounter) {
                    Start3::Fixed(m) => (Some(m), None),
                    Start3::Egg { species, version } => (None, Some((species, version))),
                    Start3::Wild => (None, None),
                };
                let h = [gen3.clone(), gen4.clone(), gen5.clone()];
                let learn = Some((&g3.learn, egg3));
                let m = transfer_moves(ctx, c5, pk, pk.moves(), species, &h, fixed, None, learn);
                let ev = transfer_evolution_valid(ctx, c5, inp, species, &h, None, Some(&g3.learn));
                (m, ev)
            }
            None => (
                moves_for(ctx, g3, inp, &c.encounter, (&gen4, &gen3)),
                evolution_ok(ctx, g3, inp, species, (&gen4, &gen3)),
            ),
        };
        if !moves.iter().all(|m| m.valid()) && !last {
            continue;
        }
        if !ev && !last {
            continue;
        }
        if is_partial(&c.encounter, inp) && !last {
            continue;
        }
        report.moves = moves;
        if !ev {
            report.flag(CheckId::Evolution);
        }
        chosen = Some((c, gen3, gen4, gen5));
        break;
    }
    let Some((c, gen3, gen4, gen5)) = chosen else {
        report.flag(CheckId::Encounter);
        report.moves = match inp.fmt5 {
            Some(c5) => {
                let h = c5.evo.transfer_history(
                    (&ctx.evo, Some(&g3.evo)),
                    &inp.evo,
                    pk.species(),
                    pk.met_level(),
                );
                let learn = Some((&g3.learn, None));
                transfer_moves(ctx, c5, pk, pk.moves(), pk.species(), &h, None, None, learn)
            }
            None => invalid_moves3(ctx, g3, inp),
        };
        verify_common_origin3(ctx, inp, None, None, None, report);
        return;
    };
    let origin = Origin3 {
        g3,
        gen3: &gen3,
        gen4: &gen4,
        gen5: &gen5,
    };
    match c.encounter {
        Enc4::Data(encounter) => {
            report.matched = Matched::Slot {
                encounter,
                lead: c.lead,
            };
        }
        Enc4::Egg3 { species, .. } => report.matched = Matched::Egg { species },
        _ => {}
    }
    if is_partial(&c.encounter, inp) {
        report.flag(CheckId::Encounter);
    }
    // A Gen 3 frame mismatch is only Fishy in PKHeX (`RNGFrameNotFound3`), so it isn't flagged.
    if c.deferral == Some(Deferral::PidIv) {
        report.flag(CheckId::PID);
    }
    // EncounterVerifier's Gen 3 rules, then TransferVerifier.VerifyTransferLegalityG3.
    if !encounter_ok(pk, &c.encounter, f5) || pk.met_location() != transfer_location(f5) {
        report.flag(CheckId::Encounter);
    }
    // PIDVerifier.VerifyEggPID, including VerifyEggGender8000 (gender from PID bit 15).
    if let Enc4::Egg3 { species, version } = c.encounter {
        // Daycare3.IsValidProcPID: the Day Care never stores a zero PID half (R/S/FR/LG) or PID.
        let pid_ok = if version == VERSION_E {
            pk.pid() != 0
        } else {
            pk.pid() & 0xFFFF != 0
        };
        if !pid_ok {
            report.flag(CheckId::EC);
        }
        if matches!(species, 29 | 32 | 313 | 314) {
            let bit = ((pk.pid() >> 15) & 1) as u8;
            let inherited = report.moves.contains(&MoveResult::InheritLevelUp);
            if pk.gender() == bit && (pk.gender() == 1 || inherited) {
                report.flag(CheckId::EC);
            }
        }
    }
    let (data, egg) = match &c.encounter {
        Enc4::Egg3 { species, .. } => (None, Some(*species)),
        e => (Some(*e), None),
    };
    verify_common_origin3(ctx, inp, data.as_ref(), egg, Some(&origin), report);
}
