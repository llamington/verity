//! Write-side FFI for the DS front end: wild encounter lists, legal generation + insertion
//! (P6 `generate::api4`), save finalisation and the game-safe validator (P7). Gen 4 D/P/Pt only.

use core::slice;

use verity::generate::api4::{self, Error};
use verity::generate::wild4::Lead;
use verity::safety::{self, SaveIssue};
use verity::save::{SaveKind, Slot};

use crate::VerityErr;
use crate::ui::{VerityLoc, legal, open};

use verity::generate::wizard;

/// The encounter tables holding `version`'s wild slots (builds only the context it needs).
fn wild_data(version: u8) -> Option<&'static verity::data::encounters::EncounterData<'static>> {
    if matches!(version, 20..=23) {
        wizard::wild_data(None, Some(legal::context5()?), version)
    } else {
        wizard::wild_data(Some(legal::context()?), None, version)
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

/// C ABI: the distinct species with wild slots in `version` (10 D, 11 P, 12 Pt, 7 HG, 8 SS),
/// ascending.
/// Returns how many were written (at most `cap`).
///
/// # Safety
///
/// `out` must point to `cap` writable `u16`s.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn verity_wild_species(version: u8, out: *mut u16, cap: usize) -> usize {
    let Some(data) = wild_data(version) else {
        return 0;
    };
    if out.is_null() {
        return 0;
    }
    // SAFETY: `out` is non-null (checked above) and valid for `cap` writable elements (this
    // function's contract).
    let out = unsafe { slice::from_raw_parts_mut(out, cap) };
    let species = wizard::wild_species(data, version);
    let n = species.len().min(cap);
    out[..n].copy_from_slice(&species[..n]);
    n
}

/// C ABI: the wild slots of `species` in `version`. Returns how many were written.
///
/// # Safety
///
/// `out` must point to `cap` writable `VerityEncounter`s.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn verity_wild_encounters(
    version: u8,
    species: u16,
    out: *mut VerityEncounter,
    cap: usize,
) -> usize {
    let Some(data) = wild_data(version) else {
        return 0;
    };
    if out.is_null() {
        return 0;
    }
    // SAFETY: `out` is non-null (checked above) and valid for `cap` writable elements (this
    // function's contract).
    let out = unsafe { slice::from_raw_parts_mut(out, cap) };
    let mut n = 0;
    for (index, e) in wizard::wild_encounters(data, version, species)
        .into_iter()
        .take(cap)
    {
        out[n] = VerityEncounter {
            index,
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

pub use verity::generate::status::GenStatus;

fn status(e: Error) -> GenStatus {
    e.into()
}

/// C ABI: human-readable text for a `verity_gen_status` (static, NUL-terminated ASCII).
#[unsafe(no_mangle)]
pub extern "C" fn verity_gen_status_text(code: u8) -> *const u8 {
    GenStatus::c_text(code).as_ptr()
}

/// C ABI: generates a legal Pokémon for the save's trainer and writes it to `loc` (through the
/// game-safe validator, with Pokédex side effects). The save is not finalised; call
/// `verity_save_finalize` before writing it out.
///
/// # Safety
///
/// `buf` must point to `len` writable bytes; `req` must be readable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn verity_gen_insert(
    buf: *mut u8,
    len: usize,
    req: *const VerityGenRequest,
    loc: VerityLoc,
) -> u8 {
    if req.is_null() {
        return GenStatus::Internal as u8;
    }
    // SAFETY: `req` is non-null (checked above) and points to a readable value (this function's
    // contract).
    let r = unsafe { *req };
    // SAFETY: `buf` is null or valid for `len` writable bytes, and the `Save` is dropped before
    // this call returns (this function's contract).
    let Some(mut save) = (unsafe { open(buf, len) }) else {
        return GenStatus::Internal as u8;
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
    let w = wizard::WildRequest {
        version: r.version,
        species: r.species,
        form: r.form,
        encounter: (r.encounter_index != 0xFFFF).then_some(r.encounter_index),
        level: opt(r.level),
        nature: opt(r.nature),
        gender: opt(r.gender),
        shiny: r.shiny != 0,
        lead,
        ball: opt(r.ball),
        met_date: r.met_date,
        seed: r.seed,
    };
    // Build only the context this save needs.
    let gen5 = matches!(save.kind(), SaveKind::BW | SaveKind::B2W2);
    let (c4, c5) = if gen5 {
        (None, legal::context5())
    } else {
        (legal::context(), None)
    };
    match wizard::insert_wild(&mut save, c4, c5, &w, slot_of(loc)) {
        Ok(()) => GenStatus::Ok as u8,
        Err(s) => s as u8,
    }
}

/// C ABI: fixes every block checksum (and DP/Pt export state) before the save is written out.
///
/// # Safety
///
/// `buf` must point to `len` writable bytes.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn verity_save_finalize(buf: *mut u8, len: usize) -> VerityErr {
    // SAFETY: `buf` is null or valid for `len` writable bytes, and the `Save` is dropped before
    // this call returns (this function's contract).
    match unsafe { open(buf, len) } {
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
#[unsafe(no_mangle)]
pub unsafe extern "C" fn verity_save_check(
    buf: *mut u8,
    len: usize,
    out: *mut VeritySaveCheck,
) -> VerityErr {
    if out.is_null() {
        return VerityErr::NullPointer;
    }
    // SAFETY: `buf` is null or valid for `len` writable bytes, and the `Save` is dropped before
    // this call returns (this function's contract).
    let Some(save) = (unsafe { open(buf, len) }) else {
        return VerityErr::Unrecognized;
    };
    let mut c = VeritySaveCheck::default();
    match game_rules(save.kind()) {
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
    // SAFETY: `out` is non-null (checked above) and points to a writable value (this function's
    // contract).
    unsafe { *out = c };
    VerityErr::Ok
}

// ---- editor --------------------------------------------------------------------------------

/// Field edits (`verity_edit`); `mask` selects the fields that apply.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct VerityEdit {
    /// 1 held item, 2 moves (PP reset to max), 4 IVs, 8 EVs, 16 level (EXP set to the level's
    /// minimum), 32 friendship.
    pub mask: u32,
    pub held_item: u16,
    pub moves: [u16; 4],
    pub ivs: [u8; 6],
    pub evs: [u8; 6],
    pub level: u8,
    pub friendship: u8,
    /// With `EDIT_PID`: the PID that goes with a closest-legal IV spread.
    pub pid: u32,
    pub met_location: u16,
    pub ball: u8,
    pub met_level: u8,
    pub met_date: [u8; 3],
    pub contest: [u8; 6],
    pub ribbons: [u8; 12],
    /// UTF-16, 0-terminated.
    pub nickname: [u16; 11],
}

pub const EDIT_ITEM: u32 = 1;
pub const EDIT_MOVES: u32 = 2;
pub const EDIT_IVS: u32 = 4;
pub const EDIT_EVS: u32 = 8;
pub const EDIT_LEVEL: u32 = 16;
pub const EDIT_FRIENDSHIP: u32 = 32;
pub const EDIT_PID: u32 = 64;
pub const EDIT_BALL: u32 = 128;
pub const EDIT_MET: u32 = 256;
pub const EDIT_CONTEST: u32 = 512;
pub const EDIT_RIBBONS: u32 = 1024;
pub const EDIT_NICKNAME: u32 = 2048;
pub const EDIT_NICKNAME_RESET: u32 = 4096;

fn game_rules(kind: SaveKind) -> Option<safety::Rules<'static>> {
    use crate::ui::{BlobKind, blob};
    use verity::data::{GameData, MoveData};
    let (g, m) = match kind {
        SaveKind::DP => (BlobKind::Dp, BlobKind::Moves4),
        SaveKind::Pt => (BlobKind::Pt, BlobKind::Moves4),
        SaveKind::HGSS => (BlobKind::Hgss, BlobKind::Moves4),
        SaveKind::BW => (BlobKind::Bw, BlobKind::Moves5),
        SaveKind::B2W2 => (BlobKind::B2w2, BlobKind::Moves5),
    };
    Some(safety::Rules {
        kind,
        game: GameData::load(blob(g)?).ok()?,
        moves: MoveData::load(blob(m)?).ok()?,
    })
}

fn slot_of(loc: VerityLoc) -> Slot {
    if loc.box_index == 0xFF {
        Slot::Party(loc.slot)
    } else {
        Slot::Box(loc.box_index, loc.slot)
    }
}

/// The edited Pokémon (party data recomputed), or None for an empty/out-of-range slot.
fn edited(
    save: &verity::save::Save,
    rules: &safety::Rules,
    loc: VerityLoc,
    e: &VerityEdit,
) -> Option<verity::pkm::Pkm> {
    use verity::edit::{Edit, Met, Nickname};
    let on = |bit: u32| e.mask & bit != 0;
    let nickname = if on(EDIT_NICKNAME) {
        let n = e.nickname.iter().position(|&c| c == 0).unwrap_or(10);
        Some(Nickname::new(&e.nickname[..n]))
    } else if on(EDIT_NICKNAME_RESET) {
        Some(Nickname::Reset)
    } else {
        None
    };
    let edit = Edit {
        held_item: on(EDIT_ITEM).then_some(e.held_item),
        moves: on(EDIT_MOVES).then_some(e.moves),
        pid: on(EDIT_PID).then_some(e.pid),
        ivs: on(EDIT_IVS).then_some(e.ivs),
        evs: on(EDIT_EVS).then_some(e.evs),
        level: on(EDIT_LEVEL).then_some(e.level),
        friendship: on(EDIT_FRIENDSHIP).then_some(e.friendship),
        ball: on(EDIT_BALL).then_some(e.ball),
        met: on(EDIT_MET).then_some(Met {
            location: e.met_location,
            level: e.met_level,
            date: e.met_date,
        }),
        contest: on(EDIT_CONTEST).then_some(e.contest),
        ribbons: on(EDIT_RIBBONS).then_some(e.ribbons),
        nickname,
    };
    // The species names live in the Gen 4 context; build it only for a reset.
    let names = if matches!(nickname, Some(Nickname::Reset)) {
        Some(&legal::context()?.names4)
    } else {
        None
    };
    verity::edit::edited(save, rules, slot_of(loc), &edit, names)
}

/// C ABI: applies `edit` to a copy of the Pokémon at `loc` and reports the game-safe verdict
/// (`issue`: 0 safe, else 1 + `safety::PkmIssue` index) and legality, without writing.
///
/// # Safety
///
/// `buf` must point to `len` writable bytes; `edit`, `legality`, `issue` must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn verity_edit_preview(
    buf: *mut u8,
    len: usize,
    loc: VerityLoc,
    edit: *const VerityEdit,
    legality: *mut crate::ui::VerityLegality,
    issue: *mut u8,
) -> VerityErr {
    if edit.is_null() || legality.is_null() || issue.is_null() {
        return VerityErr::NullPointer;
    }
    // SAFETY: `buf` is null or valid for `len` writable bytes, and the `Save` is dropped before
    // this call returns (this function's contract).
    let Some(save) = (unsafe { open(buf, len) }) else {
        return VerityErr::Unrecognized;
    };
    let Some(rules) = game_rules(save.kind()) else {
        return VerityErr::BadData;
    };
    // SAFETY: `edit` is non-null (checked above) and points to a readable value (this function's
    // contract).
    let Some(pk) = edited(&save, &rules, loc, unsafe { &*edit }) else {
        return VerityErr::BadArgument;
    };
    let code = match safety::check_pkm(&rules, &pk, pk.is_party_format()) {
        Ok(()) => 0,
        Err(i) => 1 + i as u8,
    };
    // SAFETY: `issue` is non-null (checked above) and points to a writable value (this function's
    // contract).
    unsafe { *issue = code };
    let mut r = crate::ui::empty_legality();
    legal::check(&save, &pk, &mut r);
    // SAFETY: `legality` is non-null (checked above) and points to a writable value (this
    // function's contract).
    unsafe { *legality = r };
    VerityErr::Ok
}

/// C ABI: writes `edit` to the Pokémon at `loc` through the game-safe validator. `mode` 0 is
/// legal mode (also refuses a result legality judges illegal), 1 game-safe mode. Returns 0 on
/// success, 1 + `PkmIssue` index when the validator refuses, 100 when legal mode refuses,
/// 200 for a bad slot or missing data.
///
/// # Safety
///
/// `buf` must point to `len` writable bytes; `edit` must be readable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn verity_edit_apply(
    buf: *mut u8,
    len: usize,
    loc: VerityLoc,
    edit: *const VerityEdit,
    mode: u8,
) -> u8 {
    if edit.is_null() {
        return 200;
    }
    // SAFETY: `buf` is null or valid for `len` writable bytes, and the `Save` is dropped before
    // this call returns (this function's contract).
    let Some(mut save) = (unsafe { open(buf, len) }) else {
        return 200;
    };
    let Some(rules) = game_rules(save.kind()) else {
        return 200;
    };
    // SAFETY: `edit` is non-null (checked above) and points to a readable value (this function's
    // contract).
    let Some(pk) = edited(&save, &rules, loc, unsafe { &*edit }) else {
        return 200;
    };
    if mode == 0 {
        let mut r = crate::ui::empty_legality();
        legal::check(&save, &pk, &mut r);
        if r.status == 2 {
            return 100;
        }
    }
    match safety::checked_set_pkm(&rules, &mut save, slot_of(loc), &pk) {
        Ok(()) => 0,
        Err(safety::WriteError::Pkm(i)) => 1 + i as u8,
        Err(_) => 200,
    }
}

/// C ABI: whether a character can be stored in a name (always true for Gen 5's UTF-16).
#[unsafe(no_mangle)]
pub extern "C" fn verity_char_encodable(generation: u8, c: u16) -> u8 {
    (generation != 4 || verity::text::is_encodable_g4(&[c])) as u8
}

/// Request for `verity_egg_insert`. 0xFF means any.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct VerityEggRequest {
    pub version: u8,
    pub nature: u8,
    pub species: u16,
    pub gender: u8,
    pub shiny: u8,
    pub hatched: u8,
    pub date: [u8; 3],
    pub seed: u32,
}

