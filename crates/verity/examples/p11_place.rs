//! Harness helper (not shipped): prepares a save edited on the DS for P11. Finds the box slot the
//! wizard wrote, moves it to the first slot of its box, opens the PC on that box and moves the
//! player next to a PC. Prints `<box> <pid>`.
//!
//! `p11_place <before.sav> <edited.sav> <out.sav> <map> <x> <z>`
//! `p11_place clear <in.sav> <out.sav> <box> <slot>` empties a slot for the wizard first.
//! `p11_place check <placed.sav> <after.sav> <box> <pid>` checks the save the game wrote.
use std::fs;

use verity::save::{BOX_SLOTS, Save, SaveKind, Slot};

const GEN5_POSITION: usize = 0x19500;

fn offset(save: &Save, slot: Slot) -> usize {
    save.slot_raw(slot).unwrap().as_ptr() as usize - save.data().as_ptr() as usize
}

fn clear(a: &[String]) {
    let mut data = fs::read(&a[2]).unwrap();
    let (b, s): (u8, u8) = (a[4].parse().unwrap(), a[5].parse().unwrap());
    let mut save = Save::open(&mut data).unwrap();
    let empty = (0..save.box_count() as u8)
        .flat_map(|b| (0..BOX_SLOTS as u8).map(move |s| Slot::Box(b, s)))
        .find(|&slot| save.pkm(slot).unwrap().species() == 0)
        .expect("no empty slot to copy");
    let raw = save.slot_raw(empty).unwrap().to_vec();
    save.set_slot_raw(Slot::Box(b, s), &raw).unwrap();
    save.finalize();
    fs::write(&a[3], &data).unwrap();
}

/// Exits with a message unless the save the game wrote is sound: valid checksums, boxes untouched,
/// our Pokémon where we left it, and no Bad Egg anywhere.
fn check(a: &[String]) {
    let fail = |m: String| -> ! {
        println!("{m}");
        std::process::exit(1)
    };
    let mut placed = fs::read(&a[2]).unwrap();
    let mut after = fs::read(&a[3]).unwrap();
    let (b, pid): (u8, u32) = (a[4].parse().unwrap(), a[5].parse().unwrap());
    if placed == after {
        fail("the game did not save".into());
    }
    let old = Save::open(&mut placed).unwrap();
    let save = Save::open(&mut after).unwrap_or_else(|e| fail(format!("does not open: {e:?}")));
    if !save.checksums_valid() {
        fail("block checksums are wrong".into());
    }
    let slots = (0..save.box_count() as u8)
        .flat_map(|b| (0..BOX_SLOTS as u8).map(move |s| Slot::Box(b, s)))
        .chain((0..save.party_count() as u8).map(Slot::Party));
    for slot in slots {
        let pk = save
            .pkm(slot)
            .unwrap_or_else(|e| fail(format!("{slot:?}: {e:?}")));
        if pk.species() != 0 && (!pk.checksum_valid() || pk.decrypted()[4..6] != [0, 0]) {
            fail(format!("{slot:?} is a Bad Egg"));
        }
        if matches!(slot, Slot::Box(..))
            && save.slot_raw(slot).unwrap() != old.slot_raw(slot).unwrap()
        {
            fail(format!("{slot:?} changed"));
        }
    }
    if save.pkm(Slot::Box(b, 0)).unwrap().pid() != pid {
        fail("our Pokémon moved".into());
    }
    println!("ok");
}

fn main() {
    let a: Vec<String> = std::env::args().collect();
    if a[1] == "clear" {
        return clear(&a);
    }
    if a[1] == "check" {
        return check(&a);
    }
    let mut before = fs::read(&a[1]).unwrap();
    let mut data = fs::read(&a[2]).unwrap();
    let [map, x, z]: [u16; 3] = [4, 5, 6].map(|i| a[i].parse().unwrap());
    let old = Save::open(&mut before).unwrap();
    let mut save = Save::open(&mut data).unwrap();

    let found = (0..save.box_count() as u8)
        .flat_map(|b| (0..BOX_SLOTS as u8).map(move |s| Slot::Box(b, s)))
        .find(|&slot| save.slot_raw(slot).unwrap() != old.slot_raw(slot).unwrap())
        .expect("the wizard wrote no box slot");
    let Slot::Box(b, _) = found else {
        unreachable!()
    };
    let first = Slot::Box(b, 0);
    let (ours, theirs) = (
        save.slot_raw(found).unwrap().to_vec(),
        save.slot_raw(first).unwrap().to_vec(),
    );
    save.set_slot_raw(first, &ours).unwrap();
    save.set_slot_raw(found, &theirs).unwrap();
    let pid = save.pkm(first).unwrap().pid();

    let current = match save.kind() {
        SaveKind::DP | SaveKind::Pt => offset(&save, Slot::Box(0, 0)) - 4,
        SaveKind::HGSS => offset(&save, Slot::Box(0, 0)) + 18 * 0x1000,
        SaveKind::BW | SaveKind::B2W2 => 0,
    };
    let gen5 = matches!(save.kind(), SaveKind::BW | SaveKind::B2W2);
    if !gen5 {
        // PKHeX `SAV4*.M/X/Y/X2/Y2`.
        let (om, ox, oz, ox2, oz2) = match save.kind() {
            SaveKind::DP => (0x1238, 0x1240, 0x1244, 0x25FA, 0x25FE),
            SaveKind::Pt => (0x1280, 0x1288, 0x128C, 0x287E, 0x2882),
            _ => (0x1234, 0x123C, 0x1240, 0x236E, 0x2372),
        };
        let (general, _) = save.mystery4_mut().unwrap();
        for (o, v) in [(om, map), (ox, x), (oz, z), (ox2, x), (oz2, z)] {
            general[o..o + 2].copy_from_slice(&v.to_le_bytes());
        }
    }
    save.finalize();
    data[current] = b;
    if gen5 {
        let p = GEN5_POSITION;
        data[p + 0x80..p + 0x84].copy_from_slice(&u32::from(map).to_le_bytes());
        data[p + 0x86..p + 0x88].copy_from_slice(&x.to_le_bytes());
        data[p + 0x8A..p + 0x8C].copy_from_slice(&0u16.to_le_bytes());
        data[p + 0x8E..p + 0x90].copy_from_slice(&z.to_le_bytes());
    }
    Save::open(&mut data).unwrap().finalize();
    fs::write(&a[3], &data).unwrap();
    println!("{b} {pid}");
}
