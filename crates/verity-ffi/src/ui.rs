//! Read-only FFI for the DS front end (P9.2, first slice): data blob registry, box names, slot
//! views and name lookups. Calls take the caller's save buffer; nothing is kept between calls
//! except the attached data blobs (which the caller keeps alive).

use core::cell::Cell;
use core::ptr;
use core::slice;

use verity::data::{string_table, GameData, Strings};
use verity::pkm::Pkm;
use verity::save::{Save, SaveKind, Slot};

use crate::VerityErr;

/// Data blobs the front end can attach (`verity_blob_kind`).
#[repr(u32)]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BlobKind {
    Strings = 1,
    Hgss = 2,
    Pt = 3,
    Dp = 4,
    Moves4 = 5,
    EncDppt = 6,
    B2w2 = 7,
    Names4 = 8,
    Bw = 9,
    Moves5 = 10,
    EncHgss = 11,
    Gifts4 = 12,
    Events4 = 13,
}

const SLOTS: usize = 16;

/// Attached blobs. ARMv5 has no atomics; the FFI is single-threaded by contract (the DS front end
/// calls it from its main loop only), which is what makes the `Sync` impl sound.
struct Registry {
    ptr: [Cell<*const u8>; SLOTS],
    len: [Cell<usize>; SLOTS],
}

// SAFETY: see the type's doc comment: the FFI is only called from one thread.
unsafe impl Sync for Registry {}

static REGISTRY: Registry = Registry {
    ptr: [const { Cell::new(ptr::null()) }; SLOTS],
    len: [const { Cell::new(0) }; SLOTS],
};

pub(crate) fn blob(kind: BlobKind) -> Option<&'static [u8]> {
    let i = kind as usize;
    let p = REGISTRY.ptr[i].get();
    let n = REGISTRY.len[i].get();
    // SAFETY: verity_data_attach's contract keeps the buffer alive and unmodified until detached.
    (!p.is_null()).then(|| unsafe { slice::from_raw_parts(p, n) })
}

fn kind_from_u32(v: u32) -> Option<BlobKind> {
    use BlobKind::*;
    Some(match v {
        1 => Strings,
        2 => Hgss,
        3 => Pt,
        4 => Dp,
        5 => Moves4,
        6 => EncDppt,
        7 => B2w2,
        8 => Names4,
        9 => Bw,
        10 => Moves5,
        11 => EncHgss,
        12 => Gifts4,
        13 => Events4,
        _ => return None,
    })
}

/// C ABI: attaches (or with a null `buf`, detaches) a data blob after validating it.
///
/// # Safety
///
/// `buf` must point to `len` readable bytes that stay valid and unchanged until the blob is
/// detached or replaced.
#[no_mangle]
pub unsafe extern "C" fn verity_data_attach(kind: u32, buf: *const u8, len: usize) -> VerityErr {
    let Some(k) = kind_from_u32(kind) else {
        return VerityErr::BadArgument;
    };
    if !buf.is_null() {
        let data = slice::from_raw_parts(buf, len);
        let ok = match k {
            BlobKind::Strings => Strings::load(data).is_ok(),
            BlobKind::Moves4 | BlobKind::Moves5 => verity::data::MoveData::load(data).is_ok(),
            BlobKind::EncDppt | BlobKind::EncHgss => {
                verity::data::encounters::EncounterData::load(data).is_ok()
            }
            BlobKind::Names4 => Strings::load(data).is_ok(),
            BlobKind::Gifts4 | BlobKind::Events4 => verity::gift::pcd::Gifts4::load(data).is_ok(),
            _ => GameData::load(data).is_ok(),
        };
        if !ok {
            return VerityErr::BadData;
        }
    }
    REGISTRY.len[k as usize].set(len);
    REGISTRY.ptr[k as usize].set(buf);
    VerityErr::Ok
}

/// Slot location (`verity_loc`): `box_index` 0xFF means the party.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct VerityLoc {
    pub box_index: u8,
    pub slot: u8,
}

impl VerityLoc {
    fn slot(self) -> Slot {
        if self.box_index == 0xFF {
            Slot::Party(self.slot)
        } else {
            Slot::Box(self.box_index, self.slot)
        }
    }
}