/// C ABI: species an egg can be of in `version`. Returns how many were written.
///
/// # Safety
///
/// `out` must point to `cap` writable `u16`s.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn verity_egg_species(version: u8, out: *mut u16, cap: usize) -> usize {
    let (Some(_), Some(ctx)) = (wizard::slots_group(version), legal::context()) else {
        return 0;
    };
    if out.is_null() {
        return 0;
    }
    // SAFETY: `out` is non-null (checked above) and valid for `cap` writable elements (this
    // function's contract).
    let out = unsafe { slice::from_raw_parts_mut(out, cap) };
    let species = wizard::egg_species(ctx);
    let n = species.len().min(cap);
    out[..n].copy_from_slice(&species[..n]);
    n
}

/// C ABI: builds a legal bred egg for the save's trainer and writes it to `loc`. Returns a
/// `verity_gen_status_text` code. The save is not finalised.
///
/// # Safety
///
/// `buf` must point to `len` writable bytes; `req` must be readable.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn verity_egg_insert(
    buf: *mut u8,
    len: usize,
    req: *const VerityEggRequest,
    loc: VerityLoc,
) -> u8 {
    if req.is_null() {
        return GenStatus::Internal as u8;
    }
    // SAFETY: `req` is non-null (checked above) and points to a readable value (this function's
    // contract).
    let r = unsafe { *req };
    // SAFETY: `buf` is null or valid for `len` writable bytes, and the `Save` is dropped before
    // this call returns (this function's contract).
    let Some(mut save) = (unsafe { open(buf, len) }) else {
        return GenStatus::Internal as u8;
    };
    let Some(ctx) = legal::context() else {
        return GenStatus::Unsupported as u8;
    };
    let opt = |v: u8| (v != 0xFF).then_some(v);
    let e = wizard::EggRequest {
        version: r.version,
        species: r.species,
        nature: opt(r.nature),
        gender: opt(r.gender),
        shiny: r.shiny != 0,
        hatched: r.hatched != 0,
        date: r.date,
        seed: r.seed,
    };
    match wizard::insert_egg(&mut save, ctx, &e, slot_of(loc)) {
        Ok(()) => GenStatus::Ok as u8,
        Err(s) => s as u8,
    }
}

