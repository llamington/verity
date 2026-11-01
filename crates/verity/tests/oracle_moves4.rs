use std::fs;
use std::path::PathBuf;

use serde_json::Value;
use verity::data::{GameData, MoveData};
use verity::legality::evo::EvoCriteria;
use verity::legality::moves4::Learn4;
use verity::pkm::{Format, Pkm};

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}

#[test]
fn gen4_moves_match_pkhex() {
    let b = root().join("data/blobs");
    let (Ok(hg), Ok(pt), Ok(dp), Ok(mv)) = (
        fs::read(b.join("hgss.bin")),
        fs::read(b.join("pt.bin")),
        fs::read(b.join("dp.bin")),
        fs::read(b.join("moves4.bin")),
    ) else {
        return;
    };
    let learn = Learn4 {
        hgss: GameData::load(&hg).unwrap(),
        pt: GameData::load(&pt).unwrap(),
        dp: GameData::load(&dp).unwrap(),
        moves: MoveData::load(&mv).unwrap(),
    };
    let reports = root().join("corpus/reports/oracle-smoke");
    let Ok(rd) = fs::read_dir(&reports) else {
        return;
    };
    let (mut checked, mut invalid_seen, mut bad) = (0, 0, Vec::new());
    for e in rd.flatten() {
        let p = e.path();
        let name = p.file_name().unwrap().to_string_lossy().to_string();
        let Some(stem) = name.strip_suffix(".dump.json") else {
            continue;
        };
        let dump: Value = serde_json::from_slice(&fs::read(&p).unwrap()).unwrap();
        for s in dump["slots"].as_array().unwrap() {
            if s["enc_kind"] != "EncounterSlot4" || s["evo_gen4"].is_null() {
                continue;
            }
            let sname = s["name"].as_str().unwrap();
            let pk = Pkm::from_decrypted(
                Format::Pk4,
                &fs::read(reports.join(format!("{stem}.pkm/{sname}.pk4"))).unwrap(),
            )
            .unwrap();
            let history: Vec<EvoCriteria> = s["evo_gen4"]
                .as_array()
                .unwrap()
                .iter()
                .map(|e| EvoCriteria {
                    species: e[0].as_u64().unwrap() as u16,
                    form: e[1].as_u64().unwrap() as u8,
                    level_min: e[2].as_u64().unwrap() as u8,
                    level_max: e[3].as_u64().unwrap() as u8,
                    level_up_required: e[4].as_u64().unwrap() as u8,
                    method: e[5].as_u64().unwrap() as u8,
                })
                .collect();
            let r = learn.verify(
                pk.moves(),
                pk.form(),
                history.last().map_or(0, |e| e.species),
                &history,
                None,
            );
            let ours: Vec<bool> = r.iter().map(|m| m.valid()).collect();
            let want: Vec<bool> = s["move_valid"]
                .as_array()
                .unwrap()
                .iter()
                .map(|v| v.as_bool().unwrap())
                .collect();
            invalid_seen += want.iter().filter(|v| !**v).count();
            if ours != want {
                bad.push(format!(
                    "{stem}/{sname} sp {} moves {:?}: ours {r:?} pkhex {:?}",
                    pk.species(),
                    pk.moves(),
                    s["move_method"]
                ));
            }
            checked += 1;
        }
    }
    for b in bad.iter().take(15) {
        eprintln!("{b}");
    }
    eprintln!("checked {checked} ({invalid_seen} invalid moves in PKHeX's verdicts)");
    assert!(bad.is_empty(), "{} mismatches", bad.len());
}

