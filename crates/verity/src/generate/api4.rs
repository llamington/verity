//! Request API for D/P/Pt wild encounters: `GenRequest` → generate (Method J) → build the
//! PK4 → the legality gate ([`analyze4`] must be valid) → insert into a save slot with the
//! capture side effects (party stats, Pokédex).
//!
//! Nothing illegal leaves [`generate`]: every result has passed [`analyze4`], which matches
//! PKHeX on the fuzz and corpus suites. An impossible request returns a specific [`Error`].

use alloc::vec::Vec;

use super::build4::{BuildData, Trainer4, build_wild4};
use super::wild4::{self, GenError, IvProposal, Lead, Rand, Request};
use crate::data::encounters::{Encounter, EncounterKind};
use crate::legality::analysis4::{CheckId, Context4, Matched, analyze4};
use crate::pkm::Pkm;
use crate::rng::method_j::Slot4;
use crate::save::{Save, SaveError, SaveKind, Slot};

pub const VERSION_D: u8 = 10;
pub const VERSION_P: u8 = 11;
pub const VERSION_PT: u8 = 12;
pub const VERSION_HG: u8 = 7;
pub const VERSION_SS: u8 = 8;

/// Which encounter to generate from.
#[derive(Clone, Copy)]
pub enum EncounterChoice<'a> {
    /// A specific slot, e.g. picked from [`encounters`].
    Slot(Encounter<'a>),
    /// Any wild slot of the request's game with the species and form.
    Any,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum IvRequest {
    /// Whatever the frame gives.
    Random,
    /// Exactly these IVs (HP, Atk, Def, Spe, SpA, SpD), or an error.
    Exact([u8; 6]),
    /// The closest legal spread to these IVs. The result reports the change.
    Nearest([u8; 6]),
}

#[derive(Clone, Copy)]
pub struct GenRequest<'a> {
    /// Origin game: [`VERSION_D`], [`VERSION_P`] or [`VERSION_PT`].
    pub version: u8,
    pub species: u16,
    /// Only Unown's form is selectable; other species take the slot's form.
    pub form: u8,
    pub encounter: EncounterChoice<'a>,
    /// Met level; `None` rolls within the slot's range.
    pub level: Option<u8>,
    pub nature: Option<u8>,
    pub gender: Option<u8>,
    /// Ability ID; it must be one of the species' two abilities.
    pub ability: Option<u8>,
    pub shiny: bool,
    pub lead: Lead,
    pub hidden_power_type: Option<u8>,
    pub ivs: IvRequest,
    /// Ball; `None` uses the slot's required ball or the Poké Ball.
    pub ball: Option<u8>,
    /// Moves; `None` gives the encounter's natural moves.
    pub moves: Option<[u16; 4]>,
    /// Met date: years since 2000, month, day.
    pub met_date: [u8; 3],
    /// Seed for candidate choice (results are deterministic per seed).
    pub seed: u64,
    /// Candidate budget per encounter tried.
    pub budget: u32,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Error {
    /// The generator's specific reason (nature vs Synchronize, shiny vs Cute Charm, ...).
    Gen(GenError),
    VersionUnsupported,
    /// The chosen slot is for a different species, form or game.
    NotInEncounter,
    /// No wild slot in this game has the species (and form, and level).
    NoEncounter,
    AbilityUnavailable,
    BallNotAllowed,
    /// The requested moves are not legal for the encounter.
    MovesIllegal,
    /// The built Pokémon failed the legality gate on this check. Indicates a generator bug; the
    /// Pokémon is never returned.
    Illegal(CheckId),
    Save(SaveError),
    /// Party slots must stay contiguous: insert at most at the current party count.
    PartyGap,
    /// Inserting into a party needs a party-format Pokémon (an internal error).
    WrongFormat,
    /// The game-safe validator rejected the write.
    Unsafe(crate::safety::WriteError),
    /// The operation needs a Pokémon that matched a Gen 4 wild slot.
    NotWild,
}

impl From<GenError> for Error {
    fn from(e: GenError) -> Self {
        Error::Gen(e)
    }
}

impl From<SaveError> for Error {
    fn from(e: SaveError) -> Self {
        Error::Save(e)
    }
}

pub struct Generated4 {
    /// Stored (box) format.
    pub pkm: Pkm,
    pub frame: wild4::Generated,
    /// Set for [`IvRequest::Nearest`]: the spread used and its change from the request.
    pub iv_proposal: Option<IvProposal>,
}

/// A replacement PID and IV spread from [`closest_ivs_for`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct IvFix {
    pub pid: u32,
    /// HP, Atk, Def, Spe, SpA, SpD.
    pub ivs: [u8; 6],
    /// Total IV points away from the request.
    pub distance: u32,
}

