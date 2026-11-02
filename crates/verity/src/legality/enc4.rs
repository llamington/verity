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
const SAFARI_HGSS: u16 = 202;
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
    // PKHeX `Locations4.IsSafariBallRequired`.
    if (pk.ball == BALL_SAFARI) != matches!(e.location(), MARSH_DPPT | SAFARI_HGSS) {
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
    if e.species() == UNOWN && {
        use crate::rng::unown4 as u;
        let (pid, iv) = (pk.analyze.pid, pk.analyze.iv32);
        if e.location() == u::RUINS_OF_ALPH {
            !u::alph_form_valid(pid, iv, pk.form)
        } else {
            !u::form_valid(pid, iv, pk.form)
        }
    } {
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

/// PKHeX `EncounterEnumerator4` static groups per origin version (Pokéwalker not modelled yet).
pub fn static_groups(version: u8) -> &'static [&'static [u8]] {
    match version {
        7 => &[b"StaticHG", b"Encounter_HGSS"],
        8 => &[b"StaticSS", b"Encounter_HGSS"],
        10 => &[b"StaticD", b"StaticDP", b"StaticDPPt"],
        11 => &[b"StaticP", b"StaticDP", b"StaticDPPt"],
        _ => &[b"StaticPt", b"StaticDPPt"],
    }
}

const GENDER_RANDOM: u8 = 255;
const TILE_GRASS: u8 = 2;
const TILE_WATER: u8 = 7;

/// PKHeX `EncounterStatic4.IsMatchRoamerLocation` permit masks (grass/water per region).
fn roamer_location_ok(location: u16, met: u16, tile: u8) -> bool {
    let (first, permit): (u16, u64) = match (location, tile) {
        (16, TILE_GRASS) => (16, 0x2_8033_FFFF),
        (16, TILE_WATER) => (16, 0x2_803E_3B9E),
        (177, TILE_GRASS) => (177, 0x0003_E7FF),
        (177, TILE_WATER) => (177, 0x0001_E06E),
        (149, TILE_GRASS) => (149, 0x0AB3_FFFF),
        (149, TILE_WATER) => (149, 0x0ABC_1B28),
        _ => return false,
    };
    let v = met.wrapping_sub(first);
    v < 64 && permit & (1u64 << v) != 0
}

/// PKHeX `FormInfo.IsFormChangeable` (Gen 4 species).
fn form_changeable(species: u16) -> bool {
    matches!(species, 412 | 479 | 386 | 483 | 484 | 487 | 492 | 493)
}

/// PKHeX `EncounterStatic4.IsMatchExact` (format 4).
fn static_match_exact(e: &Encounter, pk: &MatchPkm, evo: &EvoCriteria) -> bool {
    let Some(t) = e.template() else {
        return false;
    };
    let is_egg_enc = t.egg_location() != 0;
    // IsMatchEggLocation
    if !is_egg_enc {
        if pk.egg_location != 0 {
            return false;
        }
    } else if !pk.is_egg {
        if pk.egg_location != t.egg_location() && pk.egg_location != 2002 {
            return false;
        }
    } else if pk.egg_location != t.egg_location() || !matches!(pk.met_location, 0 | 2002) {
        return false;
    }
    // IsMatchLocation
    if !is_egg_enc {
        let met = pk.met_location;
        let ok = if e.has(crate::data::encounters::flag::ROAMING) {
            roamer_location_ok(e.location(), met, pk.ground_tile)
        } else {
            met == e.location()
        };
        if !ok {
            return false;
        }
    }
    // IsMatchLevel
    let want = if is_egg_enc { 0 } else { e.level_min() };
    if pk.analyze.met_level != want {
        return false;
    }
    if t.gender() != GENDER_RANDOM && pk.analyze.gender != t.gender() {
        return false;
    }
    !(e.form() != evo.form && !form_changeable(e.species()))
}

/// PKHeX `EncounterStatic4.IsCompatible`.
fn static_compatible(e: &Encounter, pk: &MatchPkm, kind: PidType) -> bool {
    const SHINY_ALWAYS: u8 = 2;
    if e.species() == 172 {
        return kind == PidType::Pokewalker;
    }
    if e.shiny() == SHINY_ALWAYS {
        return kind == PidType::ChainShiny;
    }
    match kind {
        PidType::Method1 => true,
        PidType::CuteCharm => cute_charm_valid(e.species(), pk),
        _ => false,
    }
}

/// Wild slots and statics for a Gen 4 PK4 (D/P/Pt or HG/SS origin) in PKHeX's yield order
/// (`EncounterEnumerator4` + `EncounterGenerator4.GetEncounters`). `group` names the slot
/// group (`SlotsD`/`SlotsP`/`SlotsPt`/`SlotsHG`/`SlotsSS`); statics come from the same data.
pub fn wild_slots<'a>(
    pk: &MatchPkm,
    chain: &[EvoCriteria],
    pidiv: PidIv,
    ed: &EncounterData<'a>,
    group: &[u8],
    ratio: impl Fn(u16) -> u8,
) -> WildMatches<'a> {
    let mut fallback: Option<(Encounter<'a>, Deferral)> = None;
    let mut defer = |d: Deferral, e: Encounter<'a>| {
        if fallback.is_none_or(|(_, cur)| d < cur) {
            fallback = Some((e, d));
        }
    };
    let none = WildMatches {
        candidates: Vec::new(),
        fallback: None,
    };
    // Bred eggs, events and trades are not modelled: they come before statics in PKHeX, so a
    // Pokémon with an egg location is left unmatched (callers report "not checked").
    if pk.is_egg || pk.egg_location != 0 {
        return none;
    }
    let evo_of = |sp: u16| chain.iter().find(|c| c.species == sp);
    let must_be_slot = pk.ball == BALL_SAFARI || pk.ball == BALL_SPORT;

    // EncounterEnumerator4: (encounter, partial) in yield order; the first partial match is
    // deferred to the end (shared across statics and slots).
    let mut seq: Vec<(Encounter<'a>, bool)> = Vec::new();
    let mut partial: Option<Encounter<'a>> = None;
    let push_statics = |seq: &mut Vec<(Encounter<'a>, bool)>,
                        partial: &mut Option<Encounter<'a>>| {
        for g in static_groups(pk.analyze.version) {
            for e in ed.group(g) {
                let Some(evo) = evo_of(e.species()) else {
                    continue;
                };
                if !static_match_exact(&e, pk, evo) {
                    continue;
                }
                let gift = e.ball() == BALL_POKE;
                if gift && pk.ball != e.ball() {
                    if partial.is_none() {
                        *partial = Some(e);
                    }
                } else {
                    seq.push((e, false));
                }
            }
        }
    };
    let push_slots = |seq: &mut Vec<(Encounter<'a>, bool)>, partial: &mut Option<Encounter<'a>>| {
        for e in ed.group(group) {
            if e.location() != pk.met_location {
                continue;
            }
            let Some(evo) = evo_of(e.species()) else {
                continue;
            };
            if !is_match_exact(&e, pk, evo) {
                continue;
            }
            if rating(&e, pk) == Rating::Match {
                seq.push((e, false));
            } else if partial.is_none() {
                *partial = Some(e);
            }
        }
    };
    if must_be_slot {
        // Slots, then statics; the enumerator ends without reaching its Fallback state.
        push_slots(&mut seq, &mut partial);
        push_statics(&mut seq, &mut partial);
    } else {
        push_statics(&mut seq, &mut partial);
        push_slots(&mut seq, &mut partial);
        if let Some(e) = partial {
            seq.push((e, true));
        }
    }

    // EncounterGenerator4.GetEncounters: non-slots yield as they pass; slots queue by lead.
    let mut direct: Vec<SlotMatch<'a>> = Vec::new();
    let mut queue: Vec<SlotMatch<'a>> = Vec::new();
    for (e, is_partial) in seq {
        if e.ground_tile() & (1u32 << pk.ground_tile) == 0 {
            defer(Deferral::Tile, e);
            continue;
        }
        let compatible = if e.is_slot() {
            type_compatible(&e, pk, pidiv.kind)
        } else {
            static_compatible(&e, pk, pidiv.kind)
        };
        if !compatible {
            defer(Deferral::PidIv, e);
            continue;
        }
        if !ball_compatible(&e, pk) {
            defer(Deferral::Ball, e);
            continue;
        }
        if !e.is_slot() {
            if pk.ball == BALL_SAFARI || pk.ball == BALL_SPORT {
                defer(Deferral::Ball, e);
            } else {
                direct.push(SlotMatch {
                    encounter: e,
                    lead: LeadSeed {
                        seed: pidiv.seed,
                        lead: method_j::LeadRequired::None,
                    },
                    partial: is_partial,
                });
            }
            continue;
        }
        let Some(slot) = Slot4::from_encounter(&e) else {
            continue;
        };
        let ml = pk.analyze.met_level;
        let lead = if matches!(pk.analyze.version, 7 | 8) {
            crate::rng::method_k::lead_info4_hgss(&pk.analyze, &slot, pidiv, ml, ml, 4, &ratio)
        } else {
            method_j::lead_info4_dppt(&pk.analyze, &slot, pidiv, ml, ml, 4, &ratio)
        };
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
    direct.extend(queue);
    WildMatches {
        fallback: if direct.is_empty() { fallback } else { None },
        candidates: direct,
    }
}
