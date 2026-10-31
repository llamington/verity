#![no_main]

use std::path::PathBuf;
use std::sync::OnceLock;

use libfuzzer_sys::fuzz_target;
use verity::pkm::Pkm;
use verity::save::{Save, Slot};

fn bases() -> &'static [Vec<u8>] {
    static BASES: OnceLock<Vec<Vec<u8>>> = OnceLock::new();
    BASES.get_or_init(|| {
        let root = std::env::var("VERITY_CORPUS_DIR")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../corpus"));
        let mut out = Vec::new();
        let mut stack = vec![root];
        while let Some(dir) = stack.pop() {
            let Ok(rd) = std::fs::read_dir(&dir) else { continue };
            for e in rd.flatten() {
                let p = e.path();
                if p.is_dir() && !p.ends_with("reports") {
                    stack.push(p);
                } else if p.extension().is_some_and(|x| x == "sav" || x == "dsv") {
                    out.push(std::fs::read(p).unwrap());
                }
            }
        }
        out.sort();
        assert!(!out.is_empty(), "no corpus saves");
        out
    })
}

fuzz_target!(|input: &[u8]| {
    let Some((&pick, patches)) = input.split_first() else { return };
    let bases = bases();
    let mut buf = bases[pick as usize % bases.len()].clone();
    for p in patches.chunks_exact(4) {
        let ofs = (u32::from_le_bytes([p[0], p[1], p[2], 0]) as usize) % buf.len();
        buf[ofs] = p[3];
    }
    let Ok(mut save) = Save::open(&mut buf) else { return };
    let _ = save.checksums_valid();
    let _ = save.trainer();
    for b in 0..save.box_count() {
        let _ = save.box_name_raw(b);
    }
    for pi in 0..save.pouches().len() {
        for si in 0..save.pouches()[pi].slots as usize {
            let _ = save.bag_item(pi, si);
        }
    }
    for i in 0..save.event_flag_count() {
        let _ = save.event_flag(i);
    }
    for i in 0..save.event_work_count() {
        let _ = save.event_work(i);
    }
    for s in 1..700 {
        let _ = save.dex_caught(s);
        let _ = save.dex_seen(s);
    }
    let mut mons: Vec<Pkm> = Vec::new();
    for i in 0..6 {
        if let Ok(pk) = save.pkm(Slot::Party(i)) {
            mons.push(pk);
        }
    }
    for b in 0..save.box_count() as u8 {
        for s in 0..30 {
            if let Ok(pk) = save.pkm(Slot::Box(b, s)) {
                mons.push(pk);
            }
        }
    }
    for pk in &mons {
        save.register_dex(pk);
        let _ = pk.is_shiny();
        let _ = pk.nature();
        let _ = pk.ball();
    }
    if let Some(pk) = mons.first() {
        let _ = save.set_pkm(Slot::Box(0, 0), pk);
    }
    save.finalize();
});
