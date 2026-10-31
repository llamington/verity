use std::fs;
use std::path::PathBuf;
use std::process::Command;

use verity::save::{Save, Slot, BOX_SLOTS};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn oracle() -> Option<PathBuf> {
    let p = root().join("tools/pkhex-oracle/bin/Release/net9.0/PkhexOracle");
    p.exists().then_some(p)
}

struct Rng(u64);
impl Rng {
    fn next(&mut self) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        self.0
    }
    fn below(&mut self, n: usize) -> usize {
        (self.next() % n as u64) as usize
    }
}

fn name(slot: Slot) -> String {
    match slot {
        Slot::Party(i) => format!("party_{i}"),
        Slot::Box(b, s) => format!("box{b:02}_{s:02}"),
    }
}

fn apply(save: &mut Save, op: &str, from: Slot, to: Slot) {
    let a = save.slot_raw(from).unwrap().to_vec();
    let b = save.slot_raw(to).unwrap().to_vec();
    save.set_slot_raw(to, &a).unwrap();
    if op == "swap" {
        save.set_slot_raw(from, &b).unwrap();
    }
}

#[test]
fn random_edits_match_pkhex() {
    let Some(oracle) = oracle() else {
        eprintln!("skipping: oracle not built");
        return;
    };
    let synth = root().join("corpus/synthetic");
    let Ok(rd) = fs::read_dir(&synth) else {
        eprintln!("skipping: no corpus");
        return;
    };
    let tmp = std::env::temp_dir().join(format!("verity-apply-{}", std::process::id()));
    fs::create_dir_all(&tmp).unwrap();
    let mut paths: Vec<_> = rd.flatten().map(|e| e.path()).collect();
    paths.sort();

    for (n, path) in paths.iter().enumerate() {
        let original = fs::read(path).unwrap();
        let mut buf = original.clone();
        let mut save = Save::open(&mut buf).unwrap();
        let mut rng = Rng(0x9E37_79B9_7F4A_7C15 ^ n as u64);
        let party = save.party_count();
        let boxes = save.box_count();

        let occupied = |save: &Save, s: Slot| save.slot_raw(s).unwrap().iter().any(|&b| b != 0);
        let mut ops = Vec::new();
        while ops.len() < 60 {
            let op = if rng.below(2) == 0 { "copy" } else { "swap" };
            let (from, to) = if party > 1 && rng.below(5) == 0 {
                (
                    Slot::Party(rng.below(party) as u8),
                    Slot::Party(rng.below(party) as u8),
                )
            } else {
                let mut pick = || Slot::Box(rng.below(boxes) as u8, rng.below(BOX_SLOTS) as u8);
                (pick(), pick())
            };
            // Writing an empty slot through PKHeX re-encrypts it; see docs/discrepancies.md.
            if !occupied(&save, from) || (op == "swap" && !occupied(&save, to)) {
                continue;
            }
            apply(&mut save, op, from, to);
            ops.push(serde_json::json!({"op": op, "from": name(from), "to": name(to)}));
        }
        save.finalize();

        let stem = path.file_stem().unwrap().to_str().unwrap();
        let ops_path = tmp.join(format!("{stem}.ops.json"));
        let out_path = tmp.join(format!("{stem}.out.sav"));
        fs::write(&ops_path, serde_json::to_vec(&ops).unwrap()).unwrap();
        let status = Command::new(&oracle)
            .env(
                "DOTNET_ROOT",
                std::env::var("DOTNET_ROOT")
                    .unwrap_or_else(|_| format!("{}/.dotnet", std::env::var("HOME").unwrap())),
            )
            .args(["sav-apply"])
            .arg(path)
            .arg(&ops_path)
            .arg(&out_path)
            .output()
            .unwrap();
        assert!(
            status.status.success(),
            "{}",
            String::from_utf8_lossy(&status.stderr)
        );
        let expected = fs::read(&out_path).unwrap();

        if buf != expected {
            let diffs: Vec<_> = (0..buf.len()).filter(|&i| buf[i] != expected[i]).collect();
            panic!(
                "{stem}: {} bytes differ, first at {:#X} (ours {:02X}, pkhex {:02X})",
                diffs.len(),
                diffs[0],
                buf[diffs[0]],
                expected[diffs[0]]
            );
        }
        eprintln!("ok {stem}: 60 ops byte-identical");
    }
    let _ = fs::remove_dir_all(&tmp);
}