/// Result of `verity_closest_ivs`.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default)]
pub struct VerityIvFix {
    pub ivs: [u8; 6],
    pub distance: u16,
    pub pid: u32,
}

/// Spreads tested per lead. A shiny Caterpie needing 6 points took 0.7-1.7 s on the DS; a search
/// that runs out of budget is longer and has not been timed.
const CLOSEST_IV_BUDGET: u32 = 30_000;

/// C ABI: the legal IV spread closest to `target` (6 bytes, HP Atk Def Spe SpA SpD) for the Gen 4
/// wild catch at `loc`. Nothing is written. Returns 0 and fills `out`, else a
/// `verity_gen_status_text` code.
///
/// # Safety
///
/// `buf` must point to `len` writable bytes, `target` to 6 readable bytes, `out` must be valid.
#[unsafe(no_mangle)]
pub unsafe extern "C" fn verity_closest_ivs(
    buf: *mut u8,
    len: usize,
    loc: VerityLoc,
    target: *const u8,
    out: *mut VerityIvFix,
) -> u8 {
    if target.is_null() || out.is_null() {
        return GenStatus::Internal as u8;
    }
    // SAFETY: `buf` is null or valid for `len` writable bytes, and the `Save` is dropped before
    // this call returns (this function's contract).
    let Some(save) = (unsafe { open(buf, len) }) else {
        return GenStatus::Internal as u8;
    };
    let Some(ctx) = legal::context() else {
        return GenStatus::Unsupported as u8;
    };
    let Ok(pk) = save.pkm(slot_of(loc)) else {
        return GenStatus::SlotUnavailable as u8;
    };
    if pk.species() == 0 {
        return GenStatus::SlotUnavailable as u8;
    }
    // SAFETY: `target` is non-null (checked above) and points to 6 readable bytes (this function's
    // contract).
    let goal: [u8; 6] = core::array::from_fn(|i| unsafe { *target.add(i) });
    match api4::closest_ivs_for(ctx, &pk, goal, CLOSEST_IV_BUDGET) {
        Ok(f) => {
            let fix = VerityIvFix {
                ivs: f.ivs,
                distance: f.distance as u16,
                pid: f.pid,
            };
            // SAFETY: `out` is non-null (checked above) and points to a writable value (this
            // function's contract).
            unsafe { *out = fix };
            0
        }
        Err(e) => status(e) as u8,
    }
}

