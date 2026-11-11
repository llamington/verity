//! The add-Pokémon wizards' entry points (wild Gen 4/5 slots, Gen 4 eggs), shared by the DS
//! (`verity-ffi`) and the browser (`verity-wasm`).

use alloc::vec::Vec;

use super::api4::{self, EncounterChoice, GenRequest, IvRequest};
use super::status::GenStatus;
use super::wild4::Lead;
use crate::data::encounters::{Encounter, EncounterData};
use crate::legality::analysis4::{Context4, can_be_egg};
use crate::legality::analysis5::Context5;
use crate::save::{Save, SaveKind, Slot};

/// The encounter group holding `version`'s wild slots (10 D, 11 P, 12 Pt, 7 HG, 8 SS, 20 W,
/// 21 B, 22 W2, 23 B2).
pub fn slots_group(version: u8) -> Option<&'static [u8]> {
    match version {
        10 => Some(b"SlotsD"),
        11 => Some(b"SlotsP"),
        12 => Some(b"SlotsPt"),
        7 => Some(b"SlotsHG"),
        8 => Some(b"SlotsSS"),
        20 => Some(b"SlotsW"),
        21 => Some(b"SlotsB"),
        22 => Some(b"SlotsW2"),
        23 => Some(b"SlotsB2"),
        _ => None,
    }
}

/// The encounter tables holding `version`'s wild slots.
pub fn wild_data<'a>(
    c4: Option<&'a Context4<'a>>,
    c5: Option<&'a Context5<'a>>,
    version: u8,
) -> Option<&'a EncounterData<'a>> {
    match version {
        20 | 21 => Some(&c5?.enc_bw),
        22 | 23 => Some(&c5?.enc_b2w2),
        7 | 8 => Some(&c4?.enc_hgss),
        10..=12 => Some(&c4?.enc_dppt),
        _ => None,
    }
}

/// The distinct species with wild slots in `version`, ascending.
pub fn wild_species(data: &EncounterData, version: u8) -> Vec<u16> {
    let Some(g) = slots_group(version) else {
        return Vec::new();
    };
    let mut seen = [false; 650];
    for e in data.group(g) {
        if let Some(s) = seen.get_mut(e.species() as usize) {
            *s = true;
        }
    }
    (1..seen.len() as u16)
        .filter(|&s| seen[s as usize])
        .collect()
}

/// The wild slots of `species` in `version`, with the index [`WildRequest::encounter`] takes.
pub fn wild_encounters<'a>(
    data: &'a EncounterData<'a>,
    version: u8,
    species: u16,
) -> Vec<(u16, Encounter<'a>)> {
    let Some(g) = slots_group(version) else {
        return Vec::new();
    };
    data.group(g)
        .enumerate()
        .filter(|(_, e)| e.species() == species)
        .map(|(i, e)| (i as u16, e))
        .collect()
}

/// A wild Pokémon to generate; `None` means any.
#[derive(Clone, Copy, Debug)]
pub struct WildRequest {
    pub version: u8,
    pub species: u16,
    pub form: u8,
    /// An index from [`wild_encounters`]; `None` picks one.
    pub encounter: Option<u16>,
    pub level: Option<u8>,
    pub nature: Option<u8>,
    pub gender: Option<u8>,
    pub shiny: bool,
    /// Gen 4 only.
    pub lead: Lead,
    pub ball: Option<u8>,
    /// Years since 2000, month, day.
    pub met_date: [u8; 3],
    pub seed: u32,
}

/// Generates a legal wild Pokémon for the save's trainer and writes it to `slot` (through the
/// game-safe validator, with Pokédex side effects). The save is not finalised.
pub fn insert_wild(
    save: &mut Save,
    c4: Option<&Context4>,
    c5: Option<&Context5>,
    r: &WildRequest,
    slot: Slot,
) -> Result<(), GenStatus> {
    let g = slots_group(r.version).ok_or(GenStatus::Unsupported)?;
    if matches!(save.kind(), SaveKind::BW | SaveKind::B2W2) {
        return insert_wild5(save, c5.ok_or(GenStatus::Unsupported)?, r, g, slot);
    }
    if !matches!(r.version, 7 | 8 | 10..=12) {
        return Err(GenStatus::Unsupported);
    }
    let ctx = c4.ok_or(GenStatus::Unsupported)?;
    let encounter = match r.encounter {
        None => EncounterChoice::Any,
        Some(i) => {
            let data = wild_data(Some(ctx), None, r.version).ok_or(GenStatus::Unsupported)?;
            EncounterChoice::Slot(
                data.group(g)
                    .nth(i as usize)
                    .ok_or(GenStatus::NotInEncounter)?,
            )
        }
    };
    let q = GenRequest {
        version: r.version,
        species: r.species,
        form: r.form,
        encounter,
        level: r.level,
        nature: r.nature,
        gender: r.gender,
        ability: None,
        shiny: r.shiny || r.lead == Lead::PokeRadarChain,
        lead: r.lead,
        hidden_power_type: None,
        ivs: IvRequest::Random,
        ball: r.ball,
        moves: None,
        met_date: r.met_date,
        seed: r.seed as u64 | 1,
        budget: 20_000,
    };
    let built = api4::generate(ctx, save, &q)?;
    api4::insert(ctx, save, slot, &built.pkm)?;
    Ok(())
}

