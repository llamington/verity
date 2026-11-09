//! `LegalityAnalysis` for PK4 in the Gen 4 format, D/P/Pt wild-slot scope (M2). Ported from
//! PKHeX `LegalityAnalysis.cs`, `Encounters/Generator/EncounterFinder.cs`,
//! `Encounters/Verifiers/EncounterVerifier.cs`, `Verifiers/EvolutionVerifier.cs`,
//! `Restrictions/EvolutionRestrictions.cs` and the verifiers named on each check below.

use alloc::vec::Vec;

use super::breed4::EggMove;
use super::enc4::{event_matches, wild_slots, Deferral, Enc4, MatchPkm, WildMatches};
use super::evo::{Evo4, EvoCriteria, EvoPkm};
use super::gift4;
use super::moves4::{Learn4, MoveResult};
use super::ranch4::{self, RanchGift, RANCH_GIFTS};
use crate::data::encounters::{Encounter, EncounterData};
use crate::data::GameData;
use crate::gift::pcd::{Gift4, Gifts4};
use crate::pkm::Pkm;
use crate::rng::finder::{analyze, AnalyzeInput};
use crate::rng::method_j::LeadSeed;

/// PKHeX `CheckIdentifier` (subset used by Gen 4), named as PKHeX prints them.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum CheckId {
    CurrentMove,
    Encounter,
    Shiny,
    EC,
    PID,
    Gender,
    EVs,
    Language,
    Nickname,
    Trainer,
    IVs,
    Level,
    Ball,
    Form,
    Egg,
    Misc,
    Fateful,
    Ribbon,
    Ability,
    Evolution,
    Nature,
    GameOrigin,
    HeldItem,
    Marking,
    TrashBytes,
    SlotType,
    Memory,
}

impl CheckId {
    pub fn name(self) -> &'static str {
        use CheckId::*;
        match self {
            CurrentMove => "CurrentMove",
            Encounter => "Encounter",
            Shiny => "Shiny",
            EC => "EC",
            PID => "PID",
            Gender => "Gender",
            EVs => "EVs",
            Language => "Language",
            Nickname => "Nickname",
            Trainer => "Trainer",
            IVs => "IVs",
            Level => "Level",
            Ball => "Ball",
            Form => "Form",
            Egg => "Egg",
            Misc => "Misc",
            Fateful => "Fateful",
            Ribbon => "Ribbon",
            Ability => "Ability",
            Evolution => "Evolution",
            Nature => "Nature",
            GameOrigin => "GameOrigin",
            HeldItem => "HeldItem",
            Marking => "Marking",
            TrashBytes => "TrashBytes",
            SlotType => "SlotType",
            Memory => "Memory",
        }
    }
}

pub struct Context4<'a> {
    pub hgss: GameData<'a>,
    pub learn: Learn4<'a>,
    pub evo: Evo4<'a>,
    pub enc_dppt: EncounterData<'a>,
    pub enc_hgss: EncounterData<'a>,
    /// Gender ratio for species > 493 (for PKHeX `EntityGender` lookups).
    pub gen5: GameData<'a>,
    /// Gen 4-formatted species names by language ID (`names4.bin`).
    pub names4: crate::data::Strings<'a>,
    /// PKHeX's Gen 4 event database (`gifts4.bin`); `Gifts4::empty()` disables gifts.
    pub gifts: Gifts4<'a>,
}

impl Context4<'_> {
    pub fn ratio(&self, species: u16) -> u8 {
        if species <= 493 {
            self.hgss.personal(species as usize).gender_ratio()
        } else {
            self.gen5.personal(species as usize).gender_ratio()
        }
    }
}

#[derive(Clone, Copy)]
pub enum Matched<'a> {
    Slot {
        encounter: Encounter<'a>,
        lead: LeadSeed,
    },
    /// A bred egg (PKHeX `EncounterEgg4`) of this species, hatched in this version.
    Egg {
        species: u16,
    },
    /// A Wonder Card from PKHeX's event database.
    Gift {
        gift: Gift4<'a>,
    },
    /// The Pokémon Ranger Manaphy egg (PKHeX `EncounterGenerator4.RangerManaphy`).
    RangerManaphy,
    /// A My Pokémon Ranch gift.
    Ranch {
        gift: &'static RanchGift,
    },
    Invalid,
}

pub struct Report<'a> {
    pub matched: Matched<'a>,
    pub moves: [MoveResult; 4],
    pub invalid: Vec<CheckId>,
}

impl Report<'_> {
    pub fn valid(&self) -> bool {
        self.invalid.is_empty() && self.moves.iter().all(|m| m.valid())
    }

    fn flag(&mut self, id: CheckId) {
        if !self.invalid.contains(&id) {
            self.invalid.push(id);
        }
    }
}

struct Inputs {
    pk: Pkm,
    level: u8,
    evo: EvoPkm,
    analyze: AnalyzeInput,
    matcher: MatchPkm,
}

fn inputs(ctx: &Context4, pk: &Pkm) -> Inputs {
    let pi = ctx.hgss.species_form(pk.species(), pk.form());
    let level = pk.level(&pi.base_info());
    let analyze = AnalyzeInput::from_pkm(pk);
    Inputs {
        pk: pk.clone(),
        level,
        evo: EvoPkm {
            species: pk.species(),
            form: pk.form(),
            gender: pk.gender(),
            contest_beauty: pk.contest()[1],
            current_level: level,
            met_level: pk.met_level(),
            is_egg: pk.is_egg(),
            egg_location: pk.egg_location(),
        },
        analyze,
        matcher: MatchPkm {
            analyze,
            form: pk.form(),
            met_location: pk.met_location(),
            egg_location: pk.egg_location(),
            is_egg: pk.is_egg(),
            ball: pk.ball(),
            ground_tile: pk.ground_tile(),
            is_shiny: pk.is_shiny(),
            contest: pk.contest(),
            unown_form_ok: pk.species() != 201 || unown_form_ok(pk),
        },
    }
}

/// `EncounterArea4.IsUnownFormValid` for the Pokémon's met location.
fn unown_form_ok(pk: &Pkm) -> bool {
    use crate::rng::unown4 as u;
    let (pid, iv32, form) = (pk.pid(), pk.iv32() & 0x3FFF_FFFF, pk.form());
    if pk.met_location() == u::RUINS_OF_ALPH {
        u::alph_form_valid(pid, iv32, form)
    } else {
        u::form_valid(pid, iv32, form)
    }
}