/// What the front end shows for one Pokémon (`verity_pkm_view`). Strings are UTF-16,
/// 0-terminated.
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct VerityPkmView {
    pub species: u16,
    pub held_item: u16,
    pub moves: [u16; 4],
    pub tid: u16,
    pub sid: u16,
    pub met_location: u16,
    pub pid: u32,
    pub exp: u32,
    pub nickname: [u16; 13],
    pub ot_name: [u16; 9],
    pub form: u8,
    /// 0 when the game's personal data is not attached.
    pub level: u8,
    pub nature: u8,
    pub ability: u8,
    pub gender: u8,
    pub ball: u8,
    pub met_level: u8,
    pub version: u8,
    pub language: u8,
    pub is_egg: u8,
    pub is_shiny: u8,
    pub checksum_ok: u8,
    pub ivs: [u8; 6],
    pub evs: [u8; 6],
    /// Primary and secondary type (0xFF when the personal data is not attached).
    pub types: [u8; 2],
    pub friendship: u8,
}

fn decode(gen4: bool, raw: &[u16], out: &mut [u16]) {
    let cap = out.len() - 1;
    let n = if gen4 {
        verity::text::decode4(raw, &mut out[..cap])
    } else {
        verity::text::decode5(raw, &mut out[..cap])
    };
    out[n] = 0;
}

fn personal_for(kind: SaveKind) -> Option<GameData<'static>> {
    let k = match kind {
        SaveKind::DP => BlobKind::Dp,
        SaveKind::Pt => BlobKind::Pt,
        SaveKind::HGSS => BlobKind::Hgss,
        SaveKind::BW => BlobKind::Bw,
        SaveKind::B2W2 => BlobKind::B2w2,
    };
    GameData::load(blob(k)?).ok()
}

fn view(kind: SaveKind, pk: &Pkm) -> VerityPkmView {
    let gen4 = matches!(pk.format(), verity::pkm::Format::Pk4);
    let mut v = VerityPkmView {
        species: pk.species(),
        held_item: pk.held_item(),
        moves: pk.moves(),
        tid: pk.tid(),
        sid: pk.sid(),
        met_location: pk.met_location(),
        pid: pk.pid(),
        exp: pk.exp(),
        nickname: [0; 13],
        ot_name: [0; 9],
        form: pk.form(),
        level: 0,
        nature: pk.nature(),
        ability: pk.ability(),
        gender: pk.gender(),
        ball: pk.ball(),
        met_level: pk.met_level(),
        version: pk.version(),
        language: pk.language(),
        is_egg: pk.is_egg() as u8,
        is_shiny: pk.is_shiny() as u8,
        checksum_ok: pk.checksum_valid() as u8,
        ivs: pk.ivs().map(|x| x as u8),
        evs: pk.evs().map(|x| x as u8),
        types: [0xFF; 2],
        friendship: pk.friendship(),
    };
    decode(gen4, &pk.nickname_raw(), &mut v.nickname);
    decode(gen4, &pk.ot_name_raw(), &mut v.ot_name);
    if let Some(game) = personal_for(kind) {
        if pk.species() != 0 && pk.species() <= game.max_species() {
            let pi = game.species_form(pk.species(), pk.form());
            v.level = pk.level(&pi.base_info());
            v.types = pi.types();
        }
    }
    v
}

pub(crate) fn open(buf: *mut u8, len: usize) -> Option<Save<'static>> {
    if buf.is_null() {
        return None;
    }
    // SAFETY: callers pass the save buffer they own for the duration of the call.
    Save::open(unsafe { slice::from_raw_parts_mut(buf, len) }).ok()
}

/// C ABI: number of boxes in the save (0 if unrecognised).
///
/// # Safety
///
/// `buf` must point to `len` writable bytes.
#[no_mangle]
pub unsafe extern "C" fn verity_box_count(buf: *mut u8, len: usize) -> u32 {
    open(buf, len).map_or(0, |s| s.box_count() as u32)
}

