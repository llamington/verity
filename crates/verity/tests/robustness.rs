use verity::save::{Save, Slot, detect};

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
}

#[test]
fn garbage_never_panics() {
    let mut rng = Rng(0xDEAD_BEEF_CAFE_F00D);
    for len in [0usize, 1, 0x1000, 0x7FFFF, 0x80000, 0x80001, 0x100000] {
        for fill in 0..4 {
            let mut buf: Vec<u8> = (0..len)
                .map(|_| match fill {
                    0 => 0,
                    1 => 0xFF,
                    _ => rng.next() as u8,
                })
                .collect();
            let _ = detect(&buf);
            if let Ok(mut save) = Save::open(&mut buf) {
                exercise(&mut save);
            }
        }
    }
}

#[test]
fn mutated_saves_never_panic() {
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../corpus/synthetic");
    let Ok(rd) = std::fs::read_dir(dir) else {
        return;
    };
    let mut rng = Rng(0x1234_5678_9ABC_DEF1);
    for e in rd.flatten() {
        let original = std::fs::read(e.path()).unwrap();
        for _ in 0..200 {
            let mut buf = original.clone();
            for _ in 0..(rng.next() % 64) {
                let i = (rng.next() as usize) % buf.len();
                buf[i] = rng.next() as u8;
            }
            if let Ok(mut save) = Save::open(&mut buf) {
                exercise(&mut save);
            }
        }
    }
}

fn exercise(save: &mut Save) {
    let _ = save.checksums_valid();
    let _ = save.trainer_ids();
    let _ = save.trainer_name_raw();
    let _ = save.party_count();
    for i in 0..8 {
        let _ = save.slot_raw(Slot::Party(i));
    }
    for b in 0..30 {
        for s in 0..32 {
            if let Ok(raw) = save.slot_raw(Slot::Box(b, s)) {
                let mut v = raw.to_vec();
                verity::pkm::decrypt_45(&mut v);
            }
        }
    }
    save.finalize();
}
