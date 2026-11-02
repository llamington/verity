//! Write-side FFI for the DS front end: wild encounter lists, legal generation + insertion
//! (P6 `gen::api4`), save finalisation and the game-safe validator (P7). Gen 4 D/P/Pt only.

use core::slice;

use verity::gen::api4::{self, EncounterChoice, Error, GenRequest, IvRequest};
use verity::gen::wild4::{GenError, Lead};
use verity::safety::{self, SaveIssue};
use verity::save::{SaveKind, Slot};

use crate::ui::{legal, open, VerityLoc};
use crate::VerityErr;

fn group(version: u8) -> Option<&'static [u8]> {
    match version {
        10 => Some(b"SlotsD"),
        11 => Some(b"SlotsP"),
        12 => Some(b"SlotsPt"),
        _ => None,
    }
}

/// One wild slot (`verity_encounter`). `index` identifies it for `verity_gen_insert`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct VerityEncounter {
    pub index: u16,
    pub location: u16,
    pub species: u16,
    pub form: u8,
    pub level_min: u8,
    pub level_max: u8,
    /// PKHeX `SlotType4` (0 grass, 1 surf, 2 old rod, 3 good rod, 4 super rod, ...).
    pub slot_type: u8,
    pub slot_number: u8,
    pub can_radar: u8,
}

/// C ABI: the distinct species with wild slots in `version` (10 D, 11 P, 12 Pt), ascending.
/// Returns how many were written (at most `cap`).
///
/// # Safety
///
/// `out` must point to `cap` writable `u16`s.
#[no_mangle]
pub unsafe extern "C" fn verity_wild_species(version: u8, out: *mut u16, cap: usize) -> usize {
    let (Some(g), Some(ctx)) = (group(version), legal::context()) else {
        return 0;
    };
    if out.is_null() {
        return 0;
    }
    let out = slice::from_raw_parts_mut(out, cap);
    let mut seen = [false; 494];
    for e in ctx.enc_dppt.group(g) {
        if let Some(s) = seen.get_mut(e.species() as usize) {
            *s = true;
        }
    }
    let mut n = 0;
    for (sp, &s) in seen.iter().enumerate() {
        if s && n < cap {
            out[n] = sp as u16;
            n += 1;
        }
    }
    n
}

/// C ABI: the wild slots of `species` in `version`. Returns how many were written.
///
/// # Safety
///
/// `out` must point to `cap` writable `VerityEncounter`s.
#[no_mangle]
pub unsafe extern "C" fn verity_wild_encounters(
    version: u8,
    species: u16,
    out: *mut VerityEncounter,
    cap: usize,
) -> usize {
    let (Some(g), Some(ctx)) = (group(version), legal::context()) else {
        return 0;
    };
    if out.is_null() {
        return 0;
    }
    let out = slice::from_raw_parts_mut(out, cap);
    let mut n = 0;
    for (i, e) in ctx.enc_dppt.group(g).enumerate() {
        if e.species() != species || n >= cap {
            continue;
        }
        out[n] = VerityEncounter {
            index: i as u16,
            location: e.location(),
            species: e.species(),
            form: e.form(),
            level_min: e.level_min(),
            level_max: e.level_max(),
            slot_type: e.slot_type(),
            slot_number: e.slot_number(),
            can_radar: e.can_radar() as u8,
        };
        n += 1;
    }
    n
}

/// Request for `verity_gen_insert` (`verity_gen_request`). 0xFF / 0xFFFF mean "any".
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct VerityGenRequest {
    pub version: u8,
    pub form: u8,
    pub species: u16,
    /// From `verity_wild_encounters`, or 0xFFFF for any slot.
    pub encounter_index: u16,
    pub level: u8,
    pub nature: u8,
    pub gender: u8,
    pub shiny: u8,
    /// 0 none, 1 Synchronize (lead_param = nature), 2 Cute Charm (lead_param = gender of the
    /// result), 3 Poké Radar chain.
    pub lead: u8,
    pub lead_param: u8,
    pub ball: u8,
    /// Met date: years since 2000, month, day.
    pub met_date: [u8; 3],
    pub seed: u32,
}