/// C ABI: box name as UTF-16 (0-terminated, `out` holds 21 units).
///
/// # Safety
///
/// `buf` must point to `len` writable bytes; `out` to 21 writable `u16`s.
#[no_mangle]
pub unsafe extern "C" fn verity_box_name(
    buf: *mut u8,
    len: usize,
    box_index: u32,
    out: *mut u16,
) -> VerityErr {
    if out.is_null() {
        return VerityErr::NullPointer;
    }
    let Some(save) = open(buf, len) else {
        return VerityErr::Unrecognized;
    };
    let Ok(raw) = save.box_name_raw(box_index as usize) else {
        return VerityErr::BadArgument;
    };
    let out = slice::from_raw_parts_mut(out, 21);
    decode(save.generation() == 4, &raw, out);
    VerityErr::Ok
}

/// C ABI: the Pokémon at `loc`. An empty slot gives `species == 0`.
///
/// # Safety
///
/// `buf` must point to `len` writable bytes; `out` to a writable `VerityPkmView`.
#[no_mangle]
pub unsafe extern "C" fn verity_slot_view(
    buf: *mut u8,
    len: usize,
    loc: VerityLoc,
    out: *mut VerityPkmView,
) -> VerityErr {
    if out.is_null() {
        return VerityErr::NullPointer;
    }
    let Some(save) = open(buf, len) else {
        return VerityErr::Unrecognized;
    };
    if let Slot::Party(i) = loc.slot() {
        if i as usize >= save.party_count() {
            let mut v = core::mem::zeroed::<VerityPkmView>();
            v.types = [0xFF; 2];
            *out = v;
            return VerityErr::Ok;
        }
    }
    match save.pkm(loc.slot()) {
        Ok(pk) => {
            *out = view(save.kind(), &pk);
            VerityErr::Ok
        }
        Err(_) => VerityErr::BadArgument,
    }
}

/// Name categories for `verity_name` (`verity_name_kind`).
pub const NAME_SPECIES: u32 = string_table::SPECIES as u32;

/// C ABI: a name from the attached strings blob (species, moves, abilities, natures, types,
/// items) as UTF-16, 0-terminated in `out` (`cap` units). Returns the length written.
///
/// # Safety
///
/// `out` must point to `cap` writable `u16`s.
#[no_mangle]
pub unsafe extern "C" fn verity_name(category: u32, id: u32, out: *mut u16, cap: usize) -> usize {
    if out.is_null() || cap == 0 {
        return 0;
    }
    let out = slice::from_raw_parts_mut(out, cap);
    out[0] = 0;
    let Some(strings) = blob(BlobKind::Strings).and_then(|b| Strings::load(b).ok()) else {
        return 0;
    };
    if (id as usize) >= strings.count(category as u16) {
        return 0;
    }
    let s = strings.get(category as u16, id as usize);
    let mut n = 0;
    for c in s.iter() {
        if n + 1 >= cap {
            break;
        }
        out[n] = c;
        n += 1;
    }
    out[n] = 0;
    n
}

/// C ABI: a met/egg location name (Gen 4 or 5) from the attached strings blob, UTF-16 and
/// 0-terminated in `out` (`cap` units). Returns the length (0 if unknown).
///
/// # Safety
///
/// `out` must point to `cap` writable `u16`s.
#[no_mangle]
pub unsafe extern "C" fn verity_location_name(
    generation: u8,
    id: u16,
    out: *mut u16,
    cap: usize,
) -> usize {
    if out.is_null() || cap == 0 {
        return 0;
    }
    let out = slice::from_raw_parts_mut(out, cap);
    out[0] = 0;
    let Some(strings) = blob(BlobKind::Strings).and_then(|b| Strings::load(b).ok()) else {
        return 0;
    };
    let mut n = 0;
    for c in strings.location(generation, id).iter() {
        if n + 1 >= cap {
            break;
        }
        out[n] = c;
        n += 1;
    }
    out[n] = 0;
    n
}

