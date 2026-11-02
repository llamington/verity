//! Gen 4 D/P/Pt wild-slot encounter matching. Ported from PKHeX
//! `Encounters/Generator/ByGeneration/EncounterGenerator4.cs` (slot path),
//! `EncounterEnumerator4.TryGetNext/TryGetNextSub`, `EncounterSlot4.IsMatchExact/GetMatchRating/
//! IsCompatible`, `RNG/ClassicEra/LeadEncounter.cs` and `CuteCharm4.IsValid`.

use alloc::vec::Vec;

use super::evo::EvoCriteria;
use crate::data::encounters::{Encounter, EncounterData};
use crate::rng::finder::{AnalyzeInput, PidIv, PidType};
use crate::rng::method_j::{self, LeadSeed, Slot4, SlotType4};

const BALL_SAFARI: u8 = 5;
const BALL_SPORT: u8 = 24;
const BALL_POKE: u8 = 4;
const MARSH_DPPT: u16 = 52;
const SHEDINJA: u16 = 292;
const WURMPLE: u16 = 265;
const SILCOON: u16 = 266;
const MARILL: u16 = 183;
const AZUMARILL: u16 = 184;
const AZURILL: u16 = 298;
const MUNCHLAX: u16 = 446;
const UNOWN: u16 = 201;
const BURMY: u16 = 412;

/// PK4 fields the slot matcher reads.
#[derive(Clone, Copy, Debug)]
pub struct MatchPkm {
    pub analyze: AnalyzeInput,
    pub form: u8,
    pub met_location: u16,
    pub egg_location: u16,
    pub is_egg: bool,
    pub ball: u8,
    pub ground_tile: u8,
    pub is_shiny: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Rating {
    Match,
    PartialMatch,
}

#[derive(Clone, Copy)]
pub struct SlotMatch<'a> {
    pub encounter: Encounter<'a>,
    pub lead: LeadSeed,
    /// Only a partial match (PKHeX `EncounterMatchRating` worse than `Match`).
    pub partial: bool,
}

fn wurmple_valid(species: u16, pid: u32) -> bool {
    let val = ((pid >> 16) % 10 / 5) as i32;
    let idx = species as i32 - SILCOON as i32;
    let group = if (idx & 3) != idx { -1 } else { idx >> 1 };
    val == group
}

pub fn rating(e: &Encounter, pk: &MatchPkm) -> Rating {
    let species = pk.analyze.species;
    if (pk.ball == BALL_SAFARI) != (e.location() == MARSH_DPPT) {
        return Rating::PartialMatch;
    }
    if (pk.ball == BALL_SPORT) != (e.slot_type() == SlotType4::BugContest as u8)
        && !(species == SHEDINJA && pk.ball == BALL_POKE)
    {
        return Rating::PartialMatch;
    }
    if e.species() == WURMPLE && species != WURMPLE && !wurmple_valid(species, pk.analyze.pid) {
        return Rating::PartialMatch;
    }
    // D/P/Pt Unown only appear in the Solaceon Ruins (PKHeX `EncounterArea4.IsUnownFormValid`).
    if e.species() == UNOWN
        && !crate::rng::unown4::form_valid(pk.analyze.pid, pk.analyze.iv32, pk.form)
    {
        return Rating::PartialMatch;
    }
    Rating::Match
}

fn is_match_exact(e: &Encounter, pk: &MatchPkm, evo: &EvoCriteria) -> bool {
    if e.form() != evo.form
        && e.species() != BURMY
        && !e.has(crate::data::encounters::flag::RANDOM_UNSPECIFIC_FORM)
    {
        return false;
    }
    let ml = pk.analyze.met_level;
    if !(e.level_min()..=e.level_max()).contains(&ml)
        && !(e.slot_type() == SlotType4::Grass as u8 && ml == e.pressure_level())
    {
        return false;
    }
    !(e.slot_type() == SlotType4::HoneyTree as u8
        && e.species() == MUNCHLAX
        && !is_munchlax_tree(pk.analyze.tid, pk.analyze.sid, e.location()))
}

/// PKHeX `EncounterArea4.LocationID_HoneyTree`.
const HONEY_TREE_LOCATIONS: [u16; 21] = [
    20, 20, 21, 22, 23, 24, 25, 25, 26, 27, 27, 28, 29, 30, 33, 36, 37, 47, 48, 49, 58,
];

/// PKHeX `HoneyTreeUtil.CalculateMunchlaxTrees` (including its overlap quirk).
pub fn munchlax_trees(tid: u16, sid: u16) -> [u8; 4] {
    let id = (sid as u32) << 16 | tid as u32;
    let mut r = id.to_be_bytes().map(|b| b % 21);
    for i in 1..4 {
        for j in 0..i {
            if r[i] != r[j] {
                continue;
            }
            r[i] += 1;
            if r[i] >= 21 {
                r[i] = 0;
            }
        }
    }
    r
}

