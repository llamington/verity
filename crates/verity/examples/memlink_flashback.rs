//! Harness helper (not shipped): prepares a B2W2 save for the in-game Memory Link check. Links it
//! with `memlink::apply` (unless the BW path is `-`, the control), sets event flag 2400, which the
//! Route 1 flashback requires, and moves the player.
//!
//! `memlink_flashback <bw.sav|-> <b2w2.sav> <out.sav> <zone> <x> <z>`
use std::fs;

use verity::memlink::{apply, build, Seeds};
use verity::save::Save;

const POSITION: usize = 0x19500;
const EVENT_FLAGS: usize = 0x1FF00 + 0x13E * 2;
const FLAG: usize = 2400;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let mut data = fs::read(&a[2]).unwrap();
    if a[1] != "-" {
        let linked = build(&fs::read(&a[1]).unwrap()).unwrap();
        let seeds = Seeds { gifts: 0x5A, key_info: 0x1234_5678 };
        println!("{:?}", apply(&mut data, &linked, seeds, false).unwrap());
    }
    data[EVENT_FLAGS + FLAG / 8] |= 1 << (FLAG % 8);
    let [zone, x, z]: [u16; 3] = [4, 5, 6].map(|i| a[i].parse().unwrap());
    data[POSITION + 0x80..POSITION + 0x84].copy_from_slice(&u32::from(zone).to_le_bytes());
    data[POSITION + 0x86..POSITION + 0x88].copy_from_slice(&x.to_le_bytes());
    data[POSITION + 0x8A..POSITION + 0x8C].copy_from_slice(&0u16.to_le_bytes());
    data[POSITION + 0x8E..POSITION + 0x90].copy_from_slice(&z.to_le_bytes());
    let mut save = Save::open(&mut data).unwrap();
    save.finalize();
    fs::write(&a[3], &data).unwrap();
}