/// Legality verdict for one slot (`verity_legality_report`).
#[repr(C)]
#[derive(Clone, Copy, Debug)]
pub struct VerityLegality {
    /// 0 not checked (unsupported origin or data missing), 1 legal, 2 illegal.
    pub status: u8,
    /// Number of entries in `checks` (failed PKHeX check identifiers, `CheckId` order).
    pub check_count: u8,
    pub checks: [u8; 12],
    /// Per move: 1 valid, 0 invalid.
    pub move_ok: [u8; 4],
    /// Matched encounter: location and slot number (0xFFFF/0xFF when none).
    pub enc_location: u16,
    pub enc_slot: u8,
    /// Why status is 0: 2 origin game not yet supported, 3 data missing, 4 not Gen 4,
    /// 5 hatched, 6 no encounter matched (events, trades and Pokéwalker are not checked yet).
    pub unsupported: u8,
}

pub(crate) fn empty_legality() -> VerityLegality {
    VerityLegality {
        status: 0,
        check_count: 0,
        checks: [0; 12],
        move_ok: [1; 4],
        enc_location: 0xFFFF,
        enc_slot: 0xFF,
        unsupported: 0,
    }
}

#[cfg(feature = "alloc")]
pub(crate) mod legal {
    use super::*;
    use core::cell::Cell;
    use verity::data::encounters::EncounterData;
    use verity::data::MoveData;
    use verity::legality::analysis4::{analyze4, Context4, Matched};
    use verity::legality::{evo::Evo4, moves4::Learn4};

    struct Holder(Cell<Option<&'static Context4<'static>>>);
    // SAFETY: single-threaded FFI (see `Registry`).
    unsafe impl Sync for Holder {}
    static CTX: Holder = Holder(Cell::new(None));

    /// The Gen 4 legality context, built once from attached blobs.
    pub(crate) fn context() -> Option<&'static Context4<'static>> {
        if let Some(c) = CTX.0.get() {
            return Some(c);
        }
        let g = |k| blob(k).and_then(|b| GameData::load(b).ok());
        let (hgss, pt, dp, gen5) = (
            g(BlobKind::Hgss)?,
            g(BlobKind::Pt)?,
            g(BlobKind::Dp)?,
            g(BlobKind::B2w2)?,
        );
        let ctx = Context4 {
            hgss,
            learn: Learn4 {
                hgss,
                pt,
                dp,
                moves: MoveData::load(blob(BlobKind::Moves4)?).ok()?,
            },
            evo: Evo4::new(dp, hgss),
            enc_dppt: EncounterData::load(blob(BlobKind::EncDppt)?).ok()?,
            enc_hgss: EncounterData::load(blob(BlobKind::EncHgss)?).ok()?,
            gen5,
            names4: Strings::load(blob(BlobKind::Names4)?).ok()?,
            // Optional: without the event database gifts are not matched.
            gifts: blob(BlobKind::Gifts4)
                .and_then(|b| verity::gift::pcd::Gifts4::load(b).ok())
                .unwrap_or(verity::gift::pcd::Gifts4::empty()),
        };
        let leaked: &'static Context4<'static> =
            alloc::boxed::Box::leak(alloc::boxed::Box::new(ctx));
        CTX.0.set(Some(leaked));
        Some(leaked)
    }

    pub(crate) fn check(save: &Save, pk: &Pkm, out: &mut VerityLegality) {
        if save.generation() != 4 {
            out.unsupported = 4;
            return;
        }
        if !matches!(pk.version(), 7 | 8 | 10..=12) {
            out.unsupported = 2;
            return;
        }
        let Some(ctx) = context() else {
            out.unsupported = 3;
            return;
        };
        let r = analyze4(ctx, pk);
        // Only wild slots are modelled; without a slot match the origin is something verity
        // cannot judge yet, so it says nothing rather than a false "illegal".
        if matches!(r.matched, Matched::Invalid) {
            out.unsupported = 6;
            return;
        }
        out.status = if r.valid() { 1 } else { 2 };
        for (i, c) in r.invalid.iter().take(out.checks.len()).enumerate() {
            out.checks[i] = *c as u8;
            out.check_count = i as u8 + 1;
        }
        out.move_ok = r.moves.map(|m| m.valid() as u8);
        if let Matched::Slot { encounter, .. } = r.matched {
            out.enc_location = encounter.location();
            out.enc_slot = encounter.slot_number();
        }
    }
}

