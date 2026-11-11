//! Gen 4 D/P/Pt wild-slot encounter matching. Ported from PKHeX
//! `Encounters/Generator/ByGeneration/EncounterGenerator4.cs` (slot path),
//! `EncounterEnumerator4.TryGetNext/TryGetNextSub`, `EncounterSlot4.IsMatchExact/GetMatchRating/
//! IsCompatible`, `RNG/ClassicEra/LeadEncounter.cs` and `CuteCharm4.IsValid`.

use alloc::boxed::Box;
use alloc::vec::Vec;

use super::evo::EvoCriteria;
use super::ranch4::{self, RANCH_GIFTS, RanchGift};
use crate::data::encounters::{Encounter, EncounterData};
use crate::gift::pcd::Gift4;
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
    pub fateful: bool,
    pub ball: u8,
    pub ground_tile: u8,
    pub is_shiny: bool,
    /// Contest stats (cool, beauty, cute, smart, tough, sheen).
    pub contest: [u8; 6],
    /// The Unown form is possible at its met location (PKHeX `IsUnownFormValid`). It depends on
    /// the PID, IVs and location, not the slot, so it is worked out once per Pokémon.
    pub unown_form_ok: bool,
    /// The Unown form is possible at the Ruins of Alph, for a Pokémon whose met location is gone.
    pub unown_alph_ok: bool,
    /// The Pokémon has left Gen 4, which loses its met location and original met level.
    pub format5: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Rating {
    Match,
    PartialMatch,
}

/// A Gen 4 encounter candidate: a blob template (slot, static, trade, Pokéwalker), a Wonder
/// Card, or the Ranger Manaphy egg (PKHeX `EncounterGenerator4.RangerManaphy`).
#[derive(Clone, Copy)]
pub enum Enc4<'a> {
    Data(Encounter<'a>),
    Gift(Gift4<'a>),
    Ranger,
    /// A My Pokémon Ranch gift (D only).
    Ranch(&'static RanchGift),
    /// A Gen 3 bred egg (PKHeX `EncounterEgg3`).
    Egg3 {
        species: u16,
        version: u8,
    },
}

impl<'a> Enc4<'a> {
    pub fn data(&self) -> Option<&Encounter<'a>> {
        match self {
            Enc4::Data(e) => Some(e),
            _ => None,
        }
    }
}

#[derive(Clone, Copy)]
pub struct SlotMatch<'a> {
    pub encounter: Enc4<'a>,
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
    let unown_ok = if pk.format5 && e.location() == crate::rng::unown4::RUINS_OF_ALPH {
        pk.unown_alph_ok
    } else {
        pk.unown_form_ok
    };
    if e.species() == UNOWN && !unown_ok {
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
    if pk.format5 {
        if evo.level_max < e.level_min() {
            return false;
        }
    } else if !(e.level_min()..=e.level_max()).contains(&ml)
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
    pub fallback: Option<(Enc4<'a>, Deferral)>,
}

/// `EncounterGenerator4.GetEncounters` filters for a non-slot that is not an
/// `IGroundTypeTile` (tile must be 0), then PID-IV type, then ball. Returns the deferral, or
/// `None` when the encounter is yielded.
fn gift_filter(ball: u8, compatible: bool, pk: &MatchPkm) -> Option<Deferral> {
    if pk.ground_tile != 0 {
        return Some(Deferral::Tile);
    }
    if !compatible {
        return Some(Deferral::PidIv);
    }
    let ball_ok = match ball {
        BALL_SAFARI if pk.ball == BALL_SAFARI => true,
        BALL_SPORT if pk.ball == BALL_SPORT => true,
        _ => pk.ball != BALL_SAFARI && pk.ball != BALL_SPORT,
    };
    if !ball_ok || pk.ball == BALL_SAFARI || pk.ball == BALL_SPORT {
        return Some(Deferral::Ball);
    }
    None
}

/// Event encounters (Wonder Cards or the Ranger Manaphy) that the enumerator yielded, run
/// through the generator filters. `list` holds each candidate with its ball and whether its
/// PID type is compatible (`IRandomCorrelation.IsCompatible`).
pub fn event_matches<'a>(pk: &MatchPkm, list: &[(Enc4<'a>, u8, bool)]) -> WildMatches<'a> {
    let mut fallback: Option<(Enc4<'a>, Deferral)> = None;
    let mut candidates = Vec::new();
    for &(e, ball, compatible) in list {
        match gift_filter(ball, compatible, pk) {
            None => candidates.push(SlotMatch {
                encounter: e,
                lead: LeadSeed::default(),
                partial: false,
            }),
            Some(d) => {
                if fallback.is_none_or(|(_, cur)| d < cur) {
                    fallback = Some((e, d));
                }
            }
        }
    }
    WildMatches {
        fallback: if candidates.is_empty() {
            fallback
        } else {
            None
        },
        candidates,
    }
}