/// The legal IV spread closest to `target` for a Pokémon already caught in a Gen 4 wild slot,
/// keeping its encounter, level, nature, gender, ability slot and shininess. In Gen 4 the IVs and
/// PID come from the same RNG, so the PID changes with them. `budget` caps spreads tested per lead.
pub fn closest_ivs_for(
    ctx: &Context4,
    pk: &Pkm,
    target: [u8; 6],
    budget: u32,
) -> Result<IvFix, Error> {
    let report = analyze4(ctx, pk);
    let Matched::Slot { encounter: e, .. } = report.matched else {
        return Err(Error::NotWild);
    };
    if !matches!(e.kind(), Some(EncounterKind::Slot4)) {
        return Err(Error::NotWild);
    }
    let Some(slot) = Slot4::from_encounter(&e) else {
        return Err(Error::NotWild);
    };
    let ratio = |s: u16| ctx.ratio(s);
    let pid = pk.pid();
    let enc_ratio = ratio(e.species());
    let shiny = pk.is_shiny();
    let mut request = wild4::Request {
        slot,
        tid: pk.tid(),
        sid: pk.sid(),
        species: e.species(),
        gender_ratio: enc_ratio,
        met_level: Some(pk.met_level()),
        nature: Some((pid % 25) as u8),
        gender: Some(crate::pkm::derived::gender_from_pid(pid, enc_ratio)),
        ability_bit: Some((pid & 1) as u8),
        shiny,
        lead: Lead::None,
        hidden_power_type: None,
        location: e.location(),
        form: pk.form(),
        version: pk.version(),
    };
    // A Synchronize lead reaches spreads no-lead frames can't, at the same nature.
    let mut best: Option<wild4::IvProposal> = None;
    let mut last = GenError::BudgetExhausted;
    for lead in [Lead::None, Lead::Synchronize((pid % 25) as u8)] {
        request.lead = lead;
        match wild4::closest_legal_ivs(&request, target, budget, ratio) {
            Ok(p) if best.as_ref().is_none_or(|b| p.distance < b.distance) => best = Some(p),
            Ok(_) => {}
            Err(err) => last = err,
        }
    }
    let Some(p) = best else {
        return Err(Error::Gen(last));
    };
    let fix = IvFix {
        pid: p.generated.pid,
        ivs: p.ivs,
        distance: p.distance,
    };
    // Never hand back a spread that would break the encounter match.
    let mut check = pk.clone();
    check.set_pid(fix.pid);
    check.set_ivs(fix.ivs);
    check.refresh_checksum();
    let after = analyze4(ctx, &check);
    for id in [CheckId::PID, CheckId::Encounter] {
        if after.invalid.contains(&id) && !report.invalid.contains(&id) {
            return Err(Error::Illegal(id));
        }
    }
    Ok(fix)
}

fn group(version: u8) -> Result<&'static [u8], Error> {
    match version {
        VERSION_D => Ok(b"SlotsD"),
        VERSION_P => Ok(b"SlotsP"),
        VERSION_PT => Ok(b"SlotsPt"),
        VERSION_HG => Ok(b"SlotsHG"),
        VERSION_SS => Ok(b"SlotsSS"),
        _ => Err(Error::VersionUnsupported),
    }
}

/// The encounter data holding a version's slots.
fn enc_data<'c, 'a>(
    ctx: &'c Context4<'a>,
    version: u8,
) -> &'c crate::data::encounters::EncounterData<'a> {
    if matches!(version, VERSION_HG | VERSION_SS) {
        &ctx.enc_hgss
    } else {
        &ctx.enc_dppt
    }
}

fn form_matches(e: &Encounter, form: u8) -> bool {
    e.has(crate::data::encounters::flag::RANDOM_UNSPECIFIC_FORM) || e.form() == form
}