/// C ABI: legality of the Pokémon at `loc` (PKHeX-equivalent; D/P/Pt wild origins for now).
/// Needs the Hgss, Pt, Dp, B2w2, Moves4, EncDppt, EncHgss and Names4 blobs attached; Mystery
/// Gifts are matched when Gifts4 is attached too (before the first call: the context is cached).
///
/// # Safety
///
/// `buf` must point to `len` writable bytes; `out` to a writable `VerityLegality`.
#[no_mangle]
pub unsafe extern "C" fn verity_legality(
    buf: *mut u8,
    len: usize,
    loc: VerityLoc,
    out: *mut VerityLegality,
) -> VerityErr {
    if out.is_null() {
        return VerityErr::NullPointer;
    }
    let Some(save) = open(buf, len) else {
        return VerityErr::Unrecognized;
    };
    let mut r = empty_legality();
    if let Slot::Party(i) = loc.slot() {
        if i as usize >= save.party_count() {
            return VerityErr::BadArgument;
        }
    }
    let Ok(pk) = save.pkm(loc.slot()) else {
        return VerityErr::BadArgument;
    };
    if pk.species() == 0 {
        return VerityErr::BadArgument;
    }
    #[cfg(feature = "alloc")]
    legal::check(&save, &pk, &mut r);
    #[cfg(not(feature = "alloc"))]
    {
        r.unsupported = 3;
    }
    *out = r;
    VerityErr::Ok
}

#[cfg(test)]
mod tests {
    use super::*;
    extern crate std;
    use std::vec::Vec;

    fn file(rel: &str) -> Option<Vec<u8>> {
        std::fs::read(std::format!("{}/../../{rel}", env!("CARGO_MANIFEST_DIR"))).ok()
    }