const EVO_MOVE: [(u16, u16); 7] = [
    (122, 102),
    (185, 102),
    (424, 458),
    (463, 205),
    (465, 246),
    (469, 246),
    (473, 246),
];

/// PKHeX `EvolutionRestrictions.GetSpeciesEvolutionMove` (Gen 4 species).
fn evolution_move(species: u16) -> Option<u16> {
    EVO_MOVE
        .iter()
        .find(|(s, _)| *s == species)
        .map(|(_, m)| *m)
}

/// PKHeX `EvolutionVerifier.VerifyEvolution` for a Gen 4-only history.
fn evolution_valid(
    ctx: &Context4,
    inp: &Inputs,
    enc_species: u16,
    history: &[EvoCriteria],
    moves: &[MoveResult; 4],
) -> bool {
    let cur = inp.pk.species();
    if cur == enc_species {
        return true;
    }
    if !history.iter().any(|e| e.species == cur)
        || !history.iter().any(|e| e.species == enc_species)
    {
        return false;
    }
    let Some(mv) = evolution_move(cur) else {
        return true;
    };
    // IsMoveSlotAvailable: no egg-source moves for wild slots, so a slot is always free.
    let _ = moves;
    let idx = history
        .iter()
        .position(|e| e.species == cur)
        .map_or(0, |i| i + 1);
    let pruned = &history[idx..];
    let r = ctx
        .learn
        .verify([mv, 0, 0, 0], inp.pk.form(), enc_species, pruned, None);
    r[0].valid()
}

/// Analyses a PK4 whose origin is D/P/Pt or HG/SS (versions 7, 8, 10..=12) in the Gen 4
/// format: wild slots, statics, trades, Pokéwalker, hatched eggs and Mystery Gifts.
pub fn analyze4<'a>(ctx: &'a Context4<'a>, pk: &Pkm) -> Report<'a> {
    let inp = inputs(ctx, pk);
    let ratio = |s: u16| ctx.ratio(s);
    let mut report = Report {
        matched: Matched::Invalid,
        moves: [MoveResult::Unparsed; 4],
        invalid: Vec::new(),
    };

    // EncounterFinder.FindVerifiedEncounter
    let chain = ctx.evo.encounter_origin_chain(&inp.evo);
    // EncounterEnumerator4 Event states: fateful Pokémon try the event database first, and
    // stop there once any gift matched.
    let mut event_partial = None;
    if pk.fateful() && !chain.is_empty() {
        if gift4::is_ranger_manaphy(pk) {
            let pidiv = analyze(&inp.analyze, ratio);
            let ok = gift4::manaphy_compatible(pidiv.kind, pk);
            let m = event_matches(&inp.matcher, &[(Enc4::Ranger, 4, ok)]);
            choose(ctx, &inp, Some(m), &mut report);
            return report;
        }
        let ev = gift4::search(&ctx.gifts, pk, &chain, |t| {
            t.level(&ctx.hgss.species_form(t.species(), t.form()).base_info())
        });
        if !ev.matches.is_empty() {
            let pidiv = analyze(&inp.analyze, ratio);
            let list: Vec<(Enc4, u8, bool)> = ev
                .matches
                .iter()
                .map(|g| {
                    let t = g.pk();
                    (
                        Enc4::Gift(*g),
                        t.ball(),
                        gift4::pcd_compatible(&t, pidiv.kind),
                    )
                })
                .collect();
            let m = event_matches(&inp.matcher, &list);
            choose(ctx, &inp, Some(m), &mut report);
            return report;
        }
        event_partial = ev.partial;
    }
    if egg_location_bred4(pk.egg_location(), pk.version()) {
        // Eggs are yielded first, then the D Ranch gifts (a hatched Wynaut has a Day Care egg
        // location), so a Ranch gift can only follow an egg that failed its checks.
        let ranch_follows = pk.version() == 10
            && chain.iter().any(|evo| {
                RANCH_GIFTS.iter().any(|g| {
                    g.species == evo.species && ranch4::is_match_exact(g, &inp.matcher, evo)
                })
            });
        let passed = analyze_bred_egg(ctx, &inp, &chain, &mut report);
        // A deferred Ranch gift ties with a deferred egg, and the egg came first.
        let ranch_yielded = pk.ground_tile() == 0
            && !matches!(pk.ball(), 5 | 24)
            && analyze(&inp.analyze, ratio).kind == crate::rng::finder::PidType::None;
        let egg_found = matches!(report.matched, Matched::Egg { .. });
        if passed || !ranch_follows || (!ranch_yielded && egg_found) {
            return report;
        }
        report = Report {
            matched: Matched::Invalid,
            moves: [MoveResult::Unparsed; 4],
            invalid: Vec::new(),
        };
    }
    let pidiv = analyze(&inp.analyze, ratio);
    let (group, enc_data): (&[u8], _) = match pk.version() {
        7 => (b"SlotsHG", &ctx.enc_hgss),
        8 => (b"SlotsSS", &ctx.enc_hgss),
        10 => (b"SlotsD", &ctx.enc_dppt),
        11 => (b"SlotsP", &ctx.enc_dppt),
        _ => (b"SlotsPt", &ctx.enc_dppt),
    };
    let m = if chain.is_empty() {
        None
    } else {
        Some(wild_slots(
            &inp.matcher,
            &chain,
            pidiv,
            enc_data,
            group,
            ratio,
            event_partial,
        ))
    };
    choose(ctx, &inp, m, &mut report);
    report
}

/// Species of an encounter candidate.
fn species_of(e: &Enc4) -> u16 {
    match e {
        Enc4::Data(e) => e.species(),
        Enc4::Gift(g) => g.species(),
        Enc4::Ranger => 490,
        Enc4::Ranch(g) => g.species,
    }
}

/// PKHeX `IEncounterTemplate.LevelMin` (gifts: `PCD.Level`, the template's met level).
fn enc_level_min(e: &Enc4) -> u8 {
    match e {
        Enc4::Data(e) => e.level_min(),
        Enc4::Gift(g) => g.pk().met_level(),
        Enc4::Ranger => 1,
        Enc4::Ranch(g) => g.level,
    }
}

/// The encounter's fixed moveset (PKHeX `IMoveset` with `HasMoves`), if it has one.
fn enc_moves(e: &Enc4) -> Option<[u16; 4]> {
    match e {
        Enc4::Data(e) => static_moves(e),
        Enc4::Gift(g) => {
            let m = g.pk().moves();
            (m[0] != 0).then_some(m)
        }
        Enc4::Ranger => None,
        Enc4::Ranch(g) => Some(g.moves),
    }
}