/// C ABI: text for a game-safe issue code from `verity_edit_preview`/`verity_edit_apply`.
#[unsafe(no_mangle)]
pub extern "C" fn verity_issue_text(code: u8) -> *const u8 {
    use verity::safety::PkmIssue as I;
    const ISSUES: [I; 22] = [
        I::WrongFormat,
        I::ChecksumMismatch,
        I::SanityFlags,
        I::Species,
        I::Form,
        I::Ability,
        I::HeldItem,
        I::Move,
        I::NoMoves,
        I::DuplicateMove,
        I::Pp,
        I::Nature,
        I::Level,
        I::Exp,
        I::Nickname,
        I::OtName,
        I::Language,
        I::OriginGame,
        I::Ball,
        I::MetLocation,
        I::EggLocation,
        I::PartyStats,
    ];
    match code {
        0 => c"OK".as_ptr().cast(),
        1..=22 => ISSUES[code as usize - 1].c_text().as_ptr(),
        100 => c"illegal (legal mode)".as_ptr().cast(),
        _ => c"cannot edit this slot".as_ptr().cast(),
    }
}

#[cfg(test)]
// Tests call the C ABI with buffers they own and keep alive for the call.
#[allow(clippy::undocumented_unsafe_blocks)]
mod tests {
    use super::*;
    use crate::ui::{BlobKind, VerityPkmView, verity_data_attach, verity_slot_view};
    extern crate std;

