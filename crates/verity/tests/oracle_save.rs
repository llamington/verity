use std::fs;
use std::path::{Path, PathBuf};

use verity::pkm::decrypt_45;
use verity::save::{Save, SaveKind, Slot};

fn corpus() -> PathBuf {
    std::env::var("VERITY_CORPUS_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../corpus"))
}

fn saves(root: &Path) -> Vec<PathBuf> {
    let mut out = Vec::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(rd) = fs::read_dir(&dir) else { continue };
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() && !p.ends_with("reports") {
                stack.push(p);
            } else if p.extension().is_some_and(|x| x == "sav" || x == "dsv") {
                out.push(p);
            }
        }
    }
    out.sort();
    out
}

fn expected_kind(version: &str) -> SaveKind {
    match version {
        "DP" | "D" | "P" => SaveKind::DP,
        "Pt" => SaveKind::Pt,
        "HGSS" | "HG" | "SS" => SaveKind::HGSS,
        "B" | "W" | "BW" => SaveKind::BW,
        "B2" | "W2" | "B2W2" => SaveKind::B2W2,
        v => panic!("unexpected version {v}"),
    }
}

fn slot_from_name(name: &str) -> Slot {
    if let Some(i) = name.strip_prefix("party_") {
        Slot::Party(i.parse().unwrap())
    } else {
        let (b, s) = name.strip_prefix("box").unwrap().split_once('_').unwrap();
        Slot::Box(b.parse().unwrap(), s.parse().unwrap())
    }
}

#[test]
fn saves_match_pkhex() {
    let root = corpus();
    let reports = root.join("reports/oracle-smoke");
    let list = saves(&root);
    if list.is_empty() || !reports.exists() {
        eprintln!("skipping: no corpus/oracle reports (run scripts/oracle-smoke.sh)");
        return;
    }

    for path in list {
        let name = path.file_stem().unwrap().to_str().unwrap();
        let original = fs::read(&path).unwrap();
        let info: serde_json::Value =
            serde_json::from_slice(&fs::read(reports.join(format!("{name}.info.json"))).unwrap())
                .unwrap();
        let dump: serde_json::Value =
            serde_json::from_slice(&fs::read(reports.join(format!("{name}.dump.json"))).unwrap())
                .unwrap();
        let pkm_dir = reports.join(format!("{name}.pkm"));

        let mut buf = original.clone();
        let mut save = Save::open(&mut buf).unwrap_or_else(|e| panic!("{name}: {e:?}"));

        assert_eq!(
            save.kind(),
            expected_kind(info["version"].as_str().unwrap()),
            "{name}"
        );
        assert!(save.checksums_valid(), "{name}: checksums");
        assert_eq!(
            save.party_count() as u64,
            info["party_count"].as_u64().unwrap(),
            "{name}"
        );
        let ids = save.trainer_ids();
        assert_eq!(ids.tid as u64, info["tid"].as_u64().unwrap(), "{name}");
        assert_eq!(ids.sid as u64, info["sid"].as_u64().unwrap(), "{name}");

        let raw = save.trainer_name_raw();
        let mut tbuf = [0u16; 8];
        let tlen = if save.generation() == 4 {
            verity::text::decode4(&raw, &mut tbuf)
        } else {
            verity::text::decode5(&raw, &mut tbuf)
        };
        assert_eq!(
            String::from_utf16_lossy(&tbuf[..tlen]),
            info["trainer_name"].as_str().unwrap(),
            "{name}: trainer name"
        );

        let decode = |raw: &[u16]| {
            let mut b = [0u16; 24];
            let n = if save.generation() == 4 {
                verity::text::decode4(raw, &mut b)
            } else {
                verity::text::decode5(raw, &mut b)
            };
            String::from_utf16_lossy(&b[..n])
        };
        let t = save.trainer();
        assert_eq!(
            t.gender as u64,
            info["gender_id"].as_u64().unwrap(),
            "{name}: gender"
        );
        assert_eq!(
            t.language as u64,
            info["language_id"].as_u64().unwrap(),
            "{name}: language"
        );
        assert_eq!(
            t.money as u64,
            info["money"].as_u64().unwrap(),
            "{name}: money"
        );
        assert_eq!(
            t.badges as u64,
            info["badges"].as_u64().unwrap(),
            "{name}: badges"
        );
        let played: Vec<u64> = info["played"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_u64().unwrap())
            .collect();
        assert_eq!(
            vec![
                t.played_hours as u64,
                t.played_minutes as u64,
                t.played_seconds as u64
            ],
            played,
            "{name}: play time"
        );
        for (b, want) in info["box_names"].as_array().unwrap().iter().enumerate() {
            let got = decode(&save.box_name_raw(b).unwrap());
            assert_eq!(got, want.as_str().unwrap(), "{name}: box {b} name");
        }

        let ext = if save.generation() == 4 { "pk4" } else { "pk5" };
        let slots = dump["slots"].as_array().unwrap();
        assert!(!slots.is_empty());
        for s in slots {
            let sname = s["name"].as_str().unwrap();
            let slot = slot_from_name(sname);
            let raw = save.slot_raw(slot).unwrap();
            let enc = fs::read(pkm_dir.join(format!("{sname}.enc"))).unwrap();
            assert_eq!(raw, &enc[..], "{name}/{sname}: encrypted bytes");
            let mut dec = raw.to_vec();
            decrypt_45(&mut dec);
            let want = fs::read(pkm_dir.join(format!("{sname}.{ext}"))).unwrap();
            assert_eq!(dec, want, "{name}/{sname}: decrypted bytes");
        }

        save.finalize();
        let pkhex_export = fs::read(reports.join(format!("{name}.export"))).unwrap();
        assert!(
            buf == pkhex_export,
            "{name}: unedited export must match PKHeX's export"
        );
        eprintln!("ok {name}: {} slots", slots.len());
    }
}