/// EncounterFinder: the first candidate whose moves and evolution check out (a partial match
/// only when nothing else follows), else the last; then the encounter verifiers.
fn choose<'a>(ctx: &Context4, inp: &Inputs, m: Option<WildMatches<'a>>, report: &mut Report<'a>) {
    let pk = &inp.pk;
    let mut chosen: Option<(Enc4<'a>, LeadSeed, Option<Deferral>)> = None;
    let mut partial_chosen = false;
    let mut evo_ok = true;
    if let Some(m) = &m {
        let mut list: Vec<(Enc4, LeadSeed, Option<Deferral>, bool)> = m
            .candidates
            .iter()
            .map(|c| (c.encounter, c.lead, None, c.partial))
            .collect();
        if let Some((e, d)) = m.fallback {
            list.push((e, LeadSeed::default(), Some(d), false));
        }
        let n = list.len();
        for (i, (e, lead, d, is_partial)) in list.into_iter().enumerate() {
            let last = i + 1 == n;
            let species = species_of(&e);
            let history = ctx
                .evo
                .chain_for_encounter(&inp.evo, species, enc_level_min(&e));
            let moves = if pk.is_egg() {
                enc_egg_moves(ctx, pk, &e)
            } else {
                ctx.learn
                    .verify(pk.moves(), pk.form(), species, &history, enc_moves(&e))
            };
            if !moves.iter().all(|r| r.valid()) && !last {
                continue;
            }
            let ev = evolution_valid(ctx, inp, species, &history, &moves);
            if !ev && !last {
                continue;
            }
            if is_partial && !last {
                continue;
            }
            evo_ok = ev;
            report.moves = moves;
            partial_chosen = match &e {
                Enc4::Data(d) => super::enc4::data_is_partial(d, &inp.matcher),
                Enc4::Gift(g) => gift4::is_partial(&g.pk(), pk),
                Enc4::Ranger | Enc4::Ranch(_) => false,
            };
            chosen = Some((e, lead, d));
            break;
        }
    }

    let Some((enc, lead, deferral)) = chosen else {
        report.flag(CheckId::Encounter);
        report.moves = invalid_moves(ctx, inp);
        verify_common(ctx, inp, None, report);
        return;
    };
    report.matched = match enc {
        Enc4::Data(encounter) => Matched::Slot { encounter, lead },
        Enc4::Gift(gift) => Matched::Gift { gift },
        Enc4::Ranger => Matched::RangerManaphy,
        Enc4::Ranch(gift) => Matched::Ranch { gift },
    };
    // EncounterFinder adds EncInvalid when the chosen encounter is only a partial match.
    if partial_chosen {
        report.flag(CheckId::Encounter);
    }
    if !evo_ok {
        report.flag(CheckId::Evolution);
    }
    match deferral {
        Some(Deferral::SlotNumber) => report.flag(CheckId::PID),
        Some(Deferral::PidIv) => report.flag(CheckId::PID),
        _ => {}
    }
    // EncounterVerifier: a gift egg (static with an egg location, or an event egg) hatched in
    // this game.
    let hatched_gift = match &enc {
        Enc4::Data(e) => !pk.is_egg() && e.template().is_some_and(|t| t.egg_location() != 0),
        Enc4::Gift(g) => g.is_egg() && !pk.is_egg(),
        Enc4::Ranger => !pk.is_egg(),
        Enc4::Ranch(_) => false,
    };
    if hatched_gift {
        verify_hatched4(pk, report);
    }
    verify_common(ctx, inp, Some(&enc), report);
}

/// PKHeX `EncounterVerifier.VerifyEncounterEgg4` (native format 4, hatched).
fn verify_hatched4(pk: &Pkm, report: &mut Report) {
    if pk.met_level() != 0 {
        report.flag(CheckId::Encounter);
        return;
    }
    let met = pk.met_location();
    let korean_shaymin = pk.language() == 8 && matches!(met, 63 | 85);
    let ok = !korean_shaymin
        && (super::hatch4::valid_met4(met, pk.version())
            || (pk.egg_location() == LINK_TRADE4 && super::hatch4::valid_met4_any(met)));
    if !ok {
        report.flag(CheckId::Encounter);
    }
}

/// PKHeX `EncounterVerifier.VerifyUnhatchedEgg` (Gen 4: met level 0, traded eggs at 2002).
fn verify_unhatched4(pk: &Pkm, report: &mut Report) {
    let met = pk.met_location();
    if pk.met_level() != 0 || pk.egg_location() == LINK_TRADE4 || !matches!(met, 0 | LINK_TRADE4) {
        report.flag(CheckId::Encounter);
    }
}

const WURMPLE: u16 = 265;
const PICHU: u16 = 172;
const VOLT_TACKLE: u16 = 344;
const DAYCARE4: u16 = 2000;
const LINK_TRADE4: u16 = 2002;
const FARAWAY4: u16 = 3002;

/// PKHeX `Locations.IsEggLocationBred4`.
fn egg_location_bred4(loc: u16, version: u8) -> bool {
    matches!(loc, DAYCARE4 | LINK_TRADE4) || (loc == FARAWAY4 && matches!(version, 7 | 8 | 12))
}

/// PKHeX `Breeding.IsSplitBreedNotBabySpecies4`.
fn split_breed4(species: u16) -> bool {
    matches!(species, 183 | 202 | 113 | 122 | 143 | 185 | 226 | 315 | 358)
}

/// PKHeX `Breeding.CanHatchAsEgg(species)` for Gen 4 species.
fn can_hatch(species: u16) -> bool {
    !matches!(
        species,
        132 | 144..=146 | 150 | 151 | 201 | 243..=245 | 249..=251 | 377..=386 | 480..=493
    ) || matches!(species, 489 | 490)
}

/// PKHeX `EncounterGenerator4.TryGetEgg` / `TryGetSplit`: candidate egg species, in order.
fn egg_species(ctx: &Context4, chain: &[EvoCriteria]) -> Vec<u16> {
    let mut out = Vec::new();
    let Some(devolved) = chain.last() else {
        return out;
    };
    if !(devolved.level_min..=devolved.level_max).contains(&1) {
        return out;
    }
    let (species, form) = ctx.evo.base_species_form(devolved.species, devolved.form);
    if species != devolved.species && !split_breed4(devolved.species) {
        return out;
    }
    // CanHatchAsEgg (species; form: Pichu forms cannot be bred) and presence in HG/SS.
    if !can_hatch(species) || (form != 0 && matches!(species, 25 | 133 | 172)) {
        return out;
    }
    if !ctx.evo.is_present(species, form) {
        return out;
    }
    out.push(species);
    // TryGetSplit
    let mut d = *devolved;
    if species == d.species {
        if chain.len() < 2 {
            return out;
        }
        d = chain[chain.len() - 2];
    }
    if split_breed4(d.species) {
        out.push(d.species);
    }
    out
}

