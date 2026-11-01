//! `LegalityAnalysis` for PK4 in the Gen 4 format, D/P/Pt wild-slot scope (M2). Ported from
//! PKHeX `LegalityAnalysis.cs`, `Encounters/Generator/EncounterFinder.cs`,
//! `Encounters/Verifiers/EncounterVerifier.cs`, `Verifiers/EvolutionVerifier.cs`,
//! `Restrictions/EvolutionRestrictions.cs` and the verifiers named on each check below.

use alloc::vec::Vec;

use super::enc4::{wild_slots, Deferral, MatchPkm};
use super::evo::{Evo4, EvoCriteria, EvoPkm};
use super::moves4::{Learn4, MoveResult};
use crate::data::encounters::{Encounter, EncounterData};
use crate::data::GameData;
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
    /// Gender ratio for species > 493 (for PKHeX `EntityGender` lookups).
    pub gen5: GameData<'a>,
    /// Gen 4-formatted species names by language ID (`names4.bin`).
    pub names4: crate::data::Strings<'a>,
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
        },
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

/// Analyses a PK4 whose origin is D/P/Pt (version 10..=12) in the Gen 4 format.
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
    let pidiv = analyze(&inp.analyze, ratio);
    let group: &[u8] = match pk.version() {
        10 => b"SlotsD",
        11 => b"SlotsP",
        _ => b"SlotsPt",
    };
    let m = if chain.is_empty() {
        None
    } else {
        Some(wild_slots(
            &inp.matcher,
            &chain,
            pidiv,
            &ctx.enc_dppt,
            group,
            ratio,
        ))
    };
    let mut chosen: Option<(Encounter, LeadSeed, Option<Deferral>, Vec<EvoCriteria>)> = None;
    let mut evo_ok = true;
    if let Some(m) = &m {
        let mut list: Vec<(Encounter, LeadSeed, Option<Deferral>)> = m
            .candidates
            .iter()
            .map(|c| (c.encounter, c.lead, None))
            .collect();
        if let Some((e, d)) = m.fallback {
            list.push((e, LeadSeed::default(), Some(d)));
        }
        let n = list.len();
        for (i, (e, lead, d)) in list.into_iter().enumerate() {
            let last = i + 1 == n;
            let history = ctx
                .evo
                .chain_for_encounter(&inp.evo, e.species(), e.level_min());
            let moves = ctx
                .learn
                .verify(pk.moves(), pk.form(), e.species(), &history, None);
            if !moves.iter().all(|r| r.valid()) && !last {
                continue;
            }
            let ev = evolution_valid(ctx, &inp, e.species(), &history, &moves);
            if !ev && !last {
                continue;
            }
            evo_ok = ev;
            report.moves = moves;
            chosen = Some((e, lead, d, history));
            break;
        }
    }

    let Some((enc, lead, deferral, history)) = chosen else {
        report.flag(CheckId::Encounter);
        let history = ctx
            .evo
            .chain_for_encounter(&inp.evo, pk.species(), pk.met_level());
        report.moves = ctx.learn.verify(pk.moves(), pk.form(), 0, &history, None);
        verify_common(ctx, &inp, None, &mut report);
        return report;
    };
    report.matched = Matched::Slot {
        encounter: enc,
        lead,
    };
    if !evo_ok {
        report.flag(CheckId::Evolution);
    }
    match deferral {
        Some(Deferral::SlotNumber) => report.flag(CheckId::PID),
        Some(Deferral::PidIv) => report.flag(CheckId::PID),
        _ => {}
    }
    let _ = history;
    verify_common(ctx, &inp, Some(&enc), &mut report);
    report
}

const WURMPLE: u16 = 265;

fn verify_common(ctx: &Context4, inp: &Inputs, enc: Option<&Encounter>, report: &mut Report) {
    let pk = &inp.pk;
    let enc_species = enc.map_or(pk.species(), |e| e.species());
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

    // LevelVerifier
    let lvl = inp.level;
    if !pk.is_egg() {
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
            if let Some(e) = enc {
                if e.level_min() == 100 {
                    report.flag(CheckId::EVs);
                } else {
                    let g = ctx.hgss.personal(e.species() as usize).growth();
                    if crate::pkm::derived::exp_for_level(e.level_min(), g) == pk.exp() {
                        report.flag(CheckId::EVs);
                    }
                }
            }
        }
    }

    // AbilityVerifier (Gen 4 origin, format 4: must match PID bit 0)
    let [a1, a2, _] = pi.abilities();
    let ab = pk.ability();
    if ab != a1 && ab != a2 {
        report.flag(CheckId::Ability);
    } else {
        let expect = if pk.pid() & 1 == 0 { a1 } else { a2 };
        if expect != ab {
            report.flag(CheckId::Ability);
        }
    }

    // MiscVerifier: Gen 4 fields, fateful flag, Pokérus duration, calendar dates.
    let leaf = pk.shiny_leaf();
    if (leaf > 0b1_1111 && leaf != 0b11_1111) || (leaf != 0 && pk.is_egg()) {
        report.flag(CheckId::Misc);
    }
    if pk.fateful() {
        report.flag(CheckId::Fateful);
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
                _ => {}
            }
        }
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

    // NicknameVerifier (wild slot, Gen 4 format)
    verify_nickname(ctx, pk, enc_species, report);

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

fn is_number(c: u16) -> bool {
    matches!(c, 0x30..=0x39 | 0xFF10..=0xFF19 | 0xB2 | 0xB3 | 0xB9 | 0xBC..=0xBE | 0x2460..=0x2473)
}

fn name_matches(ctx: &Context4, lang: u8, species: u16, nick: &[u16]) -> bool {
    let name = ctx.names4.get(lang as u16, species as usize);
    !name.is_empty() && name.iter().eq(nick.iter().copied())
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