/// Wild slots of `version` for `species`/`form`, in data order (for an encounter picker).
pub fn encounters<'a>(
    ctx: &Context4<'a>,
    version: u8,
    species: u16,
    form: u8,
) -> Result<Vec<Encounter<'a>>, Error> {
    let g = group(version)?;
    Ok(enc_data(ctx, version)
        .group(g)
        .filter(|e| e.species() == species && form_matches(e, form))
        .collect())
}

/// Gen 4 wild balls (PKHeX `BallUseLegality.WildPokeBalls4_DPPt`, plus Kurt's Apricorn Balls
/// on HG/SS) or the slot's required ball.
fn ball_allowed(e: &Encounter, ball: u8, version: u8) -> bool {
    let required = super::build4::required_ball(e);
    if required != 0 {
        return ball == required;
    }
    matches!(ball, 1..=4 | 6..=15) || (matches!(version, 7 | 8) && (17..=23).contains(&ball))
}

pub fn trainer_of(save: &Save) -> Trainer4 {
    let t = save.trainer();
    Trainer4 {
        name_raw: t.name_raw,
        tid: t.tid,
        sid: t.sid,
        gender: t.gender,
        language: t.language,
    }
}

/// The Pokémon's PID ability bit for an ability ID, if the species has it.
fn ability_bit(ctx: &Context4, species: u16, form: u8, ability: u8) -> Result<Option<u8>, Error> {
    let [a1, a2, _] = ctx.hgss.species_form(species, form).abilities();
    match ability {
        a if a == a1 && a == a2 => Ok(None),
        a if a == a1 => Ok(Some(0)),
        a if a == a2 => Ok(Some(1)),
        _ => Err(Error::AbilityUnavailable),
    }
}

fn first_failure(pk: &Pkm, ctx: &Context4) -> Option<Error> {
    let rep = analyze4(ctx, pk);
    if rep.valid() {
        return None;
    }
    if !rep.moves.iter().all(|m| m.valid()) {
        return Some(Error::MovesIllegal);
    }
    Some(Error::Illegal(
        rep.invalid.first().copied().unwrap_or(CheckId::Misc),
    ))
}

fn generate_slot(
    ctx: &Context4,
    req: &GenRequest,
    tr: &Trainer4,
    e: &Encounter,
    rand: &mut Rand,
) -> Result<Generated4, Error> {
    if e.species() != req.species || !form_matches(e, req.form) || e.version() != req.version {
        return Err(Error::NotInEncounter);
    }
    let slot = Slot4::from_encounter(e).ok_or(Error::NotInEncounter)?;
    let form = if req.species == 201 {
        req.form
    } else {
        e.form()
    };
    if let Some(b) = req.ball
        && !ball_allowed(e, b, req.version)
    {
        return Err(Error::BallNotAllowed);
    }
    let ability_bit = match req.ability {
        Some(a) => ability_bit(ctx, req.species, form, a)?,
        None => None,
    };
    let ratio = |s: u16| ctx.ratio(s);
    let wreq = Request {
        slot,
        tid: tr.tid,
        sid: tr.sid,
        species: req.species,
        gender_ratio: ratio(req.species),
        met_level: req.level,
        nature: req.nature,
        gender: req.gender,
        ability_bit,
        shiny: req.shiny,
        lead: req.lead,
        hidden_power_type: req.hidden_power_type,
        location: e.location(),
        form,
        version: req.version,
    };
    let (frame, iv_proposal) = match req.ivs {
        IvRequest::Random => (wild4::generate(&wreq, rand, req.budget, ratio)?, None),
        IvRequest::Exact(ivs) => (wild4::generate_from_ivs(&wreq, ivs, ratio)?, None),
        IvRequest::Nearest(ivs) => {
            let p = wild4::closest_legal_ivs(&wreq, ivs, req.budget, ratio)?;
            (p.generated, Some(p))
        }
    };
    let bd = BuildData {
        hgss: ctx.hgss,
        learn: match req.version {
            VERSION_PT => ctx.learn.pt,
            VERSION_HG | VERSION_SS => ctx.learn.hgss,
            _ => ctx.learn.dp,
        },
        moves: ctx.learn.moves,
        names4: ctx.names4,
    };
    let mut pk = build_wild4(&bd, e, &frame, tr, req.met_date);
    if let Some(b) = req.ball {
        pk.set_ball(b);
    }
    if let Some(m) = req.moves {
        pk.set_moves_with_pp(m, &ctx.learn.moves);
    }
    pk.refresh_checksum();
    if let Some(err) = first_failure(&pk, ctx) {
        return Err(err);
    }
    Ok(Generated4 {
        pkm: pk,
        frame,
        iv_proposal,
    })
}