/// LearnGroup4.CheckEncounterMoves for a bred egg: egg moves, inherited level-up moves, and
/// Volt Tackle on Pichu, for moves the regular sources did not explain.
fn apply_egg_moves(ctx: &Context4, pk: &Pkm, egg: u16, moves: &mut [MoveResult; 4]) {
    let learn = match pk.version() {
        7 | 8 => &ctx.learn.hgss,
        12 => &ctx.learn.pt,
        _ => &ctx.learn.dp,
    };
    let idx = learn.form_index(egg, 0);
    let current = pk.moves();
    for i in (0..4).rev() {
        if moves[i].valid() || !matches!(moves[i], MoveResult::Unobtainable | MoveResult::Unparsed)
        {
            continue;
        }
        let mv = current[i];
        if learn.egg_moves(idx).iter().any(|m| m == mv) {
            moves[i] = MoveResult::Initial;
        } else if learn.levelup(idx).any(|(_, m)| m == mv) {
            moves[i] = MoveResult::InheritLevelUp;
        } else if mv == VOLT_TACKLE && egg == PICHU {
            moves[i] = MoveResult::Initial;
        }
    }
}

/// Moves of a bred egg: `LearnVerifierRelearn.VerifyEggMoveset` while it is unhatched, the
/// regular sources plus inherited moves once hatched.
fn bred_moves(ctx: &Context4, pk: &Pkm, sp: u16, history: &[EvoCriteria]) -> [MoveResult; 4] {
    if pk.is_egg() {
        let current = pk.moves();
        let ok = super::breed4::verify_moves(&ctx.learn, sp, pk.version(), current);
        let mut out = ok.map(|m| match m {
            EggMove::Invalid => MoveResult::Unobtainable,
            EggMove::Empty => MoveResult::Empty,
            EggMove::Inherited => MoveResult::InheritLevelUp,
            EggMove::Other => MoveResult::Initial,
        });
        // LearnVerifierRelearn.IsAnyMoveDuplicate: the first slot repeating an earlier move.
        let dupe = (1..4).find(|&i| current[i] != 0 && current[..i].contains(&current[i]));
        if let Some(i) = dupe {
            out[i] = MoveResult::Duplicate;
        }
        super::moves4::finalize(&mut out, &current);
        return out;
    }
    let mut moves = ctx.learn.verify(pk.moves(), pk.form(), sp, history, None);
    apply_egg_moves(ctx, pk, sp, &mut moves);
    moves
}

/// `LearnVerifierEgg.VerifyMovesInitial`: exactly the initial moves, in order.
fn initial_moves(current: [u16; 4], fixed: [u16; 4], n: usize) -> [MoveResult; 4] {
    let mut out: [MoveResult; 4] = core::array::from_fn(|i| {
        if i < n {
            if current[i] != fixed[i] {
                MoveResult::Unobtainable
            } else {
                MoveResult::Initial
            }
        } else if current[i] == 0 {
            MoveResult::Empty
        } else {
            MoveResult::Unobtainable
        }
    });
    super::moves4::finalize(&mut out, &current);
    out
}

/// `LearnVerifierEgg.VerifyFromEncounter` for an encounter with no fixed moves: the base moves
/// of `species` at `level` in `version`'s learnset.
fn base_initial_moves(
    ctx: &Context4,
    current: [u16; 4],
    version: u8,
    species: u16,
    form: u8,
    level: u8,
) -> [MoveResult; 4] {
    let table = super::breed4::game(&ctx.learn, version);
    let idx = table.form_index(species, form);
    let (fixed, n) = super::breed4::base_egg_moves(table, idx, level);
    initial_moves(current, fixed, n)
}

/// Moves of an unhatched gift egg (`LearnVerifierEgg.VerifyFromEncounter`).
fn enc_egg_moves(ctx: &Context4, pk: &Pkm, e: &Enc4) -> [MoveResult; 4] {
    let current = pk.moves();
    if let Some(m) = enc_moves(e) {
        return initial_moves(current, m, m.iter().position(|&x| x == 0).unwrap_or(4));
    }
    let version = match e {
        Enc4::Data(d) => d.version(),
        Enc4::Gift(g) => g.pk().version(),
        // PGT.Version is Gen4 for the Ranger egg; GetLearnSource maps that to HG/SS.
        Enc4::Ranger => 7,
        Enc4::Ranch(_) => 10,
    };
    if version == 0 {
        // PKHeX has no learnset for version Any/Gen4 and leaves the moves unparsed.
        let mut out = [MoveResult::Unparsed; 4];
        super::moves4::finalize(&mut out, &current);
        return out;
    }
    base_initial_moves(ctx, current, version, species_of(e), 0, enc_level_min(e))
}

/// Moves when no encounter matched (`EncounterInvalid`).
fn invalid_moves(ctx: &Context4, inp: &Inputs) -> [MoveResult; 4] {
    let pk = &inp.pk;
    if pk.is_egg() {
        return base_initial_moves(
            ctx,
            pk.moves(),
            pk.version(),
            pk.species(),
            pk.form(),
            pk.met_level(),
        );
    }
    let history = ctx
        .evo
        .chain_for_encounter(&inp.evo, pk.species(), pk.met_level());
    ctx.learn
        .verify(pk.moves(), pk.form(), pk.species(), &history, None)
}