/// Result codes of `verity_gen_insert` (`verity_gen_status`).
#[repr(u8)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GenStatus {
    Ok = 0,
    Unsupported = 1,
    NoEncounter = 2,
    NotInEncounter = 3,
    LevelOutsideSlot = 4,
    NatureConflictsWithSynchronize = 5,
    ShinyImpossibleForCuteCharm = 6,
    GenderImpossible = 7,
    CuteCharmNeedsGenderedSpecies = 8,
    FormUnavailable = 9,
    RadarNotAllowedHere = 10,
    MunchlaxTreeUnavailable = 11,
    NoFrameFound = 12,
    BallNotAllowed = 13,
    AbilityUnavailable = 14,
    MovesIllegal = 15,
    SlotUnavailable = 16,
    Unsafe = 17,
    Internal = 18,
}

fn status(e: Error) -> GenStatus {
    use GenStatus as S;
    match e {
        Error::Gen(g) => match g {
            GenError::LevelOutsideSlot => S::LevelOutsideSlot,
            GenError::NatureConflictsWithSynchronize => S::NatureConflictsWithSynchronize,
            GenError::ShinyImpossibleForCuteCharm => S::ShinyImpossibleForCuteCharm,
            GenError::GenderImpossible => S::GenderImpossible,
            GenError::CuteCharmNeedsGenderedSpecies => S::CuteCharmNeedsGenderedSpecies,
            GenError::FormUnavailable => S::FormUnavailable,
            GenError::RadarNotAllowedHere => S::RadarNotAllowedHere,
            GenError::MunchlaxTreeUnavailable => S::MunchlaxTreeUnavailable,
            GenError::BudgetExhausted => S::NoFrameFound,
        },
        Error::VersionUnsupported => S::Unsupported,
        Error::NoEncounter => S::NoEncounter,
        Error::NotInEncounter => S::NotInEncounter,
        Error::BallNotAllowed => S::BallNotAllowed,
        Error::AbilityUnavailable => S::AbilityUnavailable,
        Error::MovesIllegal => S::MovesIllegal,
        Error::PartyGap | Error::Save(_) | Error::WrongFormat => S::SlotUnavailable,
        Error::Unsafe(_) => S::Unsafe,
        Error::Illegal(_) => S::Internal,
    }
}

/// C ABI: human-readable text for a `verity_gen_status` (static, NUL-terminated ASCII).
#[no_mangle]
pub extern "C" fn verity_gen_status_text(code: u8) -> *const u8 {
    let s: &'static [u8] = match code {
        0 => b"OK\0",
        1 => b"Only D/P/Pt wild encounters for now\0",
        2 => b"No wild encounter in this game\0",
        3 => b"Not in that encounter\0",
        4 => b"Level outside the slot\0",
        5 => b"Nature conflicts with Synchronize\0",
        6 => b"Shiny impossible with Cute Charm\0",
        7 => b"Gender impossible here\0",
        8 => b"Cute Charm needs a gendered species\0",
        9 => b"Form not available here\0",
        10 => b"Poke Radar not usable here\0",
        11 => b"Munchlax not in this tree for this ID\0",
        12 => b"No legal frame found (try again)\0",
        13 => b"Ball not allowed\0",
        14 => b"Ability not available\0",
        15 => b"Moves not legal\0",
        16 => b"Slot not available\0",
        17 => b"Rejected by the game-safe check\0",
        _ => b"Internal error\0",
    };
    s.as_ptr()
}