pub fn is_munchlax_tree(tid: u16, sid: u16, location: u16) -> bool {
    munchlax_trees(tid, sid)
        .iter()
        .any(|&t| HONEY_TREE_LOCATIONS[t as usize] == location)
}

fn cute_charm_valid(enc_species: u16, pk: &MatchPkm) -> bool {
    let a = &pk.analyze;
    if a.gender > 1 {
        return a.species == SHEDINJA;
    }
    if !matches!(a.species, MARILL | AZUMARILL) {
        return true;
    }
    if !(0xC8..=0xE0).contains(&a.pid) {
        return true;
    }
    enc_species == AZURILL
}

fn type_compatible(e: &Encounter, pk: &MatchPkm, kind: PidType) -> bool {
    match kind {
        PidType::Method1 => true,
        PidType::ChainShiny => pk.is_shiny && e.can_radar(),
        PidType::CuteCharm => cute_charm_valid(e.species(), pk),
        _ => false,
    }
}

fn ball_compatible(e: &Encounter, pk: &MatchPkm) -> bool {
    match e.ball() {
        BALL_SAFARI if pk.ball == BALL_SAFARI => true,
        BALL_SPORT if pk.ball == BALL_SPORT => true,
        _ => pk.ball != BALL_SAFARI && pk.ball != BALL_SPORT,
    }
}

/// PKHeX `EncounterGenerator4.DeferralType` (order matters: lower is preferred).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Deferral {
    None,
    PidIvDefer,
    SlotNumber,
    Ball,
    Tile,
    PidIv,
}

pub struct WildMatches<'a> {
    /// Valid candidates in PKHeX yield order (lead quality, then data order).
    pub candidates: Vec<SlotMatch<'a>>,
    /// Yielded as a last resort when there are no candidates.
    pub fallback: Option<(Encounter<'a>, Deferral)>,
}

/// Wild slots for a D/P/Pt PK4 in PKHeX's yield order. `group` is `SlotsD`, `SlotsP` or
/// `SlotsPt`.
pub fn wild_slots<'a>(
    pk: &MatchPkm,
    chain: &[EvoCriteria],
    pidiv: PidIv,
    ed: &EncounterData<'a>,
    group: &[u8],
    ratio: impl Fn(u16) -> u8,
) -> WildMatches<'a> {
    let mut queue: Vec<SlotMatch<'a>> = Vec::new();
    let mut fallback: Option<(Encounter<'a>, Deferral)> = None;
    let mut defer = |d: Deferral, e: Encounter<'a>| {
        if fallback.is_none_or(|(_, cur)| d < cur) {
            fallback = Some((e, d));
        }
    };
    if pk.is_egg || pk.egg_location != 0 {
        return WildMatches {
            candidates: queue,
            fallback: None,
        };
    }
    // EncounterEnumerator4: Match slots in data order, then the first partial match last.
    let mut yielded: Vec<(Encounter<'a>, bool)> = Vec::new();
    let mut partial: Option<Encounter<'a>> = None;
    for e in ed.group(group) {
        if e.location() != pk.met_location {
            continue;
        }
        let Some(evo) = chain.iter().find(|c| c.species == e.species()) else {
            continue;
        };
        if !is_match_exact(&e, pk, evo) {
            continue;
        }
        if rating(&e, pk) == Rating::Match {
            yielded.push((e, false));
        } else if partial.is_none() {
            partial = Some(e);
        }
    }
    // `mustBeSlot` (Safari/Sport Ball) iterates statics after slots and ends without reaching the
    // enumerator's Fallback state, so the deferred partial match is never yielded.
    let must_be_slot = pk.egg_location == 0 && (pk.ball == BALL_SAFARI || pk.ball == BALL_SPORT);
    if let Some(e) = partial.filter(|_| !must_be_slot) {
        yielded.push((e, true));
    }
    for (e, is_partial) in yielded {
        if e.ground_tile() & (1u32 << pk.ground_tile) == 0 {
            defer(Deferral::Tile, e);
            continue;
        }
        if !type_compatible(&e, pk, pidiv.kind) {
            defer(Deferral::PidIv, e);
            continue;
        }
        if !ball_compatible(&e, pk) {
            defer(Deferral::Ball, e);
            continue;
        }
        let Some(slot) = Slot4::from_encounter(&e) else {
            continue;
        };
        let ml = pk.analyze.met_level;
        let lead = method_j::lead_info4_dppt(&pk.analyze, &slot, pidiv, ml, ml, 4, &ratio);
        if !lead.is_valid() {
            defer(Deferral::SlotNumber, e);
            continue;
        }
        let item = SlotMatch {
            encounter: e,
            lead,
            partial: is_partial,
        };
        let at = queue
            .iter()
            .position(|q| lead.lead > q.lead.lead)
            .unwrap_or(queue.len());
        queue.insert(at, item);
    }
    WildMatches {
        fallback: if queue.is_empty() { fallback } else { None },
        candidates: queue,
    }
}