    #[test]
    fn views_of_a_real_save() {
        let _guard = crate::TEST_LOCK.lock().unwrap_or_else(|e| e.into_inner());
        let (Some(strings), Some(pt), Some(mut save)) = (
            file("data/blobs/strings_en.bin"),
            file("data/blobs/pt.bin"),
            file("corpus/real/pkhex-avalonia/gen4_platinum.sav"),
        ) else {
            return;
        };
        let strings: &'static [u8] = std::boxed::Box::leak(strings.into_boxed_slice());
        let pt: &'static [u8] = std::boxed::Box::leak(pt.into_boxed_slice());
        unsafe {
            assert_eq!(
                verity_data_attach(99, pt.as_ptr(), pt.len()),
                VerityErr::BadArgument
            );
            assert_eq!(
                verity_data_attach(BlobKind::Pt as u32, strings.as_ptr(), 16),
                VerityErr::BadData
            );
            assert_eq!(
                verity_data_attach(BlobKind::Strings as u32, strings.as_ptr(), strings.len()),
                VerityErr::Ok
            );
            assert_eq!(
                verity_data_attach(BlobKind::Pt as u32, pt.as_ptr(), pt.len()),
                VerityErr::Ok
            );
            let (b, n) = (save.as_mut_ptr(), save.len());
            assert_eq!(verity_box_count(b, n), 18);
            let mut name = [0u16; 21];
            assert_eq!(verity_box_name(b, n, 0, name.as_mut_ptr()), VerityErr::Ok);
            assert!(name[0] != 0);
            assert_eq!(
                verity_box_name(b, n, 99, name.as_mut_ptr()),
                VerityErr::BadArgument
            );

            let mut v = core::mem::zeroed::<VerityPkmView>();
            let party = VerityLoc {
                box_index: 0xFF,
                slot: 0,
            };
            assert_eq!(verity_slot_view(b, n, party, &mut v), VerityErr::Ok);
            assert!(v.species != 0 && (1..=100).contains(&v.level) && v.nickname[0] != 0);
            assert_eq!(v.checksum_ok, 1);
            let mut sp = [0u16; 16];
            assert!(verity_name(NAME_SPECIES, v.species as u32, sp.as_mut_ptr(), 16) > 0);
            assert_eq!(verity_name(NAME_SPECIES, 60000, sp.as_mut_ptr(), 16), 0);
            let mut loc = [0u16; 24];
            let loc_len = verity_location_name(4, 53, loc.as_mut_ptr(), 24);
            assert_eq!(
                std::string::String::from_utf16_lossy(&loc[..loc_len]),
                "Solaceon Ruins"
            );
            assert!(verity_location_name(4, 3002, loc.as_mut_ptr(), 24) > 0);
            assert!(verity_location_name(5, 30001, loc.as_mut_ptr(), 24) > 0);
            assert_eq!(verity_location_name(4, 999, loc.as_mut_ptr(), 24), 0);
            // Beyond the party count: empty.
            let past = VerityLoc {
                box_index: 0xFF,
                slot: 5,
            };
            if crate::verity_save_probe(b, n, &mut crate::VeritySaveInfo::default())
                == VerityErr::Ok
            {
                assert_eq!(verity_slot_view(b, n, past, &mut v), VerityErr::Ok);
            }
            // Legality: a Pt wild mon in the real save checks out like PKHeX says (see p5 corpus).
            let names = [
                "hgss.bin",
                "dp.bin",
                "b2w2.bin",
                "moves4.bin",
                "enc_dppt.bin",
                "names4.bin",
                "enc_hgss.bin",
                "gifts4.bin",
            ];
            let kinds = [
                BlobKind::Hgss,
                BlobKind::Dp,
                BlobKind::B2w2,
                BlobKind::Moves4,
                BlobKind::EncDppt,
                BlobKind::Names4,
                BlobKind::EncHgss,
                BlobKind::Gifts4,
            ];
            for (n, k) in names.iter().zip(kinds) {
                let b: &'static [u8] = std::boxed::Box::leak(
                    file(&std::format!("data/blobs/{n}"))
                        .unwrap()
                        .into_boxed_slice(),
                );
                assert_eq!(
                    verity_data_attach(k as u32, b.as_ptr(), b.len()),
                    VerityErr::Ok,
                    "{n}"
                );
            }
            // PKHeX's verdict per slot, from the oracle dump of the same save (Pt and HG).
            for stem in ["gen4_platinum", "gen4_heartgold"] {
                let Some(mut sv) = file(&std::format!("corpus/real/pkhex-avalonia/{stem}.sav"))
                else {
                    continue;
                };
                let (b, n) = (sv.as_mut_ptr(), sv.len());
                let dump: serde_json::Value = serde_json::from_slice(
                    &file(&std::format!(
                        "corpus/reports/oracle-smoke/{stem}.dump.json"
                    ))
                    .unwrap_or_default(),
                )
                .unwrap_or(serde_json::Value::Null);
                let pkhex = |name: &str| -> Option<bool> {
                    dump["slots"]
                        .as_array()?
                        .iter()
                        .find(|s| s["name"] == name)?["legal"]
                        .as_bool()
                };
                let mut r = core::mem::zeroed::<VerityLegality>();
                let (mut legal, mut illegal, mut skipped) = (0, 0, 0);
                let mut disagree = std::vec::Vec::new();
                for bx in 0..18u8 {
                    for sl in 0..30u8 {
                        let loc = VerityLoc {
                            box_index: bx,
                            slot: sl,
                        };
                        if verity_legality(b, n, loc, &mut r) != VerityErr::Ok {
                            continue;
                        }
                        match r.status {
                            1 => legal += 1,
                            2 => illegal += 1,
                            _ => skipped += 1,
                        }
                        let name = std::format!("box{bx:02}_{sl:02}");
                        if r.status != 0 {
                            if let Some(p) = pkhex(&name) {
                                if p != (r.status == 1) {
                                    disagree.push(name);
                                }
                            }
                        }
                    }
                }
                std::eprintln!("{stem}: legal {legal} illegal {illegal} not checked {skipped}");
                assert!(legal > 0, "{stem}");
                assert!(
                    disagree.is_empty(),
                    "{stem}: verdicts differ from PKHeX: {disagree:?}"
                );
            }
            verity_data_attach(BlobKind::Strings as u32, ptr::null(), 0);
            verity_data_attach(BlobKind::Pt as u32, ptr::null(), 0);
        }
    }
}