/// C ABI: generates a legal Pokémon for the save's trainer and writes it to `loc` (through the
/// game-safe validator, with Pokédex side effects). The save is not finalised; call
/// `verity_save_finalize` before writing it out.
///
/// # Safety
///
/// `buf` must point to `len` writable bytes; `req` must be readable.
#[no_mangle]
pub unsafe extern "C" fn verity_gen_insert(
    buf: *mut u8,
    len: usize,
    req: *const VerityGenRequest,
    loc: VerityLoc,
) -> u8 {
    if req.is_null() {
        return GenStatus::Internal as u8;
    }
    let r = *req;
    let Some(mut save) = open(buf, len) else {
        return GenStatus::Internal as u8;
    };
    if !matches!(save.kind(), SaveKind::DP | SaveKind::Pt | SaveKind::HGSS)
        || group(r.version).is_none()
    {
        return GenStatus::Unsupported as u8;
    }
    let Some(ctx) = legal::context() else {
        return GenStatus::Unsupported as u8;
    };
    let encounter = if r.encounter_index == 0xFFFF {
        EncounterChoice::Any
    } else {
        match ctx
            .enc_dppt
            .group(group(r.version).unwrap())
            .nth(r.encounter_index as usize)
        {
            Some(e) => EncounterChoice::Slot(e),
            None => return GenStatus::NotInEncounter as u8,
        }
    };
    let opt = |v: u8| (v != 0xFF).then_some(v);
    let lead = match r.lead {
        1 => Lead::Synchronize(r.lead_param),
        2 => Lead::CuteCharm {
            gender: r.lead_param,
        },
        3 => Lead::PokeRadarChain,
        _ => Lead::None,
    };
    let g = GenRequest {
        version: r.version,
        species: r.species,
        form: r.form,
        encounter,
        level: opt(r.level),
        nature: opt(r.nature),
        gender: opt(r.gender),
        ability: None,
        shiny: r.shiny != 0 || lead == Lead::PokeRadarChain,
        lead,
        hidden_power_type: None,
        ivs: IvRequest::Random,
        ball: opt(r.ball),
        moves: None,
        met_date: r.met_date,
        seed: r.seed as u64 | 1,
        budget: 20_000,
    };
    let built = match api4::generate(ctx, &save, &g) {
        Ok(b) => b,
        Err(e) => return status(e) as u8,
    };
    let slot = if loc.box_index == 0xFF {
        Slot::Party(loc.slot)
    } else {
        Slot::Box(loc.box_index, loc.slot)
    };
    match api4::insert(ctx, &mut save, slot, &built.pkm) {
        Ok(()) => GenStatus::Ok as u8,
        Err(e) => status(e) as u8,
    }
}

/// C ABI: fixes every block checksum (and DP/Pt export state) before the save is written out.
///
/// # Safety
///
/// `buf` must point to `len` writable bytes.
#[no_mangle]
pub unsafe extern "C" fn verity_save_finalize(buf: *mut u8, len: usize) -> VerityErr {
    match open(buf, len) {
        Some(mut s) => {
            s.finalize();
            VerityErr::Ok
        }
        None => VerityErr::Unrecognized,
    }
}

/// Validator verdict (`verity_save_check`): `issue` 0 = safe.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct VeritySaveCheck {
    /// 0 safe, 1 party count, 2 no usable party member, 3 a Pokémon (see loc), 4 checksums,
    /// 5 validator data missing.
    pub issue: u8,
    pub box_index: u8,
    pub slot: u8,
}