#[test]
fn gen4_moves_fuzz_match_pkhex() {
    let b = root().join("data/blobs");
    let oracle = root().join("tools/pkhex-oracle/bin/Release/net9.0/PkhexOracle");
    let (Ok(hg), Ok(pt), Ok(dp), Ok(mv)) = (
        fs::read(b.join("hgss.bin")),
        fs::read(b.join("pt.bin")),
        fs::read(b.join("dp.bin")),
        fs::read(b.join("moves4.bin")),
    ) else {
        return;
    };
    if !oracle.exists() {
        return;
    }
    let learn = Learn4 {
        hgss: GameData::load(&hg).unwrap(),
        pt: GameData::load(&pt).unwrap(),
        dp: GameData::load(&dp).unwrap(),
        moves: MoveData::load(&mv).unwrap(),
    };
    let reports = root().join("corpus/reports/oracle-smoke");
    let Ok(rd) = fs::read_dir(&reports) else {
        return;
    };
    let tmp = std::env::temp_dir().join(format!("verity-moves4-{}", std::process::id()));
    fs::create_dir_all(&tmp).unwrap();
    let mut seed = 0x9E37_79B9_7F4A_7C15u64;
    let mut next = || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    let mut written = Vec::new();
    for e in rd.flatten() {
        let p = e.path();
        let name = p.file_name().unwrap().to_string_lossy().to_string();
        let Some(stem) = name.strip_suffix(".dump.json") else {
            continue;
        };
        let dump: Value = serde_json::from_slice(&fs::read(&p).unwrap()).unwrap();
        for s in dump["slots"].as_array().unwrap() {
            if s["enc_kind"] != "EncounterSlot4" || s["evo_gen4"].is_null() {
                continue;
            }
            let sname = s["name"].as_str().unwrap();
            let bytes = fs::read(reports.join(format!("{stem}.pkm/{sname}.pk4"))).unwrap();
            let mut pk = Pkm::from_decrypted(Format::Pk4, &bytes).unwrap();
            let mut m = pk.moves();
            for _ in 0..(1 + next() % 3) {
                let i = (next() % 4) as usize;
                m[i] = match next() % 6 {
                    0 => 0,
                    1 => m[(i + 1) % 4],
                    _ => 1 + (next() % 467) as u16,
                };
            }
            pk.set_moves(m);
            pk.refresh_checksum();
            let f = format!("{}_{}.pk4", stem.replace(' ', "_"), sname);
            fs::write(tmp.join(&f), &pk.decrypted()[..136]).unwrap();
            written.push((f, pk));
        }
    }
    let out = std::process::Command::new(&oracle)
        .env(
            "DOTNET_ROOT",
            format!("{}/.dotnet", std::env::var("HOME").unwrap()),
        )
        .arg("legality-batch")
        .arg(&tmp)
        .output()
        .unwrap();
    let mut by_file = std::collections::HashMap::new();
    for line in String::from_utf8_lossy(&out.stdout).lines() {
        let v: Value = serde_json::from_str(line).unwrap();
        by_file.insert(v["file"].as_str().unwrap().to_string(), v);
    }
    let (mut checked, mut invalid_seen, mut bad) = (0, 0, Vec::new());
    for (f, pk) in &written {
        let v = &by_file[f];
        if v["enc_kind"] != "EncounterSlot4" || v["evo_gen4"].is_null() {
            continue;
        }
        let history: Vec<EvoCriteria> = v["evo_gen4"]
            .as_array()
            .unwrap()
            .iter()
            .map(|e| EvoCriteria {
                species: e[0].as_u64().unwrap() as u16,
                form: e[1].as_u64().unwrap() as u8,
                level_min: e[2].as_u64().unwrap() as u8,
                level_max: e[3].as_u64().unwrap() as u8,
                level_up_required: e[4].as_u64().unwrap() as u8,
                method: e[5].as_u64().unwrap() as u8,
            })
            .collect();
        let r = learn.verify(
            pk.moves(),
            pk.form(),
            history.last().map_or(0, |e| e.species),
            &history,
            None,
        );
        let ours: Vec<bool> = r.iter().map(|m| m.valid()).collect();
        let want: Vec<bool> = v["move_valid"]
            .as_array()
            .unwrap()
            .iter()
            .map(|x| x.as_bool().unwrap())
            .collect();
        invalid_seen += want.iter().filter(|x| !**x).count();
        if ours != want {
            bad.push(format!(
                "{f} sp {} moves {:?}: ours {r:?} pkhex {want:?}",
                pk.species(),
                pk.moves()
            ));
        }
        checked += 1;
    }
    let _ = fs::remove_dir_all(&tmp);
    for b in bad.iter().take(15) {
        eprintln!("{b}");
    }
    eprintln!("fuzz checked {checked} ({invalid_seen} invalid moves in PKHeX's verdicts)");
    assert!(invalid_seen > 0);
    assert!(bad.is_empty(), "{} mismatches", bad.len());
}