/// The bred-egg branch of the Gen 4 encounter search (`EncounterEnumerator4` Bred state and
/// `EncounterGenerator4` filters), then `EncounterVerifier.VerifyEncounterEgg4`.
///
/// Returns whether an egg was yielded directly and passed the moves, evolution and encounter
/// checks (`EncounterFinder` would stop there).
fn analyze_bred_egg(
    ctx: &Context4,
    inp: &Inputs,
    chain: &[EvoCriteria],
    report: &mut Report,
) -> bool {
    let pk = &inp.pk;
    let pidiv = analyze(&inp.analyze, |s| ctx.ratio(s));
    let list = egg_species(ctx, chain);
    // Generator filters: no ground tile, no PID-IV correlation, no Safari/Sport Ball.
    let mut chosen: Option<(u16, bool)> = None; // (species, deferred)
    let mut deferred: Option<(u16, u8)> = None;
    let mut defer = |sp: u16, rank: u8| {
        if deferred.is_none_or(|(_, r)| rank < r) {
            deferred = Some((sp, rank));
        }
    };
    let mut passing: Vec<u16> = Vec::new();
    for &sp in &list {
        if pk.ground_tile() != 0 {
            defer(sp, 4);
            continue;
        }
        if pidiv.kind != crate::rng::finder::PidType::None {
            defer(sp, 5);
            continue;
        }
        if matches!(pk.ball(), 5 | 24) {
            defer(sp, 3);
            continue;
        }
        passing.push(sp);
    }
    // EncounterFinder: the first candidate whose moves and evolution check out, else the last.
    let n = passing.len();
    for (i, &sp) in passing.iter().enumerate() {
        let history = ctx.evo.chain_for_encounter(&inp.evo, sp, 1);
        let moves = bred_moves(ctx, pk, sp, &history);
        let ev = evolution_valid(ctx, inp, sp, &history, &moves);
        let last = i + 1 == n;
        if (!moves.iter().all(|m| m.valid()) || !ev) && !last {
            continue;
        }
        report.moves = moves;
        if !ev {
            report.flag(CheckId::Evolution);
        }
        chosen = Some((sp, false));
        break;
    }
    if chosen.is_none() {
        if let Some((sp, rank)) = deferred {
            let history = ctx.evo.chain_for_encounter(&inp.evo, sp, 1);
            report.moves = bred_moves(ctx, pk, sp, &history);
            if !evolution_valid(ctx, inp, sp, &history, &report.moves) {
                report.flag(CheckId::Evolution);
            }
            if rank == 5 {
                report.flag(CheckId::PID);
            }
            chosen = Some((sp, true));
        }
    }
    let Some((species, deferred_choice)) = chosen else {
        report.flag(CheckId::Encounter);
        report.moves = invalid_moves(ctx, inp);
        verify_common(ctx, inp, None, report);
        return false;
    };
    let (species, deferred) = (species, deferred_choice);
    report.matched = Matched::Egg { species };
    // PIDVerifier.VerifyEggGender8000: Nidoran and Volbeat/Illumise take gender from PID bit 15;
    // a male that broke the rule needs a Ditto parent, which rules out inherited moves.
    if matches!(species, 29 | 32 | 313 | 314) {
        let bit = ((pk.pid() >> 15) & 1) as u8;
        let inherited = report.moves.contains(&MoveResult::InheritLevelUp);
        if pk.gender() == bit && (pk.gender() == 1 || inherited) {
            report.flag(CheckId::EC);
        }
    }
    let before = report.invalid.len();
    if pk.is_egg() {
        verify_unhatched4(pk, report);
    } else {
        verify_hatched4(pk, report);
    }
    let passed = !deferred
        && report.invalid.len() == before
        && report.moves.iter().all(|m| m.valid())
        && !report.invalid.contains(&CheckId::Evolution);
    verify_common_egg(ctx, inp, species, report);
    passed
}

/// A static encounter's fixed moveset (PKHeX `IMoveset`), if it has one.
fn static_moves(e: &Encounter) -> Option<[u16; 4]> {
    if e.is_slot() || !e.has(crate::data::encounters::flag::HAS_MOVES) {
        return None;
    }
    e.template().map(|t| t.moves())
}

fn verify_common(ctx: &Context4, inp: &Inputs, enc: Option<&Enc4>, report: &mut Report) {
    verify_common_inner(ctx, inp, enc, None, report);
}

fn verify_common_egg(ctx: &Context4, inp: &Inputs, egg: u16, report: &mut Report) {
    verify_common_inner(ctx, inp, None, Some(egg), report);
}