fn insert_wild5(
    save: &mut Save,
    ctx: &Context5,
    r: &WildRequest,
    g: &[u8],
    slot: Slot,
) -> Result<(), GenStatus> {
    use super::wild5::{self, Error5, Request5};
    let ed = match r.version {
        20 | 21 => &ctx.enc_bw,
        22 | 23 => &ctx.enc_b2w2,
        _ => return Err(GenStatus::Unsupported),
    };
    let encounter = match r.encounter {
        None => {
            let count = ed.group(g).filter(|e| e.species() == r.species).count();
            if count == 0 {
                return Err(GenStatus::NoEncounter);
            }
            ed.group(g)
                .filter(|e| e.species() == r.species)
                .nth(r.seed as usize % count)
        }
        Some(i) => ed.group(g).nth(i as usize),
    }
    .ok_or(GenStatus::NotInEncounter)?;
    if encounter.species() != r.species {
        return Err(GenStatus::NotInEncounter);
    }
    let q = Request5 {
        encounter,
        level: r.level,
        nature: r.nature,
        gender: r.gender,
        shiny: r.shiny,
        ball: r.ball,
        date: r.met_date,
        seed: r.seed as u64 | 1,
    };
    let built = wild5::generate(ctx, &api4::trainer_of(save), &q).map_err(|e| match e {
        Error5::LevelOutsideSlot => GenStatus::LevelOutsideSlot,
        Error5::GenderImpossible => GenStatus::GenderImpossible,
        Error5::ShinyImpossible => GenStatus::ShinyInGrotto,
        Error5::BallNotAllowed => GenStatus::BallNotAllowed,
        Error5::Illegal => GenStatus::Internal,
    })?;
    wild5::insert(ctx, save, slot, &built).map_err(|_| GenStatus::Unsafe)
}

/// Species a Gen 4 egg can be of.
pub fn egg_species(ctx: &Context4) -> Vec<u16> {
    (1..=493u16).filter(|&s| can_be_egg(ctx, s)).collect()
}

/// A bred Gen 4 egg to generate; `None` means any.
#[derive(Clone, Copy, Debug)]
pub struct EggRequest {
    pub version: u8,
    pub species: u16,
    pub nature: Option<u8>,
    pub gender: Option<u8>,
    pub shiny: bool,
    /// Hatched already (met at the hatch location) rather than still an egg.
    pub hatched: bool,
    /// Years since 2000, month, day: the egg date and, when hatched, the met date.
    pub date: [u8; 3],
    pub seed: u32,
}

/// Builds a legal bred egg for the save's trainer and writes it to `slot`. Gen 4 saves only.
pub fn insert_egg(
    save: &mut Save,
    ctx: &Context4,
    r: &EggRequest,
    slot: Slot,
) -> Result<(), GenStatus> {
    if !matches!(save.kind(), SaveKind::DP | SaveKind::Pt | SaveKind::HGSS)
        || slots_group(r.version).is_none()
    {
        return Err(GenStatus::Unsupported);
    }
    let e = super::egg4::EggRequest {
        version: r.version,
        species: r.species,
        nature: r.nature,
        gender: r.gender,
        ability: None,
        shiny: r.shiny,
        ivs: IvRequest::Random,
        moves: None,
        hatched: r.hatched,
        hatch_location: None,
        egg_date: r.date,
        met_date: r.date,
        seed: r.seed as u64 | 1,
        budget: 20_000,
    };
    let built = super::egg4::generate(ctx, &api4::trainer_of(save), &e)?;
    api4::insert(ctx, save, slot, &built)?;
    Ok(())
}