    fn file(rel: &str) -> Option<std::vec::Vec<u8>> {
        std::fs::read(std::format!("{}/../../{rel}", env!("CARGO_MANIFEST_DIR"))).ok()
    }

    /// The whole-save validator has to work for Gen 5 too, or a Gen 5 save can never be written.
    #[test]
    fn gen5_save_passes_validator() {
        let _guard = crate::TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let Some(mut save) = file("corpus/real/pkhex-avalonia/gen5_white2.sav") else {
            return;
        };
        for (n, k) in [
            ("b2w2.bin", BlobKind::B2w2),
            ("moves5.bin", BlobKind::Moves5),
        ] {
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
        let mut c = VeritySaveCheck::default();
        unsafe {
            assert_eq!(
                verity_save_check(save.as_mut_ptr(), save.len(), &mut c),
                VerityErr::Ok
            );
        }
        assert_eq!(c.issue, 0);
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
            ("enc_hgss.bin", BlobKind::EncHgss),
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
            // Editor: a moves edit that stays legal (Bidoof level-up moves) applies in legal
            // mode; an EV of 255 everywhere is legal in Gen 4 too (only the total matters
            // to PKHeX: 510), so use a duplicate move to see a game-safe refusal.
            let mut ed = VerityEdit {
                mask: EDIT_ITEM,
                held_item: 1,
                ..Default::default()
            };
            let mut lg = crate::ui::empty_legality();
            let mut issue = 0u8;
            assert_eq!(
                verity_edit_preview(b, len, loc, &ed, &mut lg, &mut issue),
                VerityErr::Ok
            );
            assert_eq!(issue, 0);
            ed = VerityEdit {
                mask: EDIT_MOVES,
                moves: [33, 33, 0, 0],
                ..Default::default()
            };
            verity_edit_preview(b, len, loc, &ed, &mut lg, &mut issue);
            assert_eq!(issue, 1 + safety::PkmIssue::DuplicateMove as u8);
            assert_eq!(verity_edit_apply(b, len, loc, &ed, 1), issue);
            // An illegal-but-safe edit (Psychic on Bidoof) is refused in legal mode only.
            ed = VerityEdit {
                mask: EDIT_MOVES,
                moves: [94, 0, 0, 0],
                ..Default::default()
            };
            verity_edit_preview(b, len, loc, &ed, &mut lg, &mut issue);
            assert_eq!((issue, lg.status), (0, 2));
            assert_eq!(verity_edit_apply(b, len, loc, &ed, 0), 100);
            assert_eq!(verity_edit_apply(b, len, loc, &ed, 1), 0);
            verity_slot_view(b, len, loc, &mut v);
            assert_eq!(v.moves[0], 94);
            assert_eq!(verity_save_finalize(b, len), VerityErr::Ok);
            let mut c = VeritySaveCheck::default();
            assert_eq!(verity_save_check(b, len, &mut c), VerityErr::Ok);
            assert_eq!(c.issue, 0, "{c:?}");
        }
    }
}