fn verify_common_inner(
    ctx: &Context4,
    inp: &Inputs,
    enc: Option<&Enc4>,
    egg: Option<u16>,
    report: &mut Report,
) {
    let pk = &inp.pk;
    let enc_species = egg.unwrap_or_else(|| enc.map_or(pk.species(), species_of));
    // Blob templates (slots, statics, trades) vs Mystery Gifts (template PK4, Ranger Manaphy).
    let data = enc.and_then(|e| e.data());
    let gift = match enc {
        Some(Enc4::Gift(g)) => Some((*g, g.pk())),
        _ => None,
    };
    let ranger = matches!(enc, Some(Enc4::Ranger));
    let ranch = match enc {
        Some(Enc4::Ranch(g)) => Some(*g),
        _ => None,
    };
    let event = gift.is_some() || ranger;
    let pi = ctx.hgss.species_form(pk.species(), pk.form());
    let growth = pi.growth();

    // PIDVerifier: Wurmple evolution from EC.
    if enc_species == WURMPLE && pk.species() != WURMPLE {
        let val = ((pk.pid() >> 16) % 10 / 5) as i32;
        let idx = pk.species() as i32 - 266;
        let group = if (idx & 3) != idx { -1 } else { idx >> 1 };
        if val != group {
            report.flag(CheckId::EC);
        }
    }

    // LevelVerifier (Mystery Gifts: met level must be the gift's; Ranger Manaphy may hatch
    // with met level 0)
    let lvl = inp.level;
    let mut level_done = false;
    if let Some(e) = enc.filter(|_| event) {
        let glevel = enc_level_min(e);
        if !(glevel == pk.met_level() || (ranger && pk.met_level() == 0)) || glevel > lvl {
            report.flag(CheckId::Level);
            level_done = true;
        }
    }
    if level_done {
    } else if pk.is_egg() {
        let lmin = match (egg, enc) {
            (Some(_), _) => 1,
            (None, Some(e)) => enc_level_min(e),
            (None, None) => pk.met_level(),
        };
        if lvl != lmin || pk.exp() != crate::pkm::derived::exp_for_level(lmin, growth) {
            report.flag(CheckId::Level);
        }
    } else {
        if lvl >= 100 && pk.exp() != crate::pkm::derived::exp_for_level(100, growth) {
            report.flag(CheckId::Level);
        }
        if lvl < pk.met_level() {
            report.flag(CheckId::Level);
        }
    }

    // EffortValueVerifier (format 4)
    let evs = pk.evs();
    if pk.is_egg() {
        if evs.iter().any(|&e| e != 0) {
            report.flag(CheckId::EVs);
        }
    } else {
        let sum: u32 = evs.iter().map(|&e| e as u32).sum();
        if sum > 510 {
            report.flag(CheckId::EVs);
        }
        if evs.iter().any(|&e| e > 100) {
            // EncounterInvalid uses the current species and met level.
            let (sp, lmin) = match (egg, enc) {
                (Some(sp), _) => (sp, 1),
                (None, Some(e)) => (species_of(e), enc_level_min(e)),
                (None, None) => (pk.species(), pk.met_level()),
            };
            if lmin == 100 {
                report.flag(CheckId::EVs);
            } else {
                let g = ctx.hgss.personal(sp as usize).growth();
                if crate::pkm::derived::exp_for_level(lmin, g) == pk.exp() {
                    report.flag(CheckId::EVs);
                }
            }
        }
    }

    // AbilityVerifier (Gen 4 origin, format 4: must match PID bit 0)
    let species_now = pk.species();
    let [a1, a2, _] = pi.abilities();
    let ab = pk.ability();
    if ab != a1 && ab != a2 {
        report.flag(CheckId::Ability);
    } else if ranger {
        // VerifyAbilityMG: PGT (Ranger Manaphy) accepts either ability slot in Gen 4.
    } else if let Some((g, t)) = gift.as_ref().filter(|(g, _)| g.species() == pk.species()) {
        // VerifyAbilityPCD: unevolved, the ability must be the gift's.
        let _ = g;
        if ab != t.ability() {
            report.flag(CheckId::Ability);
        }
    } else {
        // AbilityVerifier.VerifyFixedAbility, for an unevolved encounter with one fixed slot:
        // either the ability or the PID bit may be the one that matches.
        let fixed = ranch
            .map(|g| g.ability)
            .or(data.filter(|e| !e.is_slot()).map(|e| e.ability()))
            .filter(|a| matches!(a, 1 | 2))
            .filter(|_| enc_species == species_now);
        let ok = match fixed {
            Some(a) => {
                let index = a - 1;
                (if ab == a1 { 0 } else { 1 }) == index || (pk.pid() & 1) as u8 == index
            }
            None => ab == if pk.pid() & 1 == 0 { a1 } else { a2 },
        };
        if !ok {
            report.flag(CheckId::Ability);
        }
    }

    // MiscVerifier: Gen 4 fields, fateful flag, Pokérus duration, calendar dates.
    let leaf = pk.shiny_leaf();
    if (leaf > 0b1_1111 && leaf != 0b11_1111) || (leaf != 0 && pk.is_egg()) {
        report.flag(CheckId::Misc);
    }
    // Fateful flag must match the template (event statics set it; nothing else does).
    let fateful_expected = match (&gift, ranger) {
        (Some((_, t)), _) => t.fateful(),
        (None, true) => true,
        _ => {
            ranch.is_some_and(|g| g.fateful)
                || data.is_some_and(|e| e.has(crate::data::encounters::flag::FATEFUL))
        }
    };
    if let Some((_, t)) = &gift {
        // VerifyReceivability: version-locked cards with no stored version.
        if t.version() != pk.version() && t.version() == 0 {
            report.flag(CheckId::GameOrigin);
        }
    }
    if pk.fateful() != fateful_expected {
        report.flag(CheckId::Fateful);
    }
    if let Some((g, t)) = &gift {
        // PIDVerifier: Shiny rule, then the fixed PID.
        match gift4::shiny_rule(g, t) {
            1 if pk.is_shiny() => report.flag(CheckId::Shiny),
            2 if !pk.is_shiny() => report.flag(CheckId::Shiny),
            _ => {}
        }
        if t.pid() != 1 && pk.pid() != t.pid() {
            report.flag(CheckId::Shiny);
        }
        // IndividualValueVerifier.VerifyIVsMystery: fixed IVs must match.
        let fixed = t.iv32() & 0x3FFF_FFFF;
        if fixed != 0 && pk.iv32() & 0x3FFF_FFFF != fixed {
            report.flag(CheckId::IVs);
        }
    }
    if let Some(e) = data.filter(|e| !e.is_slot()) {
        if let Some(t) = e.template() {
            // Fixed nature.
            if t.nature() != 25 && pk.nature() != t.nature() {
                report.flag(CheckId::Nature);
            }
        }
        // Shiny.Never.
        if e.shiny() == 1 && pk.is_shiny() {
            report.flag(CheckId::Shiny);
        }
    }
    if let Some(g) = ranch {
        // BallVerifier: fixed ball.
        if pk.ball() != g.ball {
            report.flag(CheckId::Ball);
        }
    }
    let pkrs = pk.pokerus();
    if (pkrs & 0xF) as u32 > ((pkrs >> 4) & 3) as u32 + 1 {
        report.flag(CheckId::Misc);
    }
    let [my, mm, md] = pk.met_date();
    if pk.met_location() != 0 {
        if !valid_date(my, mm, md) {
            report.flag(CheckId::Memory);
        }
    } else if (enc.is_some() || egg.is_some()) && (my, mm, md) != (0, 0, 0) {
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

    // FormVerifier (Gen 4 format)
    let species = pk.species();
    let form = pk.form();
    let fc = pi.form_count();
    if !(fc <= 1 && form == 0) {
        let in_range = form < fc;
        let oob_ok = match species {
            201 => form < 28,
            414 => form < 3,
            _ => false,
        };
        if !in_range && !oob_ok {
            report.flag(CheckId::Form);
        } else {
            match species {
                487 if (form == 1) != (pk.held_item() == 112) => report.flag(CheckId::Form),
                493 => {
                    const PLATES: [u16; 17] = [
                        303, 306, 304, 305, 309, 308, 310, 313, 298, 299, 301, 300, 307, 302, 311,
                        312, 644,
                    ];
                    let it = pk.held_item();
                    let expect = if (298..=313).contains(&it) || it == 644 {
                        let f = PLATES.iter().position(|&p| p == it).map_or(0, |i| i + 1) as u8;
                        if f < 9 {
                            f
                        } else {
                            f + 1
                        }
                    } else {
                        0
                    };
                    if expect != form {
                        report.flag(CheckId::Form);
                    }
                }
                479 if pk.is_egg() && form != 0 => report.flag(CheckId::Egg),
                // FormInfo.IsBattleOnlyForm: Castform and Cherrim forms only exist in battle.
                351 | 421 if form != 0 => report.flag(CheckId::Form),
                _ => {}
            }
        }
    }

    // BallVerifier: Gen 4 eggs hatch in a Poké Ball.
    if egg.is_some() && pk.ball() != 4 {
        report.flag(CheckId::Ball);
    }
    // BallVerifier: gifts have a fixed ball (Ranger Manaphy: Poké Ball).
    if let Some((_, t)) = &gift {
        if pk.ball() != t.ball() {
            report.flag(CheckId::Ball);
        }
    }
    if ranger && pk.ball() != 4 {
        report.flag(CheckId::Ball);
    }
    // BallVerifier (wild slot / static)
    if let Some(e) = data {
        let ball = pk.ball();
        let ok = if e.species() == 290 && pk.species() == 292 {
            ball == 4 // Nincada -> Shedinja outside the Bug-Catching Contest: Poké Ball only
        } else if e.ball() != 0 {
            ball == e.ball()
        } else {
            // BallUseLegality.WildPokeBalls4_DPPt
            const DPPT: u64 = (1 << 1)
                | (1 << 2)
                | (1 << 3)
                | (1 << 4)
                | (0x7F << 6)
                | (1 << 13)
                | (1 << 14)
                | (1 << 15);
            // BallUseLegality.WildPokeBalls4_HGSS adds Kurt's Apricorn Balls (Fast..Moon).
            const KURT: u64 = 0x7F << 17;
            let allowed = if matches!(pk.version(), 7 | 8) {
                DPPT | KURT
            } else {
                DPPT
            };
            ball < 64 && allowed & (1u64 << ball) != 0
        };
        if !ok {
            report.flag(CheckId::Ball);
        }
    }

    // GroundTileVerifier (reported as Encounter)
    let tile = pk.ground_tile();
    let tile_ok = match data {
        Some(e) if e.ground_tile() != 0 => tile < 32 && e.ground_tile() & (1u32 << tile) != 0,
        _ => tile == 0,
    };
    if !tile_ok {
        report.flag(CheckId::Encounter);
    }

    // ItemVerifier
    let item = pk.held_item();
    if pk.is_egg() && item != 0 {
        report.flag(CheckId::HeldItem);
    }
    if item != 0
        && !ctx
            .learn
            .moves
            .list(crate::data::move_table::RELEASED_HELD)
            .contains(item)
    {
        report.flag(CheckId::HeldItem);
    }

    if pk.is_egg() {
        // MiscVerifier.VerifyMiscEggCommon; the encounter's hatch cycles are the species'.
        if pk.friendship() > pi.hatch_cycles() {
            report.flag(CheckId::Egg);
        }
        if pk.contest().iter().any(|&c| c != 0) {
            report.flag(CheckId::Egg);
        }
        if pk.decrypted()[0x87] != 0 {
            report.flag(CheckId::Egg);
        }
        // MovePPVerifier.VerifyEgg.
        let (moves, ups, pp) = (pk.moves(), pk.pp_ups(), pk.pp());
        if ups.iter().any(|&u| u != 0) {
            report.flag(CheckId::Egg);
        }
        if (0..4).any(|i| pp[i] != ctx.learn.moves.max_pp(moves[i], 0)) {
            report.flag(CheckId::Egg);
        }
    }

    // MovePPVerifier (not egg): PP Ups allowed except on empty/Sketch; PP within max.
    if !pk.is_egg() {
        let (moves, ups, pp) = (pk.moves(), pk.pp_ups(), pk.pp());
        for i in 0..4 {
            if ups[i] != 0 && matches!(moves[i], 0 | 166) {
                report.flag(CheckId::CurrentMove);
            }
            if pp[i] > ctx.learn.moves.max_pp(moves[i], ups[i]) {
                report.flag(CheckId::CurrentMove);
            }
        }
    }

    // LanguageVerifier (Gen 4 origin: 1..=8, not 6). The Korean/non-Korean save check needs an
    // active trainer, as in PKHeX.
    let lang = pk.language();
    if lang == 0 || lang == 6 || lang > 8 {
        report.flag(CheckId::Language);
    }

    let trade = data.filter(|e| super::enc4::is_trade(e));
    // TrainerNameVerifier (player OT; skipped for fixed-trainer trades, checked below)
    let mut ot = [0u16; 8];
    let on = crate::text::decode4(&pk.ot_name_raw(), &mut ot);
    let ot = &ot[..on];
    // IsPlayerOriginalTrainer: not for fixed-trainer trades or any gift (MysteryGift has a
    // trainer ID of its own, so `ITrainerID16ReadOnly` excludes gift eggs too).
    let gift_ot = event;
    if trade.is_none() && !gift_ot && ranch.is_none() {
        let too_many_digits = on > 4 && ot.iter().filter(|&&c| is_number(c)).count() > 4;
        if on == 0 || ot.contains(&0xFFFF) || too_many_digits {
            report.flag(CheckId::Trainer);
        }
        // Legal.GetMaxLengthOT (Gen 4: 6 for Japanese/Korean, else 12), with
        // TrainerNameVerifier.IsEdgeCaseLength for traded eggs (English maximum).
        let max = if matches!(pk.language(), 1 | 8) {
            6
        } else {
            12
        };
        if on > max {
            let enc_egg = egg.is_some()
                || ranger
                || gift.as_ref().is_some_and(|(g, _)| g.is_egg())
                || data
                    .and_then(|e| e.template())
                    .is_some_and(|t| t.egg_location() != 0);
            let traded = if pk.is_egg() {
                pk.met_location() == LINK_TRADE4
            } else {
                pk.egg_location() == LINK_TRADE4
            };
            if !(enc_egg && traded && on <= 12) {
                report.flag(CheckId::Trainer);
            }
        }
    }

    // NicknameVerifier: a gift that came un-nicknamed cannot be nicknamed.
    if let Some((g, t)) = &gift {
        if !g.is_egg() && pk.is_nicknamed() && !t.is_nicknamed() {
            report.flag(CheckId::Nickname);
        }
        // (TrashByteVerifier.VerifyTrashBytesPCD only runs for format 7+.)
    }

    // RibbonVerifier (event ribbons only as the gift had them).
    let raw = pk.decrypted();
    let ribbon_args = super::ribbons4::RibbonArgs {
        sinnoh: [raw[0x24], raw[0x25], raw[0x26], raw[0x27]],
        hoenn: [raw[0x3C], raw[0x3D], raw[0x3E], raw[0x3F]],
        contest4: [raw[0x60], raw[0x61], raw[0x62]],
        is_egg: pk.is_egg(),
        event: gift.as_ref().map_or([0; 3], |(_, t)| t.event_ribbons4()),
        gen4: !ctx.evo.encounter_origin_chain(&inp.evo).is_empty(),
        species: pk.species(),
        form: pk.form(),
        spiky_static: data.is_some_and(|e| !e.is_slot() && e.species() == PICHU && e.form() == 1),
    };
    if super::ribbons4::ribbons_invalid(&ribbon_args) {
        report.flag(CheckId::Ribbon);
    }

    // NicknameVerifier (Gen 4 format)
    if let Some(t) = trade {
        verify_trade4(ctx, pk, t, ot, report);
    } else {
        if ranch.is_some() && !ranch4::trainer_matches(pk.language(), ot) {
            report.flag(CheckId::Trainer);
        }
        verify_nickname(ctx, pk, enc_species, report);
    }

    // GenderVerifier
    let ratio = pi.gender_ratio();
    let gender = pk.gender();
    if (ratio == 255) != (gender == 2) {
        let shedinja_glitch = pk.species() == 292 && pk.met_level() != lvl;
        if !shedinja_glitch {
            report.flag(CheckId::Gender);
        }
    } else {
        let pid = pk.pid();
        let from_pid = crate::pkm::derived::gender_from_pid(pid, ratio);
        let gender_valid = match ratio {
            255 => gender == 2,
            254 => gender == 1,
            0 => gender == 0,
            _ => gender < 2 && gender == from_pid,
        };
        let ok = if !gender_valid {
            pk.species() == 292 && gender == crate::pkm::derived::gender_from_pid(pid, 127)
        } else if matches!(pk.species(), 292 | 413 | 414 | 416 | 475 | 478) {
            gender == 2
                || crate::pkm::derived::gender_from_pid(pid, ctx.ratio(enc_species)) == gender
        } else {
            true
        };
        if !ok {
            report.flag(CheckId::Gender);
        }
    }
}

/// PKHeX `DateUtil.IsValidDate(2000 + year, month, day)`.
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

/// .NET `char.IsNumber` (Unicode Nd/Nl/No) for the characters the Gen 4 charset can hold,
/// including the enclosed alphanumerics block (circled, parenthesised and full-stop digits).
fn is_number(c: u16) -> bool {
    matches!(
        c,
        0x30..=0x39
            | 0xFF10..=0xFF19
            | 0xB2
            | 0xB3
            | 0xB9
            | 0xBC..=0xBE
            | 0x2460..=0x249B
            | 0x24EA..=0x24FF
    )
}

fn name_matches(ctx: &Context4, lang: u8, species: u16, nick: &[u16]) -> bool {
    let name = ctx.names4.get(lang as u16, species as usize);
    !name.is_empty() && name.iter().eq(nick.iter().copied())
}

/// PKHeX `NicknameVerifier.VerifyTrade4`: fixed OT name and nickname in the trade's original
/// language (`EncounterTrade4PID.DetectOriginalLanguage`, `IsIncorrectEnglish`).
fn verify_trade4(ctx: &Context4, pk: &Pkm, e: &Encounter, ot: &[u16], report: &mut Report) {
    let ed = if matches!(pk.version(), 7 | 8) {
        &ctx.enc_hgss
    } else {
        &ctx.enc_dppt
    };
    let mut nbuf = [0u16; 11];
    let nn = crate::text::decode4(&pk.nickname_raw(), &mut nbuf);
    let nick = &nbuf[..nn];
    let names = |lang: u8| ed.trade_names(e, lang);
    let eq = |a: &[u16], b: crate::data::blob::U16s| {
        a.len() == b.len() && a.iter().zip(b.iter()).all(|(x, y)| *x == y)
    };
    let dp = matches!(pk.version(), 10 | 11);
    let lang = pk.language();
    if lang == 2 && dp {
        report.flag(CheckId::Language);
    }
    // DetectTradeLanguage: the first language whose OT name matches, else the current one.
    let detect = |cur: u8| -> u8 {
        (1u8..=8)
            .find(|&l| names(l).is_some_and(|(_, o)| !o.is_empty() && eq(ot, o)))
            .unwrap_or(cur)
    };
    let orig = match e.species() {
        25 => {
            if lang == 3 {
                2
            } else {
                let l = detect(lang);
                if l == 2 {
                    if names(4).is_some_and(|(n, _)| eq(nick, n)) {
                        4
                    } else {
                        7
                    }
                } else {
                    l
                }
            }
        }
        129 => {
            if lang == 2 {
                5
            } else {
                let l = detect(lang);
                if l == 2 {
                    if names(3).is_some_and(|(n, _)| eq(nick, n)) {
                        3
                    } else {
                        7
                    }
                } else {
                    l
                }
            }
        }
        _ if lang == 1 && dp => {
            if names(1).is_some_and(|(_, o)| eq(ot, o)) {
                1
            } else {
                2
            }
        }
        _ => lang,
    };
    match names(orig) {
        Some((n, o)) => {
            if !eq(ot, o) {
                report.flag(CheckId::Trainer);
            }
            if !eq(nick, n) {
                report.flag(CheckId::Nickname);
            }
        }
        None => {
            report.flag(CheckId::Trainer);
            report.flag(CheckId::Nickname);
        }
    }
}

fn verify_nickname(ctx: &Context4, pk: &Pkm, enc_species: u16, report: &mut Report) {
    let mut buf = [0u16; 11];
    let n = crate::text::decode4(&pk.nickname_raw(), &mut buf);
    let nick = &buf[..n];
    if n == 0 || nick.contains(&0xFFFF) {
        report.flag(CheckId::Nickname);
        return;
    }
    if pk.is_egg() {
        // NicknameVerifier.VerifyNicknameEgg (Gen 4: flag clear, title-case egg name).
        if pk.is_nicknamed() {
            report.flag(CheckId::Egg);
        }
        let french: [u16; 4] = [0x4F, 0x65, 0x75, 0x66];
        let matches = if pk.language() == 3 {
            nick == french
        } else {
            name_matches(ctx, pk.language(), 0, nick)
        };
        if !matches {
            report.flag(CheckId::Egg);
        }
        return;
    }
    let lang = pk.language();
    let species = pk.species();
    if pk.is_nicknamed() {
        if nick.iter().any(|&c| (0x4E00..=0x9FFF).contains(&c)) {
            report.flag(CheckId::Nickname);
            return;
        }
        let max = if matches!(lang, 1 | 8) { 5 } else { 10 };
        if n > max {
            report.flag(CheckId::Nickname);
            return;
        }
        // Word filter not applied (docs/discrepancies.md); digit count is.
        if n > 4 && nick.iter().filter(|&&c| is_number(c)).count() > 4 {
            report.flag(CheckId::Nickname);
        }
        return;
    }
    if name_matches(ctx, lang, species, nick) {
        return;
    }
    let any = [1u8, 2, 3, 4, 5, 7, 8]
        .iter()
        .any(|&l| name_matches(ctx, l, species, nick));
    if !(enc_species != species && any) {
        report.flag(CheckId::Nickname);
    }
}
