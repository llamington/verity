//! Harness helper (not shipped): injects one Gen 4 gift and optionally moves the player, so a
//! retail game can be driven to the deliveryman.
//!
//! `gift4_inject <in.sav> <out.sav> <blob dir> <mgdb index> [map x z] [party]`
use std::fs;

use verity::gift::inject4::inject;
use verity::gift::pcd::Gifts4;
use verity::save::{Save, SaveKind};

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let mut data = fs::read(&a[1]).unwrap();
    let want: u16 = a[4].parse().unwrap();
    let mut save = Save::open(&mut data).unwrap();
    if std::env::var_os("CLEAR_SLOTS").is_some() {
        // Empty the pending-gift and card slots (types only) so the injected gift is first.
        let (general, m) = save.mystery4_mut().unwrap();
        let (m, sentinels) = (m, 0);
        general[m..m + 0x100].fill(0);
        for i in 0..8 {
            general[m + 0x100 + sentinels + i * 0x104] = 0;
        }
        for i in 0..3 {
            general[m + 0x100 + sentinels + 8 * 0x104 + i * 0x358] = 0;
        }
    }
    let mut done = false;
    for f in ["gifts4.bin", "events4.bin"] {
        let blob = fs::read(format!("{}/{f}", a[3])).unwrap();
        let db = Gifts4::load(&blob).unwrap();
        for g in db.iter().filter(|g| g.mgdb_index() == want) {
            inject(&mut save, &g, 9000).unwrap();
            done = true;
        }
    }
    assert!(done, "no such gift");
    if a.len() > 7 {
        let (m, x, z): (u16, u16, u16) = (
            a[5].parse().unwrap(),
            a[6].parse().unwrap(),
            a[7].parse().unwrap(),
        );
        // PKHeX `SAV4*.M/X/Y/X2/Y2`.
        let (om, ox, oz, ox2, oz2) = match save.kind() {
            SaveKind::DP => (0x1238, 0x1240, 0x1244, 0x25FA, 0x25FE),
            SaveKind::Pt => (0x1280, 0x1288, 0x128C, 0x287E, 0x2882),
            _ => (0x1234, 0x123C, 0x1240, 0x236E, 0x2372),
        };
        let (general, _) = save.mystery4_mut().unwrap();
        for (o, v) in [(om, m), (ox, x), (oz, z), (ox2, x), (oz2, z)] {
            general[o..o + 2].copy_from_slice(&v.to_le_bytes());
        }
    }
    if a.len() > 8 {
        save.set_party_count(a[8].parse().unwrap());
    }
    save.finalize();
    fs::write(&a[2], &data).unwrap();
}
