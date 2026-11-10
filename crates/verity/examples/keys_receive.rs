//! Harness helper (not shipped): prepares a B2W2 save for the in-game Key System check. Clears its
//! keys, then receives from a copy of itself whose five keys are earned, as the other version.
//!
//! `keys_receive <b2w2.sav> <out.sav> <blank|receive|partner>`; `partner` writes that copy itself.
use std::fs;

use verity::memlink::keys::receive;
use verity::save::Save;

const KEY_INFO: usize = 0x25800 + 0x28;
const VERSION: usize = 0x19400 + 0x1F;
const UNLOCKED: [u32; 5] = [0x35691, 0x18256, 0x59389, 0x48292, 0x09892];

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let mut target = fs::read(&a[1]).unwrap();
    target[KEY_INFO..KEY_INFO + 0x28].fill(0);
    Save::open(&mut target).unwrap().finalize();
    if a[3] != "blank" {
        let mut partner = target.clone();
        let seed = u32::from_le_bytes(
            partner[KEY_INFO + 0x34..KEY_INFO + 0x38]
                .try_into()
                .unwrap(),
        );
        for (i, m) in UNLOCKED.iter().enumerate() {
            partner[KEY_INFO + i * 4..KEY_INFO + i * 4 + 4]
                .copy_from_slice(&(m ^ seed).to_le_bytes());
        }
        partner[VERSION] ^= 1;
        Save::open(&mut partner).unwrap().finalize();
        if a[3] == "partner" {
            fs::write(&a[2], &partner).unwrap();
            return;
        }
        println!("{:?}", receive(&mut target, &partner).unwrap());
    }
    fs::write(&a[2], &target).unwrap();
}