/// Errors ranked by how much they say about the request (an "any encounter" request reports
/// the most specific reason across the slots it tried).
fn specificity(e: &Error) -> u8 {
    match e {
        Error::Gen(GenError::BudgetExhausted) => 0,
        Error::Gen(GenError::LevelOutsideSlot) | Error::NotInEncounter => 1,
        Error::Gen(GenError::MunchlaxTreeUnavailable | GenError::RadarNotAllowedHere) => 2,
        Error::BallNotAllowed => 3,
        Error::Illegal(_) => 5,
        _ => 4,
    }
}

/// Generates a legal D/P/Pt wild Pokémon for the save's trainer.
pub fn generate(ctx: &Context4, save: &Save, req: &GenRequest) -> Result<Generated4, Error> {
    generate_for(ctx, &trainer_of(save), req)
}

/// [`generate`] for an explicit trainer.
pub fn generate_for(ctx: &Context4, tr: &Trainer4, req: &GenRequest) -> Result<Generated4, Error> {
    group(req.version)?;
    let mut rand = Rand::new(req.seed);
    match req.encounter {
        EncounterChoice::Slot(e) => generate_slot(ctx, req, tr, &e, &mut rand),
        EncounterChoice::Any => {
            let mut list = encounters(ctx, req.version, req.species, req.form)?;
            if let Some(l) = req.level {
                list.retain(|e| {
                    (e.level_min()..=e.level_max().max(e.pressure_level())).contains(&l)
                });
            }
            if list.is_empty() {
                return Err(Error::NoEncounter);
            }
            // Fisher-Yates: spread results across slots rather than always the first.
            for i in (1..list.len()).rev() {
                let j = rand.next_u32() as usize % (i + 1);
                list.swap(i, j);
            }
            let mut best: Option<Error> = None;
            for e in &list {
                match generate_slot(ctx, req, tr, e, &mut rand) {
                    Ok(g) => return Ok(g),
                    Err(err) => {
                        if best.is_none_or(|b| specificity(&err) > specificity(&b)) {
                            best = Some(err);
                        }
                    }
                }
            }
            Err(best.unwrap_or(Error::NoEncounter))
        }
    }
}

/// The safety rules for a Gen 4 save, from the legality context's tables.
pub fn rules4<'a>(ctx: &Context4<'a>, kind: SaveKind) -> Option<crate::safety::Rules<'a>> {
    let game = match kind {
        SaveKind::DP => ctx.learn.dp,
        SaveKind::Pt => ctx.learn.pt,
        SaveKind::HGSS => ctx.hgss,
        _ => return None,
    };
    Some(crate::safety::Rules {
        kind,
        game,
        moves: ctx.learn.moves,
    })
}

/// Writes a Pokémon into `slot` as the game would on capture: party slots get computed stats
/// (PKHeX `ResetPartyStats`) and must stay contiguous, the write goes through the game-safe validator,
/// and the Pokédex records it (`apply_capture_side_effects`). Call [`Save::finalize`] before
/// writing the save out.
pub fn insert(ctx: &Context4, save: &mut Save, slot: Slot, pk: &Pkm) -> Result<(), Error> {
    let rules = rules4(ctx, save.kind()).ok_or(Error::Save(SaveError::BadLength))?;
    let pk = match slot {
        Slot::Party(i) => {
            if i as usize > save.party_count() || i >= 6 {
                return Err(Error::PartyGap);
            }
            let info = ctx.hgss.species_form(pk.species(), pk.form()).base_info();
            let mut p = pk.clone();
            p.reset_party_stats(&info);
            p.refresh_checksum();
            if !p.is_party_format() {
                return Err(Error::WrongFormat);
            }
            p
        }
        Slot::Box(..) => {
            let mut p = pk.to_stored();
            p.refresh_checksum();
            p
        }
    };
    crate::safety::checked_set_pkm(&rules, save, slot, &pk).map_err(Error::Unsafe)?;
    apply_capture_side_effects(save, &pk);
    Ok(())
}

/// Seen/caught and form/gender Pokédex flags for a newly obtained Pokémon (PKHeX `SetDex`).
pub fn apply_capture_side_effects(save: &mut Save, pk: &Pkm) {
    save.register_dex(pk);
}