/// PKHeX `EncounterEnumerator4` static groups per origin version (Pokéwalker not modelled yet).
pub fn static_groups(version: u8) -> &'static [&'static [u8]] {
    match version {
        7 => &[b"StaticHG", b"Encounter_HGSS", b"Encounter_PokeWalker"],
        8 => &[b"StaticSS", b"Encounter_HGSS", b"Encounter_PokeWalker"],
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
pub(crate) fn form_changeable(species: u16) -> bool {
    matches!(species, 412 | 479 | 386 | 483 | 484 | 487 | 492 | 493)
}

/// PKHeX `Locations.IsPtHGSSLocationEgg`.
fn is_pt_hgss_egg_location(location: u16) -> bool {
    (2011..3000).contains(&location)
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
    } else if pk.format5 {
        // Poké Transfer turns Pt/HG/SS egg locations into Faraway Place.
        let expect = if is_pt_hgss_egg_location(t.egg_location()) {
            3002
        } else {
            t.egg_location()
        };
        if pk.egg_location != 2002 && pk.egg_location != expect {
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
    if !is_egg_enc && !pk.format5 {
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
    if pk.format5 {
        if e.level_min() > evo.level_max {
            return false;
        }
    } else if pk.analyze.met_level != want {
        return false;
    }
    if t.gender() != GENDER_RANDOM && pk.analyze.gender != t.gender() {
        return false;
    }
    !(e.form() != evo.form && !form_changeable(e.species()))
}

fn is_walker(e: &Encounter) -> bool {
    e.kind() == Some(crate::data::encounters::EncounterKind::Pokewalker4)
}

/// PKHeX `EncounterStatic4Pokewalker.IsMatchExact` (format 4; `IsMatchGender` always passes
/// apart from an Azurill quirk that also returns true).
fn walker_match_exact(e: &Encounter, pk: &MatchPkm, evo: &EvoCriteria) -> bool {
    let at_place = if pk.format5 {
        evo.level_max >= e.level_min()
    } else {
        pk.met_location == e.location() && pk.analyze.met_level == e.level_min()
    };
    pk.egg_location == 0 && at_place && !(e.form() != evo.form && !form_changeable(e.species()))
}

/// PKHeX `PokewalkerRNG.GetLeastEffortSeed(...).Type != None`: the mic test reaches any seed,
/// so this holds whenever the IV halves have an LCRNG seed at all.
fn walker_seed_exists(iv32: u32) -> bool {
    let ivs: [u8; 6] = core::array::from_fn(|i| ((iv32 >> (5 * i)) & 0x1F) as u8);
    let mut buf = [0u32; crate::rng::reversal::MAX_SEEDS_IV];
    crate::rng::reversal::seeds_from_ivs(&mut buf, ivs) != 0
}

/// PKHeX `EncounterMatchRating` for statics: 0 Match, 1 DeferredErrors, 2 PartialMatch.
fn static_rating(e: &Encounter, pk: &MatchPkm) -> u8 {
    if is_walker(e) {
        if pk.ball != BALL_POKE || !walker_seed_exists(pk.analyze.iv32) {
            return 2;
        }
        return if pk.is_shiny { 1 } else { 0 };
    }
    if e.ball() == BALL_POKE && pk.ball != e.ball() {
        2
    } else {
        0
    }
}

/// Whether PKHeX would rate this encounter `PartialMatch` (`EncounterFinder` then reports it).
pub fn data_is_partial(e: &Encounter, pk: &MatchPkm) -> bool {
    if e.is_slot() {
        rating(e, pk) != Rating::Match
    } else {
        static_rating(e, pk) == 2
    }
}

pub fn is_trade(e: &Encounter) -> bool {
    e.kind() == Some(crate::data::encounters::EncounterKind::Trade4Pid)
}

/// PKHeX `EncounterEnumerator4` trade groups (D's Pal Park ranch gifts not modelled yet).
fn trade_groups(version: u8) -> &'static [&'static [u8]] {
    match version {
        7 | 8 => &[b"TradeGift_HGSS"],
        _ => &[b"TradeGift_DPPtIngame"],
    }
}

/// PKHeX `EncounterTrade4PID.IsMatchExact` (format 4).
fn trade_match_exact(e: &Encounter, pk: &MatchPkm, evo: &EvoCriteria) -> bool {
    let Some(t) = e.template() else {
        return false;
    };
    let a = &pk.analyze;
    let met_unset = t.met_location() == 0;
    let level_ok = if pk.format5 {
        evo.level_max >= e.level_min()
    } else if met_unset {
        a.met_level >= e.level_min()
    } else {
        a.met_level == e.level_min()
    };
    let ivs: [u8; 6] = core::array::from_fn(|i| ((a.iv32 >> (5 * i)) & 0x1F) as u8);
    let t_ivs = t.ivs();
    // IndividualValueSet order HP, Atk, Def, Spe, SpA, SpD (same as the PK4 IV32 order).
    level_ok
        && (a.sid as u32) << 16 | a.tid as u32 == (t.sid() as u32) << 16 | t.tid() as u32
        && (pk.format5 || pk.met_location == e.location())
        && ivs == t_ivs
        && a.pid == t.pid()
        && (a.pid % 25) as u8 == t.nature()
        && !(evo.form != e.form() && !form_changeable(e.species()))
        && a.ot_gender == t.ot_gender()
        && pk.egg_location == 0
        && !pk
            .contest
            .iter()
            .zip(t.contest())
            .any(|(&have, need)| have < need)
}

/// PKHeX `EncounterStatic4.IsCompatible` / `EncounterStatic4Pokewalker.IsCompatible`.
fn static_compatible(
    e: &Encounter,
    pk: &MatchPkm,
    kind: PidType,
    ratio: &impl Fn(u16) -> u8,
) -> bool {
    if is_walker(e) {
        return match kind {
            PidType::Pokewalker => true,
            PidType::CuteCharm => {
                crate::rng::finder::is_cute_charm(&pk.analyze, ratio)
                    && cute_charm_valid(e.species(), pk)
            }
            _ => false,
        };
    }
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
    event_partial: Option<Gift4<'a>>,
) -> WildMatches<'a> {
    let mut fallback: Option<(Enc4<'a>, Deferral)> = None;
    let mut defer = |d: Deferral, e: Enc4<'a>| {
        if fallback.is_none_or(|(_, cur)| d < cur) {
            fallback = Some((e, d));
        }
    };
    // Bred eggs are handled by the caller; any other egg location can only be a gift-egg static.
    let gift_egg_only = pk.egg_location != 0;
    let evo_of = |sp: u16| chain.iter().find(|c| c.species == sp);
    let must_be_slot = pk.ball == BALL_SAFARI || pk.ball == BALL_SPORT;

    // EncounterEnumerator4: (encounter, partial) in yield order; the first partial match is
    // deferred to the end (shared across statics and slots).
    let mut seq: Vec<(Enc4<'a>, bool)> = Vec::new();
    // The enumerator's Deferred: the best (lowest) non-Match rating, first on ties. A gift
    // that only partially matched (Event state, PartialMatch) comes first.
    let mut partial: Option<(Enc4<'a>, u8)> = event_partial.map(|g| (Enc4::Gift(g), 2));
    fn keep<'b>(partial: &mut Option<(Enc4<'b>, u8)>, e: Enc4<'b>, r: u8) {
        if partial.is_none_or(|(_, cur)| r < cur) {
            *partial = Some((e, r));
        }
    }
    let push_statics = |seq: &mut Vec<(Enc4<'a>, bool)>, partial: &mut Option<(Enc4<'a>, u8)>| {
        for g in static_groups(pk.analyze.version) {
            for e in ed.group(g) {
                let Some(evo) = evo_of(e.species()) else {
                    continue;
                };
                let exact = if is_walker(&e) {
                    walker_match_exact(&e, pk, evo)
                } else {
                    static_match_exact(&e, pk, evo)
                };
                if !exact {
                    continue;
                }
                match static_rating(&e, pk) {
                    0 => seq.push((Enc4::Data(e), false)),
                    r => keep(partial, Enc4::Data(e), r),
                }
            }
        }
    };
    let push_slots = |seq: &mut Vec<(Enc4<'a>, bool)>, partial: &mut Option<(Enc4<'a>, u8)>| {
        // Poké Transfer loses the met location, so every area has to be tried.
        let areas: Box<dyn Iterator<Item = Encounter<'a>>> = if pk.format5 {
            Box::new(ed.group(group).filter(|e| e.is_slot()))
        } else {
            Box::new(ed.slots_at(group, pk.met_location))
        };
        for e in areas {
            let Some(evo) = evo_of(e.species()) else {
                continue;
            };
            if !is_match_exact(&e, pk, evo) {
                continue;
            }
            if rating(&e, pk) == Rating::Match {
                seq.push((Enc4::Data(e), false));
            } else {
                keep(partial, Enc4::Data(e), 2);
            }
        }
    };
    // TradeRanch: Diamond's Ranch gifts are the first trades tried.
    if pk.analyze.version == 10 {
        for g in &RANCH_GIFTS {
            if evo_of(g.species).is_some_and(|evo| ranch4::is_match_exact(g, pk, evo)) {
                seq.push((Enc4::Ranch(g), false));
            }
        }
    }
    // Trades come before captures (TradeStart state), with no partial ratings.
    for g in trade_groups(pk.analyze.version)
        .iter()
        .filter(|_| !gift_egg_only)
    {
        for e in ed.group(g) {
            if let Some(evo) = evo_of(e.species())
                && trade_match_exact(&e, pk, evo)
            {
                seq.push((Enc4::Data(e), false));
            }
        }
    }
    if gift_egg_only {
        push_statics(&mut seq, &mut partial);
        if let Some((e, _)) = partial {
            seq.push((e, true));
        }
    } else if must_be_slot {
        // Slots, then statics; the enumerator ends without reaching its Fallback state.
        push_slots(&mut seq, &mut partial);
        push_statics(&mut seq, &mut partial);
    } else {
        push_statics(&mut seq, &mut partial);
        push_slots(&mut seq, &mut partial);
        if let Some((e, _)) = partial {
            seq.push((e, true));
        }
    }

    // EncounterGenerator4.GetEncounters: non-slots yield as they pass; slots queue by lead.
    let mut direct: Vec<SlotMatch<'a>> = Vec::new();
    let mut queue: Vec<SlotMatch<'a>> = Vec::new();
    for (enc, is_partial) in seq {
        let e = match enc {
            Enc4::Data(e) => e,
            Enc4::Gift(g) => {
                let t = g.pk();
                let ok = crate::legality::gift4::pcd_compatible(&t, pidiv.kind);
                match gift_filter(t.ball(), ok, pk) {
                    Some(d) => defer(d, enc),
                    None => direct.push(SlotMatch {
                        encounter: enc,
                        lead: LeadSeed::default(),
                        partial: is_partial,
                    }),
                }
                continue;
            }
            Enc4::Ranger | Enc4::Egg3 { .. } => continue,
            Enc4::Ranch(_) => {
                // Not an IGroundTypeTile or IRandomCorrelation, like the in-game trades.
                let safari_sport = pk.ball == BALL_SAFARI || pk.ball == BALL_SPORT;
                if pk.ground_tile != 0 {
                    defer(Deferral::Tile, enc);
                } else if pidiv.kind != PidType::None {
                    defer(Deferral::PidIv, enc);
                } else if safari_sport {
                    // IsBallCompatible fails on a Poké Ball gift; on a Safari Ball gift the
                    // non-slot Safari/Sport deferral applies. Either way, Ball.
                    defer(Deferral::Ball, enc);
                } else {
                    direct.push(SlotMatch {
                        encounter: enc,
                        lead: LeadSeed::default(),
                        partial: false,
                    });
                }
                continue;
            }
        };
        if is_trade(&e) {
            // Not an IGroundTypeTile: the tile must be 0; not an IRandomCorrelation: no PID-IV.
            if pk.ground_tile != 0 {
                defer(Deferral::Tile, Enc4::Data(e));
                continue;
            }
            if pidiv.kind != PidType::None {
                defer(Deferral::PidIv, Enc4::Data(e));
                continue;
            }
            if !ball_compatible(&e, pk) {
                defer(Deferral::Ball, Enc4::Data(e));
                continue;
            }
            if pk.ball == BALL_SAFARI || pk.ball == BALL_SPORT {
                defer(Deferral::Ball, Enc4::Data(e));
            } else {
                direct.push(SlotMatch {
                    encounter: Enc4::Data(e),
                    lead: LeadSeed::default(),
                    partial: false,
                });
            }
            continue;
        }
        // Pokéwalker encounters aren't IGroundTypeTile: the tile just has to be empty.
        let tile_ok = if is_walker(&e) {
            pk.ground_tile == 0
        } else {
            e.ground_tile() & (1u32 << pk.ground_tile) != 0
        };
        if !tile_ok {
            defer(Deferral::Tile, Enc4::Data(e));
            continue;
        }
        let compatible = if e.is_slot() {
            type_compatible(&e, pk, pidiv.kind)
        } else {
            static_compatible(&e, pk, pidiv.kind, &ratio)
        };
        if !compatible {
            defer(Deferral::PidIv, Enc4::Data(e));
            continue;
        }
        if !ball_compatible(&e, pk) {
            defer(Deferral::Ball, Enc4::Data(e));
            continue;
        }
        if !e.is_slot() {
            if pk.ball == BALL_SAFARI || pk.ball == BALL_SPORT {
                defer(Deferral::Ball, Enc4::Data(e));
            } else {
                direct.push(SlotMatch {
                    encounter: Enc4::Data(e),
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
        // LeadFinder.GetLevelConstraint.
        let (lo, hi, format) = if pk.format5 {
            let evo = evo_of(e.species()).copied().unwrap_or_default();
            (
                evo.level_min.max(e.level_min()),
                evo.level_max.min(e.level_max()),
                5,
            )
        } else {
            let ml = pk.analyze.met_level;
            (ml, ml, 4)
        };
        let lead = if matches!(pk.analyze.version, 7 | 8) {
            crate::rng::method_k::lead_info4_hgss(&pk.analyze, &slot, pidiv, lo, hi, format, &ratio)
        } else {
            method_j::lead_info4_dppt(&pk.analyze, &slot, pidiv, lo, hi, format, &ratio)
        };
        if !lead.is_valid() {
            defer(Deferral::SlotNumber, Enc4::Data(e));
            continue;
        }
        let item = SlotMatch {
            encounter: Enc4::Data(e),
            lead,
            partial: is_partial,
        };
        let at = queue
            .iter()
            .position(|q| lead.lead > q.lead.lead)
            .unwrap_or(queue.len());
        queue.insert(at, item);
    }
    // GetEncounters: the deferred encounter still follows statics and trades that were yielded;
    // only a queued slot ends the search before it.
    let queued = !queue.is_empty();
    direct.extend(queue);
    WildMatches {
        fallback: if queued { None } else { fallback },
        candidates: direct,
    }
}