/// C ABI: runs the P7 game-safe validator over the whole save (Gen 4).
///
/// # Safety
///
/// `buf` must point to `len` writable bytes; `out` to a writable `VeritySaveCheck`.
#[no_mangle]
pub unsafe extern "C" fn verity_save_check(
    buf: *mut u8,
    len: usize,
    out: *mut VeritySaveCheck,
) -> VerityErr {
    if out.is_null() {
        return VerityErr::NullPointer;
    }
    let Some(save) = open(buf, len) else {
        return VerityErr::Unrecognized;
    };
    let mut c = VeritySaveCheck::default();
    match legal::context().and_then(|ctx| api4::rules4(ctx, save.kind())) {
        None => c.issue = 5,
        Some(rules) => match safety::check_save(&rules, &save) {
            Ok(()) => {}
            Err(SaveIssue::PartyCount) => c.issue = 1,
            Err(SaveIssue::NoUsablePartyMember) => c.issue = 2,
            Err(SaveIssue::Pkm(slot, _)) => {
                c.issue = 3;
                (c.box_index, c.slot) = match slot {
                    Slot::Party(i) => (0xFF, i),
                    Slot::Box(b, s) => (b, s),
                };
            }
            Err(SaveIssue::Checksums) => c.issue = 4,
        },
    }
    *out = c;
    VerityErr::Ok
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::ui::{verity_data_attach, verity_slot_view, BlobKind, VerityPkmView};
    extern crate std;

    fn file(rel: &str) -> Option<std::vec::Vec<u8>> {
        std::fs::read(std::format!("{}/../../{rel}", env!("CARGO_MANIFEST_DIR"))).ok()
    }

    #[test]
    fn generate_insert_finalize_check() {
        let _guard = crate::TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let Some(mut save) = file("corpus/real/pkhex-avalonia/gen4_platinum.sav") else {
            return;
        };
        let blobs = [
            ("hgss.bin", BlobKind::Hgss),
            ("pt.bin", BlobKind::Pt),
            ("dp.bin", BlobKind::Dp),
            ("b2w2.bin", BlobKind::B2w2),
            ("moves4.bin", BlobKind::Moves4),
            ("enc_dppt.bin", BlobKind::EncDppt),
            ("names4.bin", BlobKind::Names4),
        ];
        for (n, k) in blobs {
            let Some(b) = file(&std::format!("data/blobs/{n}")) else {
                return;
            };
            let b: &'static [u8] = std::boxed::Box::leak(b.into_boxed_slice());
            unsafe {
                assert_eq!(
                    verity_data_attach(k as u32, b.as_ptr(), b.len()),
                    VerityErr::Ok
                )
            };
        }
        unsafe {
            let mut sp = [0u16; 494];
            let n = verity_wild_species(12, sp.as_mut_ptr(), sp.len());
            assert!(n > 100 && sp[..n].contains(&399));
            let mut enc = [VerityEncounter::default(); 64];
            let m = verity_wild_encounters(12, 399, enc.as_mut_ptr(), enc.len());
            assert!(m > 0);
            let (b, len) = (save.as_mut_ptr(), save.len());
            // Find an empty box slot.
            let mut v = core::mem::zeroed::<VerityPkmView>();
            let mut target = None;
            'f: for bx in 0..18u8 {
                for sl in 0..30u8 {
                    let loc = VerityLoc {
                        box_index: bx,
                        slot: sl,
                    };
                    verity_slot_view(b, len, loc, &mut v);
                    if v.species == 0 {
                        target = Some(loc);
                        break 'f;
                    }
                }
            }
            let loc = target.unwrap();
            let req = VerityGenRequest {
                version: 12,
                form: 0,
                species: 399,
                encounter_index: enc[0].index,
                level: 0xFF,
                nature: 3,
                gender: 0xFF,
                shiny: 1,
                lead: 0,
                lead_param: 0,
                ball: 0xFF,
                met_date: [26, 11, 2],
                seed: 7,
            };
            assert_eq!(verity_gen_insert(b, len, &req, loc), GenStatus::Ok as u8);
            verity_slot_view(b, len, loc, &mut v);
            assert_eq!((v.species, v.nature, v.is_shiny), (399, 3, 1));
            // Synchronize Adamant with Modest requested: specific error.
            let bad = VerityGenRequest {
                nature: 15,
                lead: 1,
                lead_param: 3,
                ..req
            };
            assert_eq!(
                verity_gen_insert(b, len, &bad, loc),
                GenStatus::NatureConflictsWithSynchronize as u8
            );
            assert_eq!(verity_save_finalize(b, len), VerityErr::Ok);
            let mut c = VeritySaveCheck::default();
            assert_eq!(verity_save_check(b, len, &mut c), VerityErr::Ok);
            assert_eq!(c.issue, 0, "{c:?}");
        }
    }
}
