use std::fs;
use std::path::PathBuf;

use serde_json::Value;
use verity::data::GameData;
use verity::pkm::{Format, Pkm};
use verity::rng::finder::{AnalyzeInput, analyze};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn method_finder_matches_pkhex() {
    let blobs = root().join("data/blobs");
    let (Ok(hgss), Ok(b2w2)) = (
        fs::read(blobs.join("hgss.bin")),
        fs::read(blobs.join("b2w2.bin")),
    ) else {
        eprintln!("skipping: build data blobs first");
        return;
    };
    let (g4, g5) = (
        GameData::load(&hgss).unwrap(),
        GameData::load(&b2w2).unwrap(),
    );
    let ratio = |s: u16| {
        if s <= 493 {
            g4.personal(s as usize).gender_ratio()
        } else {
            g5.personal(s as usize).gender_ratio()
        }
    };
    let reports = root().join("corpus/reports/oracle-smoke");
    let Ok(rd) = fs::read_dir(&reports) else {
        eprintln!("skipping: no reports");
        return;
    };
    let mut checked = 0;
    let mut kinds = std::collections::BTreeMap::new();
    let mut bad = Vec::new();
    for e in rd.flatten() {
        let p = e.path();
        let name = p.file_name().unwrap().to_string_lossy().to_string();
        let Some(stem) = name.strip_suffix(".dump.json") else {
            continue;
        };
        let dump: Value = serde_json::from_slice(&fs::read(&p).unwrap()).unwrap();
        let gen5 = dump["version"].as_str().unwrap().contains(['B', 'W']);
        let (fmt, ext) = if gen5 {
            (Format::Pk5, "pk5")
        } else {
            (Format::Pk4, "pk4")
        };
        for s in dump["slots"].as_array().unwrap() {
            let sname = s["name"].as_str().unwrap();
            let bytes = fs::read(reports.join(format!("{stem}.pkm/{sname}.{ext}"))).unwrap();
            let pk = Pkm::from_decrypted(fmt, &bytes).unwrap();
            let r = analyze(&AnalyzeInput::from_pkm(&pk), ratio);
            let want_t = s["pidiv_type"].as_u64().unwrap();
            let want_s = s["pidiv_seed"].as_u64().unwrap();
            *kinds.entry(format!("{:?}", r.kind)).or_insert(0) += 1;
            let seedless = matches!(want_t, 0 | 25 | 33 | 34);
            if r.kind as u64 != want_t || (!seedless && r.seed as u64 != want_s) {
                bad.push(format!(
                    "{stem}/{sname}: ours {:?} {:08X} pkhex {want_t} {want_s:08X}",
                    r.kind, r.seed
                ));
            }
            checked += 1;
        }
    }
    for b in bad.iter().take(20) {
        eprintln!("{b}");
    }
    eprintln!("checked {checked}: {kinds:?}");
    assert!(bad.is_empty(), "{} mismatches", bad.len());
}
