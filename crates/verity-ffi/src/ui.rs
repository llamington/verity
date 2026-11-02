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
            BlobKind::EncDppt => verity::data::encounters::EncounterData::load(data).is_ok(),
            BlobKind::Names4 => Strings::load(data).is_ok(),
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

fn open(buf: *mut u8, len: usize) -> Option<Save<'static>> {
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
            verity_data_attach(BlobKind::Strings as u32, ptr::null(), 0);
            verity_data_attach(BlobKind::Pt as u32, ptr::null(), 0);
        }
    }
}
